use std::hint::black_box;

use ultimate_tic_tac_toe::tree::Tree;

fn main() {
    let mut mcts_tree = Tree::new();

    let n = 10;

    for _ in 0..n {
        black_box(mcts_tree.search());
    }
}
