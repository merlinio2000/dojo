use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    num::NonZero,
    sync::atomic::AtomicBool,
    time::Instant,
};

use crate::{
    bitmagic, consts, rng,
    tree::node_state::NodeState,
    types::{MonteCarloScore, PLAYER1_U8, PLAYER2_U8, Player, PlayerU8},
};

mod node_state;
mod simulation;

type NodeIdx = u32;

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

pub struct TreeForPlayer<const SCORE_IN_FAVOR_OF: PlayerU8> {
    root: NodeIdx,
    // TODO PERF: maybe try to get this automatically promoted to a huge page by alignment
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    // TODO PERF: std lib hash function is probably sub optimal because of hashDoS mitigations
    lookup_without_root: HashMap<NodeState, NodeIdx>,
    edge_selection_buf: [NodeIdx; consts::N_CELLS_NESTED as usize],
}

pub type TreePlayer1 = TreeForPlayer<PLAYER1_U8>;
pub type TreePlayer2 = TreeForPlayer<PLAYER2_U8>;

impl Default for TreePlayer1 {
    fn default() -> TreePlayer1 {
        Self::new()
    }
}

impl TreePlayer1 {
    fn new_from_node(root: NodeState) -> Self {
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
        this.insert_root_node(root);
        this
    }
    pub fn new() -> Self {
        Self::new_from_node(NodeState::empty())
    }
    fn insert_root_node(&mut self, node_state: NodeState) -> NodeIdx {
        debug_assert_eq!(self.nodes.len(), 0);
        debug_assert_eq!(self.edges.len(), 0);
        let idx = 0;

        let available_children = node_state.available_in_board_or_fallback();
        let child_count = bitmagic::count_ones_u128(available_children.get()) as u8;

        let first_edge = 0;
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
}
impl TreePlayer2 {
    pub fn new(move_by_player1: u8) -> Self {
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

        this.insert_root_node_player2(move_by_player1);

        this
    }
    fn insert_root_node_player2(&mut self, move_by_player1: u8) -> NodeIdx {
        debug_assert_eq!(self.nodes.len(), 0);
        debug_assert_eq!(self.edges.len(), 0);
        let idx = 0;
        let node_state = NodeState::empty().apply_move(move_by_player1).0;

        let available_children = node_state.available_in_board_or_fallback();
        let child_count = bitmagic::count_ones_u128(available_children.get()) as u8;

        let first_edge = 0;
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
}

impl<const SCORE_IN_FAVOR_OF: PlayerU8> TreeForPlayer<SCORE_IN_FAVOR_OF> {
    const INITIAL_N_NODES: usize = 5_000_000;
    /// pulled straight out of where the sun dont shine
    const GUESSTIMATE_AVG_CHILDREN: usize = 30;

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
        {
            let merbug_root_node = self.nodes[self.root as usize];
            let available_mask = merbug_root_node
                .game_state
                .available_in_board_or_fallback()
                .get();
            let forced = merbug_root_node.game_state.forced_board();
            let forced_idx_min = forced * 9;
            let forced_idx_max_incl = forced_idx_min + 8;
            eprintln!(
                "MERBUG forced/available: {forced} e [{forced_idx_min}, {forced_idx_max_incl}]\n{available_mask:081b}"
            );
            let wins_p1 = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|board_idx| {
                let board = merbug_root_node
                    .game_state
                    .get_player_board(Player::Player1, board_idx);
                (format!("{:09b}", board.get()), board.has_won())
            });
            let wins_p2 = [0, 1, 2, 3, 4, 5, 6, 7, 8].map(|board_idx| {
                let board = merbug_root_node
                    .game_state
                    .get_player_board(Player::Player2, board_idx);
                (format!("{:09b}", board.get()), board.has_won())
            });
            eprintln!(
                "MERBUG wins p1: {:09b}\n{wins_p1:?}",
                merbug_root_node
                    .game_state
                    .super_board_for_player(Player::Player1)
            );
            eprintln!(
                "MERBUG wins p2: {:09b}\n{wins_p2:?}",
                merbug_root_node
                    .game_state
                    .super_board_for_player(Player::Player2)
            );
        }
        self.root
    }

