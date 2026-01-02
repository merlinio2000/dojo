use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
};

use crate::{
    bitmagic,
    board::one_bit::OneBitBoard,
    consts::{self, N_CELLS},
    types::{BoardState, Player},
    util::BoardMajorBitset,
};

type NodeIdx = u32;

type MonteCarloScore = i32;
const NO_MOVE_FORCED: u8 = 9;

/// TODO MERBUG: is it possible to reach the same state but with a different active player?
/// NOTE: NodeState::default() is not a valid node state and more of a placeholder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct NodeState {
    /// # bits[0]
    /// bitset indicating is_occupied for player 1
    ///
    /// # bits[1] (including meta)
    /// [80:0] = bitset indicating is_occupied for player 2
    /// upper 32 bits are reserved for meta data
    /// the upper 4 bits are used to signify the forced board
    /// MSB(127) = 0 -> no move forced
    /// [99:96] = forced board idx (9 = no forced board)
    /// [113:112] = active player as u8, see [`Player`]
    bits: [u128; 2],
}

impl NodeState {
    const META_OFFSET: usize = (128 - 32);
    const PLAYER_OFFSET_IN_META: usize = 16;
    const fn empty() -> Self {
        Self {
            bits: [0, (NO_MOVE_FORCED as u128) << Self::META_OFFSET],
        }
    }
    const fn player1_occupied(&self) -> BoardMajorBitset {
        // safety: no metadata is ever stored in the first bitset
        unsafe { BoardMajorBitset::new_unchecked(self.bits[0]) }
    }
    const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[1])
    }

    const fn meta(&self) -> u32 {
        (self.bits[1] >> Self::META_OFFSET) as u32
    }
    const fn forced_move(&self) -> u8 {
        self.meta() as u8
    }
    const fn active_player(&self) -> Player {
        Player::from_is_player2(self.meta() >> Self::PLAYER_OFFSET_IN_META != 0)
    }

    const fn get_player_board(&self, player: Player, board_idx: u8) -> OneBitBoard {
        OneBitBoard::new((self.bits[player as usize] >> (board_idx as u32 * N_CELLS)) as BoardState)
    }
    fn available_in_board_or_fallback(&self) -> BoardMajorBitset {
        // TODO PERF: this code has an unnecessary '& GRID_MASK', check the asm
        let is_occupied = self.player1_occupied() | self.player2_occupied();
        let is_available = !is_occupied;

        let forced_move = self.forced_move();

        if forced_move == NO_MOVE_FORCED {
            return is_available;
        }

        let board_mask = BoardMajorBitset::new_full_board(forced_move as u32);
        let is_available_for_board = board_mask & is_available;

        // no moves are available for the board, return the whole grid as it is
        if is_available_for_board.is_empty() {
            is_available
        } else {
            is_available_for_board
        }
    }

    #[must_use]
    /// # Returns
    /// - new node state with move applied (and board bits won if board was won)
    /// - true if the active player won using this move
    fn apply_move(&self, board_col_major_idx: u8) -> (NodeState, bool) {
        let mut child_state = *self;
        let player = self.active_player();

        let board_idx = board_col_major_idx / 9;

        let new_meta: u32 = ((player.other() as u32) << Self::PLAYER_OFFSET_IN_META)
            | (board_col_major_idx % 9) as u32;

        child_state.bits[player as usize] |= 0b1 << board_col_major_idx;
        child_state.bits[player as usize] |= (new_meta as u128) << Self::META_OFFSET;

        let has_won = child_state.get_player_board(player, board_idx).has_won();
        if has_won {
            child_state.bits[player as usize] = 0b1_1111_1111 << board_idx;
        }
        (child_state, has_won)
    }
}

/// NOTE: Node::default() is not a valid node and more of a placeholder
#[derive(Debug, Clone, Copy, Default)]
struct Node {
    game_state: NodeState,
    visits: u32,
    score: MonteCarloScore,
    /// first child node at `first_edge + 1`
    first_edge: NodeIdx,
    child_count: u8, // <= N_CELLS_NESTED
}

#[derive(Debug, Clone, Copy, Default)]
struct Edge {
    /// nonzero because arriving back at an empty board would make no sense
    /// (also allows niche optimization with Option<NonZero>)
    child_node: Option<NonZero<NodeIdx>>,
    move_: u8,
}

struct Tree {
    root: usize,
    // TODO PERF: maybe try to get this automatically promoted to a huge page by alignment
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    // TODO PERF: std lib hash function is probably sub optimal because of hashDoS mitigations
    lookup_without_root: HashMap<NodeState, NodeIdx>,
    edge_selection_buf: [NodeIdx; consts::N_CELLS_NESTED as usize],
}

impl Tree {
    const INITIAL_N_NODES: usize = 500_000;
    /// pulled straight out of where the sun dont shine
    const GUESSTIMATE_AVG_CHILDREN: usize = 30;

    fn new() -> Self {
        let nodes = Vec::with_capacity(Self::INITIAL_N_NODES);
        let edges = Vec::with_capacity(Self::INITIAL_N_NODES * Self::GUESSTIMATE_AVG_CHILDREN);

        let lookup_without_root = HashMap::with_capacity(Self::INITIAL_N_NODES);

        let mut this = Self {
            root: 0,
            nodes,
            edges,
            lookup_without_root,
            edge_selection_buf: [0; consts::N_CELLS_NESTED as usize],
        };

        this.insert_root_node();

        this
    }

