use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
    sync::atomic::AtomicBool,
    time::Instant,
};

use crate::{
    bitmagic,
    consts::{self},
    rng,
    tree::node_state::NodeState,
};

mod node_state;
mod simulation;

type NodeIdx = u32;

type MonteCarloScore = i32;
const NO_MOVE_FORCED: u8 = 9;

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
    const INITIAL_N_NODES: usize = 5_000_000;
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

    /// changes the root by choosing the child with the corresponding move
    /// # Returns
    /// the index of the new root
    pub fn apply_explored_move(&mut self, move_: u8) -> NodeIdx {
        let root_node = self.nodes[self.root as usize];
        assert_ne!(root_node.child_count, 0);

        let edges = &self.edges[root_node.first_edge as usize
            ..(root_node.first_edge as usize + root_node.child_count as usize)];
        let target_edge = edges
            .iter()
            .find(|edge| edge.move_ == move_)
            .expect("move to apply must have been explored previously");
        self.root = target_edge
            .child_node
            .expect("explored edges must have their child_node set")
            .get();
        self.root
    }

    pub fn apply_maybe_explored_move(&mut self, move_: u8) -> NodeIdx {
        let root_node = self.nodes[self.root as usize];
        assert_ne!(root_node.child_count, 0);

        let edges = &self.edges[root_node.first_edge as usize
            ..(root_node.first_edge as usize + root_node.child_count as usize)];
        let target_edge_opt = edges.iter().find(|edge| edge.move_ == move_);
        if let Some(target_edge) = target_edge_opt {
            self.root = target_edge
                .child_node
                .expect("explored edges must have their child_node set")
                .get();
        } else {
            let available_moves: u128 = root_node.game_state.available_in_board_or_fallback().get();
            // remove all moves including and above the selected, the index of the move is the
            // amount of less significant 1
            let available_moves = available_moves << (128 - move_);
            let move_edge_idx = bitmagic::count_ones_u128(available_moves);
            debug_assert_eq!(edges[move_edge_idx as usize].child_node, None);
            debug_assert_eq!(edges[move_edge_idx as usize].move_, 0);

            let child_node = self.get_or_insert_node(root_node.game_state, move_);

            let edge_for_move = &mut self.edges[(root_node.first_edge + move_edge_idx) as usize];
            edge_for_move.move_ = move_;
            edge_for_move.child_node = NonZero::new(child_node);
            self.root = child_node;
        }
        self.root
    }

    fn insert_root_node(&mut self) -> NodeIdx {
        let idx = self.nodes.len() as u32;

        let node_state = NodeState::empty();

        let available_children = node_state.available_in_board_or_fallback();
        let child_count = bitmagic::count_ones_u128(available_children.get()) as u8;

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
                    let child_count = bitmagic::count_ones_u128(available_children.get()) as u8;
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
    pub fn search(&mut self) {
        self.search_n(50_000);
    }
    pub fn search_n(&mut self, n: usize) {
        for _i in 0..n {
            let _score_from_leaf = self.expand(self.root);
        }
    }
    pub fn search_flag(&mut self, keep_going: AtomicBool) {
        // TOOD: i think this ordering is fine but don't know for sure
        while keep_going.load(std::sync::atomic::Ordering::Acquire) {
            let _score_from_leaf = self.expand(self.root);
        }
    }
    pub fn search_until(&mut self, instant: Instant) {
        while instant > Instant::now() {
            let _score_from_leaf = self.expand(self.root);
        }
    }

    pub fn best_explored_move(&self) -> u8 {
        let root_node = &self.nodes[self.root as usize];
        self.edges[root_node.first_edge as usize
            ..root_node.first_edge as usize + root_node.child_count as usize]
            .iter()
            .filter_map(|edge| edge.child_node.map(|child_node| (edge, child_node)))
            .max_by_key(|(_, child_node)| self.nodes[child_node.get() as usize].visits)
            .expect("at least one child must have been explored")
            .0
            .move_
    }

    fn expand(&mut self, parent_node_idx: NodeIdx) -> MonteCarloScore {
        let parent_node = &mut self.nodes[parent_node_idx as usize];
        parent_node.visits += 1;

        // terminal leaf node
        if parent_node.child_count == 0 {
            return parent_node.score;
        }

        let edge_offset = parent_node.first_edge as usize;

        let edges = &self.edges[edge_offset..(edge_offset + parent_node.child_count as usize)];
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
            let edge_absolute_idx = edge_offset + rand_unvisited_edge_relative_idx as usize;
            // NOTE not-resuing local variable edges because it conflicts with the mutable borrow
            // of inserting a node
            self.edges[edge_absolute_idx].child_node = NonZero::new(child_node_idx);
            self.edges[edge_absolute_idx].move_ = move_;

            let new_child_node = &mut self.nodes[child_node_idx as usize];
            new_child_node.score += new_child_node
                .game_state
                .into_simulation()
                .simulate_random();
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
        tree.search();
        let chosen_move = tree.best_explored_move();
        assert!((0..consts::N_CELLS_NESTED as u8).contains(&chosen_move));
    }

    #[test]
    fn children_are_explored_first() {
        let mut tree = Tree::new();
        let root = &tree.nodes[tree.root as usize];
        assert_eq!(root.child_count, consts::N_CELLS_NESTED as u8);
        assert_eq!(tree.nodes.len(), 1);
        assert_eq!(tree.edges.len(), consts::N_CELLS_NESTED as usize);
        tree.search_n(consts::N_CELLS_NESTED as usize);
        assert_eq!(tree.nodes.len(), 1 + consts::N_CELLS_NESTED as usize);

        let root = &tree.nodes[tree.root as usize];
        let edges = &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)];
        assert_eq!(edges.len(), consts::N_CELLS_NESTED as usize);

        for (i, edge) in edges.iter().enumerate() {
            assert_eq!(edge.move_, i as u8);
            match edge.child_node {
                None => panic!("child {i} must have been explored"),
                Some(child_node) => {
                    let child_node = &tree.nodes[child_node.get() as usize];
                    assert_eq!(child_node.visits, 1);
                }
            }
        }

        let move_ = 0;
        tree.apply_explored_move(move_);
        // we placed in the top left corner of the top left board, forcing top left board again
        // with 8 possible moves
        tree.search_n(consts::N_CELLS as usize - 1);
        let root = &tree.nodes[tree.root as usize];
        assert_eq!(root.visits, consts::N_CELLS);
        let edges = &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)];
        assert_eq!(edges.len(), consts::N_CELLS as usize - 1);

        for (i, edge) in edges.iter().enumerate() {
            // first available move is move 1
            let i = i + 1;
            assert_eq!(edge.move_, i as u8);
            match edge.child_node {
                None => panic!("move {i} must have been explored"),
                Some(child_node) => {
                    let child_node = &tree.nodes[child_node.get() as usize];
                    assert_eq!(child_node.visits, 1);
                }
            }
        }
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

    #[test]
    fn apply_move() {
        let mut tree = Tree::new();
        tree.expand(tree.root);
        let move_to_apply = tree.best_explored_move();
        let new_root = tree.apply_explored_move(move_to_apply);
        assert_eq!(new_root, 1);
        assert_eq!(tree.root, new_root);

        tree.expand(new_root);
        let new_root_node = &tree.nodes[tree.root as usize];
        let edges_of_new_root = &tree.edges[(new_root_node.first_edge as usize)
            ..(new_root_node.first_edge as usize + new_root_node.child_count as usize)];

        let defined_children_cnt = edges_of_new_root
            .iter()
            .filter(|edge| edge.child_node.is_some())
            .count();
        assert_eq!(defined_children_cnt, 1);
    }
}
