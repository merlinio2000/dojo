use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
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
            new_child_node.score = new_child_node
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
