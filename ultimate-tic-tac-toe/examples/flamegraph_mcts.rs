use std::hint::black_box;

use ultimate_tic_tac_toe::tree::TreePlayer1;

fn main() {
    let mut mcts_tree = TreePlayer1::new();

    let n = 10;

    for _ in 0..n {
        black_box(mcts_tree.search());
    }
}