    fn insert_root_node(&mut self) -> NodeIdx {
        let idx = self.nodes.len() as u32;

        let node_state = NodeState::empty();

        let available_children = node_state.available_in_board_or_fallback();
        let child_count = bitmagic::count_ones(available_children.get()) as u8;

        let first_edge = self.edges.len() as u32;
        // TODO PERF check how well this optimizes (should essentially just advance the len) since the vec is zeroed anyways
        self.edges
            .extend(iter::repeat_n(Edge::default(), child_count as usize));

        self.nodes.push(Node {
            game_state: node_state,
            visits: 0,
            score: 0,
            child_count,
            first_edge,
        });

        idx
        // no need to add this to the lookup, empty state can not be reused as its impossible
        // to clear occupied cells
    }

    fn get_or_insert_node(&mut self, previous_state: NodeState, move_: u8) -> NodeIdx {
        let (new_node_state, has_won) = previous_state.apply_move(move_);

        match self.lookup_without_root.entry(new_node_state) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                let idx = self.nodes.len() as u32;

                let (score, child_count) = if has_won {
                    // games where someone won have no children
                    (1, 0)
                } else {
                    let available_children = new_node_state.available_in_board_or_fallback();
                    let child_count = bitmagic::count_ones(available_children.get()) as u8;
                    (0, child_count)
                };

                let first_edge = self.edges.len() as u32;
                // TODO PERF check how well this optimizes (should essentially just advance the len) since the vec is zeroed anyways
                self.edges
                    .extend(iter::repeat_n(Edge::default(), child_count as usize));

                self.nodes.push(Node {
                    game_state: previous_state,
                    visits: 0,
                    score,
                    child_count,
                    first_edge,
                });

                vacant_entry.insert(idx);
                idx
            }
        }
    }

    fn get(&self, idx: NodeIdx) -> &Node {
        &self.nodes[idx as usize]
    }

    fn expand(&mut self, parent_node_idx: NodeIdx) -> MonteCarloScore {
        let parent_node = &mut self.nodes[parent_node_idx as usize];
        parent_node.visits += 1;

        // terminal leaf node
        if parent_node.child_count == 0 {
            return parent_node.score;
        }

        let edge_offset = parent_node.first_edge;

        let edges = &self.edges
            [edge_offset as usize..(edge_offset as usize + parent_node.child_count as usize)];
        // NOTE PERF: this could be maybe optimized by storing a u128 per node for unvisited
        // children
        // all unvisited edges have an infite UCB so they are all the max and we have to choose one
        // randomly
        let mut unvisited_edge_counter = 0;
        for (relative_edge_idx, _) in edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.child_node.is_none())
        {
            self.edge_selection_buf[unvisited_edge_counter] = relative_edge_idx as NodeIdx;
            unvisited_edge_counter += 1;
        }

        let child_to_visit = if unvisited_edge_counter != 0 {
            let rand_idx = rand::random_range(0..unvisited_edge_counter);
            let rand_unvisited_edge_relative_idx = self.edge_selection_buf[rand_idx];
            let move_ = bitmagic::index_of_nth_setbit(
                parent_node
                    .game_state
                    .available_in_board_or_fallback()
                    .get(),
                rand_unvisited_edge_relative_idx,
            ) as u8;
            let current_state = parent_node.game_state;
            let child_node_idx = self.get_or_insert_node(current_state, move_);

            // NOTE: this should never be zero, a move can not possibly result in the first/empty
            // node
            debug_assert_ne!(child_node_idx, 0);
            let edge_absolute_idx = (edge_offset + rand_unvisited_edge_relative_idx) as usize;
            // NOTE not-resuing local variable edges because it conflicts with the mutable borrow
            // of inserting a node
            self.edges[edge_absolute_idx].child_node = NonZero::new(child_node_idx);
            self.edges[edge_absolute_idx].move_ = move_;

            child_node_idx
        } else {
            let parent_visits_ln = (parent_node.visits as UCBScore).ln();
            let (mut max_ucb, mut max_ucb_node) = (0.0, 0);
            for edge in edges {
                // safety: if any child node is unvisited the code path above this for loop returns early
                let child_node_idx = unsafe { edge.child_node.unwrap_unchecked() };
                let child = &self.nodes[child_node_idx.get() as usize];
                let child_ucb = upper_confidence_bound(parent_visits_ln, child);
                if child_ucb > max_ucb {
                    max_ucb = child_ucb;
                    max_ucb_node = child_node_idx.get();
                }
            }
            max_ucb_node
        };

        let child_score = self.expand(child_to_visit);
        let parent_node = &mut self.nodes[parent_node_idx as usize];
        parent_node.score -= child_score;
        parent_node.score
    }
}

type UCBScore = f32;

// TODO: determine better value empirically
const EXPLORATION_C: UCBScore = core::f32::consts::SQRT_2;
/// https://en.wikipedia.org/wiki/Monte_Carlo_tree_search
fn upper_confidence_bound(parent_visits_ln: UCBScore, child: &Node) -> UCBScore {
    // [-1, 1]
    let exploitation = (child.score as UCBScore) / child.visits.max(1) as UCBScore;
    let exploration = EXPLORATION_C * UCBScore::sqrt(parent_visits_ln / child.visits as UCBScore);

    exploitation + exploration
}
