use std::hint::black_box;

use ultimate_tic_tac_toe::{
    board::{Board, move_finder::BoardMoveFinder},
    types::Player,
};

fn main() {
    let empty_board = Board::new();
    let move_calc = &mut BoardMoveFinder::new();

    let n = 1_500;

    for _ in 0..n {
        black_box(
            empty_board.find_best_move_score(black_box(Player::Player1), black_box(move_calc)),
        );
    }
}
