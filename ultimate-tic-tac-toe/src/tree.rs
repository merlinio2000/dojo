use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
};

use crate::{
    bitmagic, consts,
    types::{Index, Move, Player},
};

type NodeIdx = u32;

type MonteCarloScore = i16;
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
    bits: [u128; 2],
}

impl NodeState {
    const META_OFFSET_PLAYER2: usize = (128 - 32);
    const fn empty() -> Self {
        Self {
            bits: [0, (NO_MOVE_FORCED as u128) << Self::META_OFFSET_PLAYER2],
        }
    }
    const fn player1_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset(self.bits[0])
    }
    const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset(self.bits[1] & BoardMajorBitset::GRID_MASK)
    }
    const fn meta(&self) -> u32 {
        (self.bits[1] >> Self::META_OFFSET_PLAYER2) as u32
    }
    const fn forced_move(&self) -> u8 {
        (self.meta() & 0b1111) as u8
    }
    fn available_in_board_or_fallback(&self) -> u128 {
        let is_occupied = self.player1_occupied().0 | self.player2_occupied().0;
        let is_available = !is_occupied & BoardMajorBitset::GRID_MASK;

        let forced_move = self.forced_move();

        if forced_move == NO_MOVE_FORCED {
            return is_available;
        }

        let board_mask = BoardMajorBitset::BOARD_FULL_MASK << (forced_move * consts::N_CELLS as u8);
        let is_available_for_board = board_mask & is_available;

        // no moves are available for the board, return the whole grid as it is
        if is_available_for_board == 0 {
            is_available
        } else {
            is_available_for_board
        }
    }

    #[must_use]
    fn apply_move(&self, board_col_major_idx: u8, player: Player) -> NodeState {
        let mut child_state = *self;
        child_state.bits[player as usize] |= 0b1 << board_col_major_idx;
        child_state
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
struct BoardMajorBitset(u128);

impl BoardMajorBitset {
    const BOARD_FULL_MASK: u128 = 0b1_1111_1111;
    const BITS: u32 = consts::N_CELLS * consts::N_CELLS;
    const GRID_MASK: u128 = 2u128.pow(Self::BITS) - 1;
    fn fill_board(&mut self, board_idx: Index) {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 |= board_full_mask;
    }
    fn is_board_full(&self, board_idx: Index) -> bool {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 & board_full_mask == board_full_mask
    }
    fn apply_move(&mut self, move_: u8) {
        self.0 |= 1 << move_;
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
    child_count: u8, // <= 81
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
    lookup: HashMap<NodeState, NodeIdx>,
}

impl Tree {
    const INITIAL_N_NODES: usize = 500_000;
    // pulled straight out of where the sun dont shine
    const GUESSTIMATE_AVG_CHILDREN: usize = 30;

    fn get_or_insert_node(&mut self, node_state: NodeState) -> NodeIdx {
        match self.lookup.entry(node_state) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                debug_assert_eq!(self.edges.len(), self.nodes.len());

                let idx = self.nodes.len() as u32;

                let available_children = node_state.available_in_board_or_fallback();
                let child_count = bitmagic::count_ones(available_children) as u8;

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

                vacant_entry.insert(idx);
                idx
            }
        }
    }

    fn new() -> Self {
        let nodes = Vec::with_capacity(Self::INITIAL_N_NODES);
        let edges = Vec::with_capacity(Self::INITIAL_N_NODES * Self::GUESSTIMATE_AVG_CHILDREN);

        let lookup = HashMap::with_capacity(Self::INITIAL_N_NODES);

        let mut this = Self {
            root: 0,
            nodes,
            edges,
            lookup,
        };

        this.get_or_insert_node(NodeState::empty());

        this
    }
    fn get(&self, idx: NodeIdx) -> &Node {
        &self.nodes[idx as usize]
    }

    fn expand(&mut self, node_idx: NodeIdx) {
        let node = &mut self.nodes[node_idx as usize];

        node.visits += 1;

        let parent_visits_ln = (node.visits as UCBScore).ln();
        let edges = &self.edges
            [node.first_edge as usize..(node.first_edge as usize + node.child_count as usize)];
        let (mut max_ucb, mut max_ucb_edge) = (0.0, 0);
        for (edge_idx_for_node, edge) in edges.iter().enumerate() {
            match edge.child_node {
                None => {
                    max_ucb = UCBScore::MAX;
                    max_ucb_edge = edge_idx_for_node;
                    break;
                }
                Some(child_node) => {
                    let child = &self.nodes[child_node.get() as usize];
                    let child_ucb = upper_confidence_bound(parent_visits_ln, child);
                    if child_ucb > max_ucb {
                        max_ucb = child_ucb;
                        max_ucb_edge = edge_idx_for_node;
                    }
                }
            }
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
