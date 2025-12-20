use crate::{board::Board, consts, types::Player};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NestedBoard {
    root: Board,
    children: [Board; consts::N_CELLS],
}

impl NestedBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calc_winner(&self) -> Option<Player> {
        self.root.calc_winner()
    }
    pub fn set(&mut self, row: usize, col: usize, player: Player) {
        let board_row = row / consts::ROWS;
        let row_in_board = row % consts::ROWS;
        let board_col = col / consts::COLS;
        let col_in_board = col % consts::COLS;

        self.children[Board::to_1d_idx(board_row, board_col)].set(
            row_in_board,
            col_in_board,
            player,
        );
    }
}
