use crate::{
    board::Board,
    consts,
    types::{Index, Player},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NestedBoard {
    root: Board,
    children: [Board; consts::N_CELLS as usize],
}

impl NestedBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calc_winner(&self) -> Option<Player> {
        self.root.calc_winner()
    }
    pub fn set(&mut self, row: Index, col: Index, player: Player) {
        let board_row = row / consts::ROWS;
        let row_in_board = row % consts::ROWS;
        let board_col = col / consts::COLS;
        let col_in_board = col % consts::COLS;

        self.children[Board::to_1d_idx(board_row, board_col) as usize].set(
            row_in_board,
            col_in_board,
            player,
        );
    }
}
