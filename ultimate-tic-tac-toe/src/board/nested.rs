use crate::{
    board::{Board, move_finder::BoardMoveFinder},
    consts,
    types::{Index, Move, Player, Score},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NestedBoard {
    root: Board,
    children: [Board; consts::N_CELLS as usize],
}

// let board_row = row / consts::ROWS;
// let row_in_board = row % consts::ROWS;
// let board_col = col / consts::COLS;
// let col_in_board = col % consts::COLS;

impl NestedBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn calc_winner(&self) -> Option<Player> {
        self.root.calc_winner()
    }
    pub fn is_full(&self) -> bool {
        self.root.is_full()
    }
    pub fn is_empty(&self) -> bool {
        self.root.is_empty()
    }
    pub fn is_available(&self, board_idx: Index) -> bool {
        self.root.is_available_1d(board_idx)
    }

    fn set_root_1d(&mut self, board_idx: Index, player: Player) {
        self.root.set_1d(board_idx, player);
    }

    pub fn find_best_move(
        &mut self,
        player: Player,
        board_idx: Index,
        move_calc: &mut BoardMoveFinder,
    ) -> Move {
        debug_assert!(!self.is_full());

        if self.is_empty() {
            // start case is fixed, always the center board & cell
            (4, 4)
        } else {
            let ((best_board, best_idx_in_board), _best_score) = self
                .find_move_scores(board_idx, player, move_calc)
                .max_by_key(|(_, score)| *score)
                .expect("at least one move is available");

            let (board_row, board_col) = Board::to_2d_idx(best_board);
            let (row_in_board, col_in_board) = Board::to_2d_idx(best_idx_in_board);
            let row = board_row * consts::ROWS + row_in_board;
            let col = board_col * consts::COLS + col_in_board;
            (row, col)
        }
    }

    pub fn find_move_scores(
        self,
        board_idx: Index,
        player: Player,
        move_calc: &mut BoardMoveFinder,
    ) -> impl Iterator<Item = ((Index, Index), Score)> {
        let children_to_search = if self.is_available(board_idx) {
            move_calc.set_single(board_idx)
        } else {
            move_calc.available_moves(self.root.0)
        };
        children_to_search
            .iter()
            .flat_map(move |available_board_idx| {
                self.children[*available_board_idx as usize]
                    .iter_moves()
                    .map(move |idx_in_child| {
                        (
                            (*available_board_idx, idx_in_child),
                            self.evaluate_move(*available_board_idx, idx_in_child, player),
                        )
                    })
            })
    }

    pub fn evaluate_move(mut self, board_idx: Index, idx_in_board: Index, player: Player) -> Score {
        self.set_1d(board_idx, idx_in_board, player);

        if self.children[board_idx as usize].has_winner() {
            self.set_root_1d(board_idx, player);
        }

        match self.calc_winner() {
            Some(our_player) if our_player == player => consts::SCORE_WIN,
            Some(_other_player) => consts::SCORE_LOSE,
            None if self.is_full() => 0,
            _ => {
                let other_player = player.other();

                if self.is_available(idx_in_board) {
                    self.children[idx_in_board as usize]
                        .iter_moves()
                        .map(|available_move_in_board| {
                            self.evaluate_move(idx_in_board, available_move_in_board, other_player)
                        })
                        .sum::<Score>()
                } else {
                    self.root
                        .iter_moves()
                        .flat_map(|available_board| {
                            let child_board = self.children[available_board as usize];
                            child_board
                                .iter_moves()
                                .map(move |available_move_in_board| {
                                    self.evaluate_move(
                                        available_board,
                                        available_move_in_board,
                                        other_player,
                                    )
                                })
                        })
                        .sum::<Score>()
                }
            }
        }
    }

    /// col major
    pub(crate) fn set_1d(&mut self, board_idx: Index, idx_in_board: Index, player: Player) {
        self.children[board_idx as usize].set_1d(idx_in_board, player);
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{
        board::{move_finder::BoardMoveFinder, nested::NestedBoard},
        types::Player,
    };

    #[test]
    fn test_find_move_scores_empty() {
        let move_calc = &mut BoardMoveFinder::new();
        let empty = NestedBoard::new();
        let scores = empty
            .find_move_scores(4, Player::Player1, move_calc)
            .collect::<HashMap<_, _>>();
        dbg!(scores);
    }
}
