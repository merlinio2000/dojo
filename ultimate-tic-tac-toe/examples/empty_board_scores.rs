use std::collections::HashMap;
use std::hint::black_box;

use ultimate_tic_tac_toe::{
    board::{Board, move_finder::BoardMoveFinder},
    types::Player,
};

fn main() {
    let empty_board = Board::new();
    let move_calc = &mut BoardMoveFinder::new();
    let _scores = empty_board
        .find_move_scores(black_box(Player::Player1), black_box(move_calc))
        .collect::<HashMap<_, _>>();
    // println!("scores (column-major) {_scores:#?}",)
}
