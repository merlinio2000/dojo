use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
};

use crate::{
    bitmagic,
    board::one_bit::OneBitBoard,
    consts::{self, N_CELLS},
    rng,
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
    /// upper 32 bits are reserved for meta data
    /// [127:119] = "super board" / board containing is_won for subboards for player 1
    ///
    /// # bits[1] (including meta)
    /// [80:0] = bitset indicating is_occupied for player 2
    /// upper 32 bits are reserved for meta data
    /// MSB(127) = 0 -> no move forced
    /// [99:96] = forced board idx (9 = no forced board)
    /// [112:112] = active player as u8, see [`Player`]
    /// [127:119] = "super board" / board containing is_won for subboards for player 2
    bits: [u128; 2],
}

impl NodeState {
    const META_OFFSET: u8 = (128 - 32);
    const PLAYER_OFFSET_IN_META: u8 = 16;
    const SUPER_BOARD_OFFSET_IN_META: u8 = 23;
    const META_MASK: u32 = u32::MAX;
    const fn empty() -> Self {
        Self {
            bits: [0, (NO_MOVE_FORCED as u128) << Self::META_OFFSET],
        }
    }
    const fn player1_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[0])
    }
    const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[1])
    }

    const fn meta_player(&self, player: Player) -> u32 {
        (self.bits[player as usize] >> Self::META_OFFSET) as u32
    }
    /// TODO better name
    const fn meta_player2(&self) -> u32 {
        self.meta_player(Player::Player2)
    }
    const fn forced_board(&self) -> u8 {
        self.meta_player2() as u8
    }
    const fn active_player(&self) -> Player {
        Player::from_is_player2((self.meta_player2() >> Self::PLAYER_OFFSET_IN_META) & 0b1 != 0)
    }

    const fn super_board_for_player(&self, player: Player) -> BoardState {
        self.meta_player(player) >> Self::SUPER_BOARD_OFFSET_IN_META
    }

    const fn get_player_board(&self, player: Player, board_idx: u8) -> OneBitBoard {
        OneBitBoard::new((self.bits[player as usize] >> (board_idx as u32 * N_CELLS)) as BoardState)
    }
    fn available_in_board_or_fallback(&self) -> BoardMajorBitset {
        // TODO PERF: this code has an unnecessary '& GRID_MASK', check the asm
        let is_occupied = self.player1_occupied() | self.player2_occupied();
        let is_available = !is_occupied;

        let forced_board = self.forced_board();

        if forced_board == NO_MOVE_FORCED {
            return is_available;
        }

        let board_mask = BoardMajorBitset::new_full_board(forced_board);
        let is_available_for_board = board_mask & is_available;

        // no moves are available for the board, return the whole grid as it is
        if is_available_for_board.is_empty() {
            is_available
        } else {
            is_available_for_board
        }
    }

    fn has_won(&self, player: Player) -> bool {
        OneBitBoard::new(self.super_board_for_player(player)).has_won()
    }

    #[must_use]
    /// Applys a move and correctly changes the metadata, active player and won boards
    /// # Returns
    /// - new node state with move applied (and board bits won if board was won)
    /// - true if the active player won using this move
    fn apply_move(&self, board_col_major_idx: u8) -> (NodeState, bool) {
        let mut child_state = *self;
        let player = self.active_player();

        let board_idx = board_col_major_idx / consts::N_CELLS as u8;

        child_state.bits[player as usize] |= 0b1 << board_col_major_idx;

        let has_won_subboard = child_state.get_player_board(player, board_idx).has_won();
        let new_general_meta: u32 = ((player.other() as u32) << Self::PLAYER_OFFSET_IN_META)
            | (board_col_major_idx % consts::N_CELLS as u8) as u32;

        if has_won_subboard {
            // block all cells in that board (simpler logic for available moves)
            child_state.bits[player as usize] |= 0b1_1111_1111 << board_idx;
            // track wins in super board (specific to each player, not in general meta)
            child_state.bits[player as usize] |=
                1 << (Self::META_OFFSET + Self::SUPER_BOARD_OFFSET_IN_META + board_idx);
        }

        // clear meta bits before setting
        child_state.bits[Player::Player2 as usize] &=
            !((Self::META_MASK as u128) << Self::META_OFFSET);
        child_state.bits[Player::Player2 as usize] |=
            (new_general_meta as u128) << Self::META_OFFSET;
        let won_game = if has_won_subboard {
            child_state.has_won(player)
        } else {
            false
        };

        (child_state, won_game)
    }

    /// # Returns
    /// - -1 if the not initially active player wins
    /// - 0 for a draw
    /// - 1 if the initally active player wins
    fn simulate_random(self) -> MonteCarloScore {
        let (mut game, mut has_won) = (self, false);
        let inital_player = game.active_player();
        let mut available_moves = game.available_in_board_or_fallback();
        debug_assert!(
            !available_moves.is_empty(),
            "can not simulate from a terminal state"
        );

        while !(has_won || available_moves.is_empty()) {
            let n_moves = bitmagic::count_ones(available_moves.get()) as u8;
            let rand_nth_setbit = rng::rand_in_move_range_exclusive(n_moves);
            let rand_move =
                bitmagic::index_of_nth_setbit(available_moves.get(), rand_nth_setbit) as u8;
            (game, has_won) = game.apply_move(rand_move);

            available_moves = game.available_in_board_or_fallback();
        }

        if has_won {
            let winner = game.active_player().other();
            // branchless score
            // lost: -1 + 0*2 = -1
            // won: -1 + 1*2 = 1
            -1 + ((winner == inital_player) as i32 * 2)
        } else {
            0
        }
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

pub struct Tree {
    root: NodeIdx,
    // TODO PERF: maybe try to get this automatically promoted to a huge page by alignment
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    // TODO PERF: std lib hash function is probably sub optimal because of hashDoS mitigations
    lookup_without_root: HashMap<NodeState, NodeIdx>,
    edge_selection_buf: [NodeIdx; consts::N_CELLS_NESTED as usize],
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

impl Tree {
    const INITIAL_N_NODES: usize = 500_000;
    /// pulled straight out of where the sun dont shine
    const GUESSTIMATE_AVG_CHILDREN: usize = 30;

    pub fn new() -> Self {
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
                    game_state: new_node_state,
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

    /// should only be called if root is not at a terminal state
    pub fn search(&mut self) -> u8 {
        for _i in 0..300_000 {
            let _score_from_leaf = self.expand(self.root);
        }
        let root_node = &self.nodes[self.root as usize];
        self.edges[root_node.first_edge as usize
            ..root_node.first_edge as usize + root_node.child_count as usize]
            .iter()
            .max_by_key(|edge| {
                edge.child_node
                    .map_or(0, |child_idx| self.nodes[child_idx.get() as usize].visits)
            })
            .expect("can not search on terminal node")
            .move_
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

        if unvisited_edge_counter != 0 {
            let rand_idx = rng::rand_in_move_range_exclusive(unvisited_edge_counter as u8);
            let rand_unvisited_edge_relative_idx = self.edge_selection_buf[rand_idx as usize];
            let move_ = bitmagic::index_of_nth_setbit(
                parent_node
                    .game_state
                    .available_in_board_or_fallback()
                    .get(),
                rand_unvisited_edge_relative_idx as u8,
            ) as u8;
            let current_state = parent_node.game_state;
            let child_node_idx = self.get_or_insert_node(current_state, move_);

            // NOTE: this should never be zero, a move can not possibly result in the first/empty
            // node as this would mean "un-setting" cells
            debug_assert_ne!(child_node_idx, 0);
            let edge_absolute_idx = (edge_offset + rand_unvisited_edge_relative_idx) as usize;
            // NOTE not-resuing local variable edges because it conflicts with the mutable borrow
            // of inserting a node
            self.edges[edge_absolute_idx].child_node = NonZero::new(child_node_idx);
            self.edges[edge_absolute_idx].move_ = move_;

            let new_child_node = &mut self.nodes[child_node_idx as usize];
            new_child_node.score = new_child_node.game_state.simulate_random();
            new_child_node.visits += 1;

            new_child_node.score
        } else {
            let parent_visits_ln = (parent_node.visits as UCBScore).ln();
            let (mut max_ucb, mut max_ucb_node) = (f32::MIN, 0);

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

            // negate because of negamax (a win for a child is a loss for us)
            let score_delta = -self.expand(max_ucb_node);
            let parent_node = &mut self.nodes[parent_node_idx as usize];
            parent_node.score += score_delta;
            score_delta
        }
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

#[cfg(test)]
mod test {
    use crate::{consts, tree::Tree};

    #[test]
    fn search_works_on_root() {
        let mut tree = Tree::new();
        let chosen_move = tree.search();
        assert!((0..consts::N_CELLS_NESTED as u8).contains(&chosen_move));
    }

    #[test]
    fn expand_adds_node() {
        let mut tree = Tree::new();
        assert_eq!(tree.nodes.len(), 1);
        tree.expand(0);
        assert_eq!(tree.nodes.len(), 2);
        tree.expand(0);
        assert_eq!(tree.nodes.len(), 3);
    }

    #[test]
    fn expanded_nodes_are_plausible() {
        let mut tree = Tree::new();
        tree.expand(0);

        let root = &tree.nodes[0];
        assert_eq!(root.visits, 1);

        let root_children = &tree.edges
            [(root.first_edge as usize)..(root.first_edge as usize + root.child_count as usize)];
        let defined_root_children_nodes: Vec<_> = root_children
            .iter()
            .filter_map(|edge| edge.child_node)
            .collect();
        assert_eq!(defined_root_children_nodes.len(), 1);
        assert_eq!(
            tree.nodes[defined_root_children_nodes[0].get() as usize].visits,
            1
        );
    }
}