    fn get_or_insert_node(&mut self, previous_state: NodeState, move_: u8) -> NodeIdx {
        let (new_node_state, child_count, _) = previous_state.apply_move(move_);
        #[cfg(debug_assertions)]
        {
            use crate::tree::node_state::NodeScore;

            let n_children = previous_state
                .available_in_board_or_fallback()
                .get()
                .count_ones();
            if n_children == 1
                && new_node_state.node_score_favoring_previous_player() != NodeScore::Indeterminate
            {
                eprintln!(
                    "MERBUG supers previous\n{:09b}\n{:09b}",
                    previous_state.super_board_for_player(Player::Player1),
                    previous_state.super_board_for_player(Player::Player2)
                );
                eprintln!(
                    "MERBUG supers new\n{:09b}\n{:09b}",
                    new_node_state.super_board_for_player(Player::Player1),
                    new_node_state.super_board_for_player(Player::Player2)
                );
                debug_assert_eq!(
                    previous_state.into_simulation().simulate_random(),
                    new_node_state
                        .node_score_favoring_previous_player()
                        .as_monte_carlo_score(),
                    "simulation of previous state with only one move mismatches score of new node with move {move_}",
                );
            }
        }

        match self.lookup_without_root.entry(new_node_state) {
            Entry::Occupied(occupied_entry) => *occupied_entry.get(),
            Entry::Vacant(vacant_entry) => {
                let idx = self.nodes.len() as u32;

                // yes this is unnecessary for terminal nodes but it is preferable to not branch
                // as it doesn't cost much and the vast majority of nodes are non-terminal
                let first_edge = self.edges.len() as u32;
                // TODO PERF check how well this optimizes (should essentially just advance the len) since the vec is zeroed anyways
                self.edges
                    .extend(iter::repeat_n(Edge::default(), child_count as usize));

                self.nodes.push(Node {
                    game_state: new_node_state,
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
            let score_delta = parent_node
                .game_state
                .node_score_favoring_previous_player()
                .as_monte_carlo_score();
            parent_node.score += score_delta;
            return score_delta;
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

            let child_node = &mut self.nodes[child_node_idx as usize];
            child_node.visits += 1;
            // NOTE: += intentional because the node might be re-used
            let score_delta = child_node.game_state.into_simulation().simulate_random();
            child_node.score += score_delta;

            let parent_node = &mut self.nodes[parent_node_idx as usize];
            // negamax: child.score favors the child's previous player (the parent's active player),
            // but parent.score favors the parent's previous player (the opponent).
            // So we subtract: a child win (positive) is a parent loss (negative contribution).
            let score_delta_for_parent = -score_delta;
            parent_node.score += score_delta_for_parent;

            score_delta_for_parent
        } else {
            let parent_visits_ln = (parent_node.visits as UCBScore).ln();
            let (mut max_ucb, mut max_ucb_node) = (f32::MIN, 0);

            for edge in edges {
                // safety: if any child node is unvisited the code path above this for loop returns early
                let child_node_idx = unsafe { edge.child_node.unwrap_unchecked() };
                let child = &self.nodes[child_node_idx.get() as usize];
                let child_ucb = upper_confidence_bound(parent_visits_ln, child.score, child.visits);
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
fn upper_confidence_bound(
    parent_visits_ln: UCBScore,
    child_score: MonteCarloScore,
    child_visits: u32,
) -> UCBScore {
    // [-1, 1]
    let exploitation = (child_score as UCBScore) / child_visits.max(1) as UCBScore;
    let exploration = EXPLORATION_C * UCBScore::sqrt(parent_visits_ln / child_visits as UCBScore);

    exploitation + exploration
}

#[cfg(test)]
mod test {

    use itertools::Itertools;
    use rand::seq::IndexedRandom;

    use crate::{
        board::one_bit::OneBitBoard,
        consts,
        tree::{NodeState, TreePlayer1, upper_confidence_bound},
        types::Player,
    };

    // ==================== DIAGNOSTIC TESTS FOR MCTS ISSUES ====================

    /// Test 1: Verify UCB calculation handles score convention correctly
    /// Scores favor the previous player (who made the move to reach this node).
    /// Higher score = better for parent, so UCB should select higher scores.
    #[test]
    fn ucb_score_perspective_is_consistent() {
        // Parent has 100 visits
        let parent_visits_ln = (100.0_f32).ln();

        // Score +8 means parent won 80% of simulations from this move (good for parent)
        // Score -8 means parent lost 80% of simulations from this move (bad for parent)

        let ucb_parent_winning = upper_confidence_bound(parent_visits_ln, 8, 10);
        let ucb_parent_losing = upper_confidence_bound(parent_visits_ln, -8, 10);

        println!(
            "UCB (parent winning): {ucb_parent_winning}, UCB (parent losing): {ucb_parent_losing}"
        );

        assert!(
            ucb_parent_winning > ucb_parent_losing,
            "UCB should prefer moves where the parent wins"
        );
    }

    /// Test 2: Verify score backpropagation signs are correct
    #[test]
    fn backpropagation_signs_are_consistent() {
        let mut tree = TreePlayer1::new();
        tree.search_n(100);

        let root = &tree.nodes[tree.root as usize];
        let root_children_scores: Vec<_> = tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
            .iter()
            .filter_map(|e| e.child_node)
            .map(|idx| {
                let child = &tree.nodes[idx.get() as usize];
                (
                    child.visits,
                    child.score,
                    child.score as f32 / child.visits.max(1) as f32,
                )
            })
            .collect();

        println!("Root visits: {}, Root score: {}", root.visits, root.score);
        println!("Children (visits, score, avg): {:?}", root_children_scores);

        let expected_parent_score: i32 = root_children_scores.iter().map(|(_, s, _)| -s).sum();
        println!("Expected parent score (negated sum of children): {expected_parent_score}");
        println!("Actual parent score: {}", root.score);
    }

    /// Test 3: Verify simulation returns correct score perspective
    #[test]
    fn simulation_score_perspective() {
        let empty_sim = NodeState::empty().into_simulation();
        // active player = player1 -> favored(previous) player = player2

        let mut p1_wins = 0;
        let mut p2_wins = 0;
        let mut draws = 0;

        for _ in 0..1000 {
            let sim_clone = empty_sim;
            let result = sim_clone.simulate_random();
            match result {
                1 => p2_wins += 1,
                -1 => p1_wins += 1,
                0 => draws += 1,
                _ => panic!("Invalid simulation result: {result}"),
            }
        }

        println!("From empty board with P1 to move first in simulation:");
        println!("P1 wins: {p1_wins}, P2 wins: {p2_wins}, Draws: {draws}");
    }

    /// Test 4: Verify MCTS strongly prefers immediate winning moves over all others
    /// This tests that when there's a winning move available, MCTS explores it heavily
    #[test]
    fn mcts_heavily_explores_winning_move() {
        // Setup: Player 1 has won boards 0 and 1, needs cell 0 in board 2 to win the game
        let p1 = [
            OneBitBoard::new(0b111), // Won board 0
            OneBitBoard::new(0b111), // Won board 1
            OneBitBoard::new(0b110), // Needs cell 0 to win board 2 (and game)
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        let p2 = [
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];

        let node_state = NodeState::from_boards([p1, p2], 2, Player::Player1);
        let mut tree = TreePlayer1::new_from_node(node_state);

        tree.search_n(500);

        let root = &tree.nodes[tree.root as usize];
        let edges = &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)];

        // Find the winning move (move 18 = cell 0 in board 2)
        let winning_edge = edges
            .iter()
            .find(|e| e.move_ == 18)
            .expect("winning move should exist");
        let winning_node = &tree.nodes[winning_edge.child_node.unwrap().get() as usize];

        // Find max visits among non-winning moves
        let max_other_visits = edges
            .iter()
            .filter(|e| e.move_ != 18)
            .filter_map(|e| e.child_node)
            .map(|idx| tree.nodes[idx.get() as usize].visits)
            .max()
            .unwrap_or(0);

        println!(
            "Winning move visits: {}, Best other move visits: {}",
            winning_node.visits, max_other_visits
        );

        // The winning move should have SIGNIFICANTLY more visits
        assert!(
            winning_node.visits > max_other_visits,
            "Winning move should be explored much more than others. Got {} vs {}",
            winning_node.visits,
            max_other_visits
        );

        assert_eq!(
            tree.best_explored_move(),
            18,
            "Best move should be the winning move"
        );
    }

    /// Test 5: Verify MCTS avoids moves that let opponent win immediately
    #[test]
    fn mcts_avoids_losing_moves() {
        // Create a situation where most moves let opponent win, but one blocks
        // Player 1's turn, opponent (P2) is about to win

        let p1 = [
            OneBitBoard::new(0b111), // Won board 0
            OneBitBoard::new(0b111), // Won board 1
            OneBitBoard::new(0b000), // Empty - can play here
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        // P2 is close to winning with boards 3,4 and needs board 5
        let p2 = [
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0b111), // Won board 3
            OneBitBoard::new(0b111), // Won board 4
            OneBitBoard::new(0b110), // Needs cell 0 to win board 5 (and game via diagonal 2,4,6... wait that's not a line)
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];

        // Player 1 forced to board 2, but let's test they make reasonable moves
        let node_state = NodeState::from_boards([p1, p2], 2, Player::Player1);
        let mut tree = TreePlayer1::new_from_node(node_state);

        tree.search_n(1000);

        let best_move = tree.best_explored_move();
        println!("Best move chosen: {}", best_move);

        // Just verify MCTS completes without panic and chooses a valid move
        assert!((18..=26).contains(&best_move), "Move should be in board 2");
    }

    /// Test 6: Verify scores are properly bounded
    #[test]
    fn scores_are_properly_normalized_in_ucb() {
        let mut tree = TreePlayer1::new();
        tree.search_n(1000);

        let root = &tree.nodes[tree.root as usize];

        for edge in &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
        {
            if let Some(child_idx) = edge.child_node {
                let child = &tree.nodes[child_idx.get() as usize];
                let avg_score = child.score as f32 / child.visits.max(1) as f32;

                assert!(
                    (-1.0..=1.0).contains(&avg_score),
                    "Average score {avg_score} is outside [-1, 1] range for node with {} visits and {} score",
                    child.visits,
                    child.score
                );
            }
        }
    }

    /// Test 7: Examine score distribution at different tree levels
    #[test]
    fn negamax_score_signs_alternate_correctly() {
        let mut tree = TreePlayer1::new();
        tree.search_n(2000);

        let root = &tree.nodes[tree.root as usize];
        println!("\n=== NEGAMAX SCORE ANALYSIS ===");
        println!(
            "Root: visits={}, score={}, avg={:.3}",
            root.visits,
            root.score,
            root.score as f32 / root.visits.max(1) as f32
        );

        let mut level1_info = Vec::new();
        for edge in &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
        {
            if let Some(child_idx) = edge.child_node {
                let child = &tree.nodes[child_idx.get() as usize];
                if child.visits > 10 {
                    level1_info.push((
                        edge.move_,
                        child.visits,
                        child.score,
                        child.score as f32 / child.visits as f32,
                    ));
                }
            }
        }

        level1_info.sort_by(|a, b| b.1.cmp(&a.1));
        println!("\nTop Level 1 children (move, visits, score, avg):");
        for (m, v, s, avg) in level1_info.iter().take(5) {
            println!("  Move {m}: visits={v}, score={s}, avg={avg:.3}");
        }
    }

    /// Test 8: Verify root score accumulation is consistent with child scores
    #[test]
    fn root_score_matches_negated_child_sum() {
        let mut tree = TreePlayer1::new();
        tree.search_n(500);

        let root = &tree.nodes[tree.root as usize];

        let child_score_sum: i32 = tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
            .iter()
            .filter_map(|e| e.child_node)
            .map(|idx| tree.nodes[idx.get() as usize].score)
            .sum();

        println!(
            "Root score: {}, Child sum: {}, Expected: {}",
            root.score, child_score_sum, -child_score_sum
        );
    }

    /// Test 9: Detailed trace of score flow to diagnose issues
    #[test]
    fn trace_score_flow() {
        // Create a near-terminal game where Player 1 can win in one move
        let p1 = [
            OneBitBoard::new(0b111), // Won board 0
            OneBitBoard::new(0b111), // Won board 1
            OneBitBoard::new(0b110), // Needs cell 0 to win
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        let p2 = [
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];

        let node_state = NodeState::from_boards([p1, p2], 2, Player::Player1);
        let mut tree = TreePlayer1::new_from_node(node_state);

        // Just do a few expansions and trace what happens
        println!("\n=== SCORE FLOW TRACE ===");
        println!(
            "Initial root: visits={}, score={}",
            tree.nodes[tree.root as usize].visits, tree.nodes[tree.root as usize].score
        );

        tree.expand(tree.root);
        let root = &tree.nodes[tree.root as usize];
        println!(
            "After 1 expand - Root: visits={}, score={}",
            root.visits, root.score
        );

        // Look at all children
        for edge in &tree.edges
            [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
        {
            if let Some(child_idx) = edge.child_node {
                let child = &tree.nodes[child_idx.get() as usize];
                let terminal = if child.child_count == 0 {
                    " [TERMINAL]"
                } else {
                    ""
                };
                println!(
                    "  Child (move {}): visits={}, score={}{}",
                    edge.move_, child.visits, child.score, terminal
                );
            }
        }

        // Do more expansions
        for i in 2..=10 {
            tree.expand(tree.root);
            let root = &tree.nodes[tree.root as usize];
            println!(
                "\nAfter {} expands - Root: visits={}, score={}",
                i, root.visits, root.score
            );

            for edge in &tree.edges
                [root.first_edge as usize..(root.first_edge as usize + root.child_count as usize)]
            {
                if let Some(child_idx) = edge.child_node {
                    let child = &tree.nodes[child_idx.get() as usize];
                    let terminal = if child.child_count == 0 {
                        " [TERMINAL WIN]"
                    } else {
                        ""
                    };
                    println!(
                        "  Move {}: visits={}, score={}{}",
                        edge.move_, child.visits, child.score, terminal
                    );
                }
            }
        }

        // The winning terminal node should have:
        // - score = visits (each visit adds +1 for Win)
        // The root should accumulate scores properly
    }

    // ==================== END DIAGNOSTIC TESTS ====================

    #[test]
    fn mcts_responds_to_opponent_threat() {
        let p1 = [
            OneBitBoard::new(0),
            OneBitBoard::new(0b111), // Won board 1
            OneBitBoard::new(0b111), // Won board 2
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        let p2 = [
            OneBitBoard::new(0b111), // Won board 0
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0b111), // Won board 3
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0b110), // Needs cell 0 to win board 6 and game
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];

        let node_state = NodeState::from_boards([p1, p2], 4, Player::Player1);
        for _ in 0..10 {
            let mut tree = TreePlayer1::new_from_node(node_state);

            tree.search_n(5000);

            let best_move = tree.best_explored_move();
            // move should not send to board 6
            assert_ne!(best_move % consts::N_CELLS, 6);
        }
    }

    #[test]
    fn search_works_on_root() {
        let mut tree = TreePlayer1::new();
        tree.search();
        let chosen_move = tree.best_explored_move();
        assert!((0..consts::N_CELLS_NESTED).contains(&chosen_move));
    }

    #[test]
    fn children_are_explored_first() {
        let mut tree = TreePlayer1::new();
        let root = &tree.nodes[tree.root as usize];
        assert_eq!(root.child_count, consts::N_CELLS_NESTED);
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
        assert_eq!(root.visits, consts::N_CELLS as u32);
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
        let mut tree = TreePlayer1::new();
        assert_eq!(tree.nodes.len(), 1);
        tree.expand(0);
        assert_eq!(tree.nodes.len(), 2);
        tree.expand(0);
        assert_eq!(tree.nodes.len(), 3);
    }

    #[test]
    fn expanded_nodes_are_plausible() {
        let mut tree = TreePlayer1::new();
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
        let mut tree = TreePlayer1::new();
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

    #[test]
    fn must_pick_obvious_winning_move() {
        let not_won_boards = (0..=OneBitBoard::new_full().get())
            .map(OneBitBoard::new)
            .filter(|board| !board.has_won())
            .collect_vec();

        let rand_not_won_board = || *not_won_boards.choose(&mut rand::rng()).unwrap();

        let rand_exclusive = |player1: [OneBitBoard; 9]| {
            player1.map(|p1_board| OneBitBoard::new(rand_not_won_board().get() & !p1_board.get()))
        };
        let p1 = [
            OneBitBoard::new_full(),
            OneBitBoard::new_full(),
            // only needs a move in cell 0 to win
            OneBitBoard::new(0b110),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        assert!({
            let mut b = p1[2];
            b.set_cell(0);
            b.has_won()
        });
        let mut p2 = rand_exclusive(p1);
        // ensure there are many moves to choose
        p2[2] = OneBitBoard::new(0);

        let node_state = NodeState::from_boards([p1, p2], 2, Player::Player1);

        let mut tree = TreePlayer1::new_from_node(node_state);

        tree.search_n(1_000);
        let root_node = &tree.nodes[tree.root as usize];
        let edges = &tree.edges[(root_node.first_edge as usize)
            ..(root_node.first_edge as usize + root_node.child_count as usize)];
        assert_eq!(edges.len(), 9 - 2);
        dbg!(edges);
        let children = edges
            .iter()
            .filter_map(|edge| {
                edge.child_node
                    .map(|child_idx| tree.nodes[child_idx.get() as usize])
            })
            .collect_vec();
        assert_eq!(children.len(), 9 - 2);
        dbg!(&children);
        let max_idx = children
            .iter()
            .position_max_by_key(|node| node.visits)
            .unwrap();
        assert_eq!(max_idx, 0);
        let max_child = children[max_idx];
        assert_eq!(tree.best_explored_move(), 2 * 9);
        assert_eq!(max_child.child_count, 0);
        let max_edge = edges[max_idx];
        assert_eq!(max_edge.move_, 2 * 9);
    }

    #[test]
    fn must_pick_obvious_winning_move_in_2() {
        let not_won_boards = (0..=OneBitBoard::new_full().get())
            .map(OneBitBoard::new)
            .filter(|board| !board.has_won())
            .collect_vec();

        let rand_not_won_board = || *not_won_boards.choose(&mut rand::rng()).unwrap();

        let rand_exclusive = |player1: [OneBitBoard; 9]| {
            player1.map(|p1_board| OneBitBoard::new(rand_not_won_board().get() & !p1_board.get()))
        };
        let mut p1 = [
            OneBitBoard::new_full(),
            OneBitBoard::new_full(),
            // only needs a move in cell 0 to win
            OneBitBoard::new(0b110),
            OneBitBoard::new(0),
            // the forced board, has to allow a move on 3 to be able to force the board
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
            OneBitBoard::new(0),
        ];
        assert!({
            let mut b = p1[2];
            b.set_cell(0);
            b.has_won()
        });
        let mut p2 = rand_exclusive(p1);
        // ensure there are many moves to choose
        p2[2] = OneBitBoard::new(0);
        p2[4] = OneBitBoard::new(0);

        p1[3] = OneBitBoard::new(0b010_101_010);
        p2[3] = OneBitBoard::new(0b101_010_001);
        // move that forces board 2 needs to be available
        assert_eq!(p2[3].get() & 0b100, 0);

        let node_state = NodeState::from_boards([p1, p2], 4, Player::Player1);

        let mut tree = TreePlayer1::new_from_node(node_state);

        tree.search_n(100_000);
        let root_node = &tree.nodes[tree.root as usize];
        let edges = &tree.edges[(root_node.first_edge as usize)
            ..(root_node.first_edge as usize + root_node.child_count as usize)];
        assert_eq!(edges.len(), 9);
        dbg!(edges);
        let children = edges
            .iter()
            .filter_map(|edge| {
                edge.child_node
                    .map(|child_idx| tree.nodes[child_idx.get() as usize])
            })
            .collect_vec();
        assert_eq!(children.len(), 9);
        dbg!(&children);
        let max_idx = children
            .iter()
            .position_max_by_key(|node| node.visits)
            .unwrap();
        assert_eq!(max_idx, 3);
        let max_child = children[max_idx];

        let expected_best_move = 4 * 9 + 3;
        assert_eq!(tree.best_explored_move(), expected_best_move);
        assert_eq!(max_child.child_count, 1);

        let max_edge = edges[max_idx];
        assert_eq!(max_edge.move_, expected_best_move);

        tree.apply_explored_move(expected_best_move);

        // should be the only available child
        tree.apply_explored_move(3 * 9 + 2);

        tree.search_n(3_000);
        let expected_best_move = 2 * 9;
        assert_eq!(tree.best_explored_move(), expected_best_move);

        let root_node = &tree.nodes[tree.root as usize];
        assert_eq!(root_node.child_count, 9 - 2);
    }
}
