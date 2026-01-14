use crate::{
    board::{move_finder::BoardMoveFinder, move_iter::BoardMoveIter},
    consts::{self, LOOKUP_1D_TO_2D},
    types::{BoardState, CellState, Index, Move, Player, Score},
};

pub mod move_finder;
pub mod move_iter;
pub mod one_bit;

// 2 bits per cell
// bit1: is occupied (bool)
// bit0: player1 = 0, player2 = 1
//
// # Cell Layout
// 0 | 3 | 6 |
// - - - - - -
// 1 | 4 | 7 |
// - - - - - -
// 2 | 5 | 8
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Board(BoardState);
impl Board {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(unused)]
    pub fn from_matrix(
        matrix: [[CellState; consts::COLS as usize]; consts::ROWS as usize],
    ) -> Self {
        let mut this = Self::new();
        for (row_idx, row) in matrix.into_iter().enumerate() {
            for (col_idx, cell) in row.into_iter().enumerate() {
                match cell {
                    CellState::Free => {}
                    CellState::Player1 => {
                        this.set(row_idx as Index, col_idx as Index, Player::Player1)
                    }
                    CellState::Player2 => {
                        this.set(row_idx as Index, col_idx as Index, Player::Player2)
                    }
                }
            }
        }
        this
    }
    /// col major
    fn to_1d_idx(row: Index, col: Index) -> Index {
        debug_assert!(row < consts::ROWS);
        debug_assert!(col < consts::COLS);
        row + (col * consts::ROWS)
    }
    /// col major
    pub fn to_2d_idx(one_d_idx: Index) -> Move {
        debug_assert!(one_d_idx < consts::N_CELLS);

        LOOKUP_1D_TO_2D[one_d_idx as usize]
    }
    /// col major
    pub fn get_1d(&self, idx: Index) -> CellState {
        let bits = ((self.0 >> (consts::CELL_BITS * idx as usize)) & 0b11) as u8;
        CellState::try_from(bits).expect("invalid bits for CellState")
    }
    pub fn get(&self, row: Index, col: Index) -> CellState {
        self.get_1d(Self::to_1d_idx(row, col))
    }
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
    fn is_full(&self) -> bool {
        self.0 & consts::ALL_CELLS_OCCUPIED_MASK == consts::ALL_CELLS_OCCUPIED_MASK
    }
    /// col-major
    pub fn is_available_1d(&self, idx: Index) -> bool {
        let mask = 0b10 << (consts::CELL_BITS * idx as usize);
        self.0 & mask == 0
    }
    #[expect(unused)]
    pub fn is_available(&self, row: Index, col: Index) -> bool {
        self.is_available_1d(Self::to_1d_idx(row, col))
    }
    /// col major
    pub fn set_1d(&mut self, idx: Index, player: Player) {
        debug_assert_eq!(self.get_1d(idx), CellState::Free);
        let new_cell_state = player.cell_state();
        self.0 |= (new_cell_state as BoardState) << (consts::CELL_BITS * idx as usize);
    }
    pub fn set(&mut self, row: Index, col: Index, player: Player) {
        self.set_1d(Self::to_1d_idx(row, col), player);
    }

    pub fn iter_moves(self) -> BoardMoveIter {
        BoardMoveIter::new(self.0)
    }

    // NOTE PERF: avx/avx2 using 256bit registers have been evaluated but perform worse in this case
    // than the autovectorized code using `pand` `pcmpeqd` `packssdw` `pmovmskb`
    fn calc_winner(&self) -> Option<Player> {
        let mask_results = consts::WINNER_MASKS.map(|mask| mask & self.0);
        // NOTE PERF: this might look like multiple loops but it vectorizes much better
        // and there are NO loops in the generated assembly
        if mask_results
            .into_iter()
            .enumerate()
            .any(|(idx, result)| result == consts::WINNER_MASKS[idx])
        {
            return Some(Player::Player2);
        }

        if mask_results
            .into_iter()
            .enumerate()
            .any(|(idx, result)| result == consts::MASK_RESULTS_PLAYER1[idx])
        {
            return Some(Player::Player1);
        }
        None
    }

    // NOTE PERF: avx/avx2 using 256bit registers have been evaluated but perform worse in this case
    // than the autovectorized code using `pand` `pcmpeqd` `packssdw` `pmovmskb`
    fn has_winner(&self) -> bool {
        let mask_results = consts::WINNER_MASKS.map(|mask| mask & self.0);
        mask_results.into_iter().enumerate().any(|(idx, result)| {
            result == consts::WINNER_MASKS[idx] || result == consts::MASK_RESULTS_PLAYER1[idx]
        })
    }

    #[inline]
    pub fn find_move_scores(
        self,
        player: Player,
        move_calc: &mut BoardMoveFinder,
    ) -> impl ExactSizeIterator<Item = (Index, Score)> {
        let mut inner_move_calc = BoardMoveFinder::default();
        move_calc
            .available_moves(self.0)
            .iter()
            .map(move |curr_move| {
                let curr_move_score = self.evaluate_move(*curr_move, player, &mut inner_move_calc);
                (*curr_move, curr_move_score)
            })
    }

    /// TODO: make unsafe
    pub fn find_best_move_score(
        self,
        player: Player,
        move_calc: &mut BoardMoveFinder,
    ) -> (Index, Score) {
        // safety: this is done for CPU features which are asserted in main
        unsafe { self.find_best_move_score_inner(player, move_calc) }
    }

    unsafe fn find_best_move_score_inner(
        self,
        player: Player,
        move_calc: &mut BoardMoveFinder,
    ) -> (Index, Score) {
        Self::find_move_scores(self, player, move_calc).fold(
            (Index::MAX as Index, Score::MIN),
            |(best_move, best_move_score), (curr_move, curr_move_score)| {
                if curr_move_score > best_move_score {
                    (curr_move, curr_move_score)
                } else {
                    (best_move, best_move_score)
                }
            },
        )
    }

    pub fn find_best_move(self, player: Player, move_calc: &mut BoardMoveFinder) -> Index {
        debug_assert_eq!(
            self.calc_winner(),
            None,
            "there should be no winner when finding best move"
        );
        debug_assert!(!self.is_full(), "cannot find move on full board");
        debug_assert_eq!(consts::N_CELLS / 2, 4);

        // start case is fixed, only choose the center cell
        // (negamax would arrive to the same conclusion)
        if self.is_empty() {
            return 4;
        }

        // safety: main asserts the features are available
        let (best_move, _best_move_score) = Self::find_best_move_score(self, player, move_calc);

        best_move
    }

    fn evaluate_move(
        mut self,
        idx: Index,
        player: Player,
        move_calc: &mut BoardMoveFinder,
    ) -> Score {
        debug_assert_eq!(self.get_1d(idx), CellState::Free);
        self.set_1d(idx, player);

        match self.calc_winner() {
            Some(our_player) if our_player == player => consts::SCORE_WIN,
            Some(_other_player) => consts::SCORE_LOSE,
            None if self.is_full() => 0,
            _ => {
                let other_player = player.other();
                let mut inner_move_calc = BoardMoveFinder::default();
                -move_calc
                    .available_moves(self.0)
                    .iter()
                    .map(|next_idx| {
                        self.evaluate_move(*next_idx, other_player, &mut inner_move_calc)
                    })
                    .sum::<Score>()
            }
        }
    }
}

/// TODO: the symmetric tests could potentially greatly reduce the number of combinations to try
#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{
        board::{Board, move_finder::BoardMoveFinder},
        consts,
        types::{CellState, Index, Move, Player, Score},
    };

    fn _2d(idx: Index) -> Move {
        Board::to_2d_idx(idx)
    }

    #[test]
    /// verify we are truly col-major as all the bitmasks rely on it
    fn test_1d_idx() {
        assert_eq!(Board::to_1d_idx(0, 0), 0);
        assert_eq!(Board::to_1d_idx(1, 0), 1);
        assert_eq!(Board::to_1d_idx(2, 0), 2);
        assert_eq!(Board::to_1d_idx(0, 1), 3);
        assert_eq!(Board::to_1d_idx(1, 1), 4);
        assert_eq!(Board::to_1d_idx(2, 1), 5);
        assert_eq!(Board::to_1d_idx(0, 2), 6);
        assert_eq!(Board::to_1d_idx(1, 2), 7);
        assert_eq!(Board::to_1d_idx(2, 2), 8);
    }

    #[test]
    /// verify we are truly col-major as all the bitmasks rely on it
    fn test_2d_idx() {
        assert_eq!(Board::to_2d_idx(0), (0, 0));
        assert_eq!(Board::to_2d_idx(1), (1, 0));
        assert_eq!(Board::to_2d_idx(2), (2, 0));
        assert_eq!(Board::to_2d_idx(3), (0, 1));
        assert_eq!(Board::to_2d_idx(4), (1, 1));
        assert_eq!(Board::to_2d_idx(5), (2, 1));
        assert_eq!(Board::to_2d_idx(6), (0, 2));
        assert_eq!(Board::to_2d_idx(7), (1, 2));
        assert_eq!(Board::to_2d_idx(8), (2, 2));
    }

    #[test]
    fn test_board_empty() {
        let board = Board::new();
        for row in 0..consts::ROWS - 1 {
            for col in 0..consts::COLS - 1 {
                assert_eq!(board.get(row, col), CellState::Free)
            }
        }
    }
    #[test]
    fn test_board_set_bits() {
        let mut board = Board::new();
        board.set(1, 0, Player::Player1);
        let expected = 0b1000;
        assert_eq!(
            board.0, expected,
            "expected/got\n{expected:#018b}\n{:#018b}",
            board.0
        );
        board.set(2, 1, Player::Player2);
        //                     5 4  3 2  1 0
        let expected = 0b1100_0000_1000;
        assert_eq!(
            board.0, expected,
            "expected/got\n{expected:#018b}\n{:#018b}",
            board.0
        );
    }
    #[test]
    fn test_board_fill() {
        let mut board = Board::new();
        for row in 0..consts::ROWS - 1 {
            for col in 0..consts::COLS - 1 {
                let player = if (row + col) % 2 == 0 {
                    Player::Player1
                } else {
                    Player::Player2
                };
                board.set(row, col, player);
            }
        }
        for row in 0..consts::ROWS - 1 {
            for col in 0..consts::COLS - 1 {
                let player = if (row + col) % 2 == 0 {
                    Player::Player1
                } else {
                    Player::Player2
                };
                assert_eq!(board.get(row, col), player.cell_state())
            }
        }
    }

    #[allow(clippy::unusual_byte_groupings)]
    #[test]
    fn test_from_matrix() {
        use CellState::{Free, Player1, Player2};
        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        assert!(board.is_empty());
        let board = Board::from_matrix([
            [Free, Free, Player2],
            [Player2, Player1, Player2],
            [Player1, Player2, Player1],
        ]);
        assert_eq!(board.get(0, 0), Free);
        assert_eq!(board.get(0, 1), Free);
        assert_eq!(board.get(0, 2), Player2);
        assert_eq!(board.get(1, 0), Player2);
        assert_eq!(board.get(1, 1), Player1);
        assert_eq!(board.get(1, 2), Player2);
        assert_eq!(board.get(2, 0), Player1);
        assert_eq!(board.get(2, 1), Player2);
        assert_eq!(board.get(2, 2), Player1);

        //                     8   7 6  5 4  3 2  1 0
        let expected = 0b10__1111_1110_0010_1100;
        assert_eq!(
            board.0, expected,
            "expected/got\n{expected:#018b}\n{:#018b}",
            board.0
        )
    }

    #[test]
    fn test_winner_no_winner() {
        use CellState::{Free, Player1, Player2};

        assert_eq!(Board::new().calc_winner(), None);

        let board = Board::from_matrix([
            [Free, Free, Player2],
            [Player2, Player1, Player2],
            [Player1, Player2, Player1],
        ]);
        assert_eq!(board.calc_winner(), None);

        let board = Board::from_matrix([
            [Player2, Free, Player2],
            [Player2, Player1, Player2],
            [Player1, Player2, Player1],
        ]);
        assert_eq!(board.calc_winner(), None);

        let board = Board::from_matrix([
            [Player2, Free, Player2],
            [Free, Player1, Player2],
            [Player2, Player2, Player1],
        ]);
        assert_eq!(board.calc_winner(), None);
    }

    #[test]
    fn test_winner_row() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Free, Free, Player2],
            [Player2, Player1, Player2],
            [Player1, Player1, Player1],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player1));

        let board = Board::from_matrix([
            [Player1, Free, Player1],
            [Player2, Player2, Player2],
            [Player2, Player1, Player1],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player2));

        let board = Board::from_matrix([
            [Player1, Player1, Player1],
            [Player2, Player1, Player2],
            [Player2, Free, Player2],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player1));
    }

    #[test]
    fn test_winner_col() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Free, Free, Player2],
            [Player1, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player2));

        let board = Board::from_matrix([
            [Player1, Free, Player2],
            [Player1, Player2, Player2],
            [Player1, Player1, Player1],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player1));

        let board = Board::from_matrix([
            [Player1, Player1, Free],
            [Player2, Player1, Player2],
            [Player2, Player1, Player2],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player1));
    }
    #[test]
    fn test_winner_diag() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Free, Free, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player1));

        let board = Board::from_matrix([
            [Player1, Free, Player2],
            [Player1, Player2, Player2],
            [Player2, Player1, Player1],
        ]);
        assert_eq!(board.calc_winner(), Some(Player::Player2));
    }

    #[test]
    fn test_best_move_1_left() {
        use CellState::{Free, Player1, Player2};

        let move_calc = &mut BoardMoveFinder::default();
        let board = Board::from_matrix([
            [Free, Player1, Player1],
            [Player1, Player2, Player2],
            [Player2, Player1, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player1, move_calc);
        assert_eq!(_2d(best_move), (0, 0));

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player1, Free, Player2],
            [Player2, Player1, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player1, move_calc);
        assert_eq!(_2d(best_move), (1, 1));
    }

    #[test]
    fn test_best_move_2_left() {
        use CellState::{Free, Player1, Player2};

        let move_calc = &mut BoardMoveFinder::default();

        let board = Board::from_matrix([
            [Player1, Player2, Player1],
            [Player1, Free, Player1],
            [Player2, Free, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player2, move_calc);
        assert_eq!(_2d(best_move), (2, 1));
    }

    #[test]
    fn test_move_scores_symmetric_vertically_and_horizontally_mirrored() {
        use CellState::{Free, Player2};

        let empty = Board::new();
        let move_calc = &mut BoardMoveFinder::new();
        let (best_move, best_score) = empty.find_best_move_score(Player::Player1, move_calc);
        assert_eq!(_2d(best_move), (1, 1));
        assert!(best_score >= 0);

        let only_center = Board::from_matrix([
            [Free, Free, Free],
            [Free, Player2, Free],
            [Free, Free, Free],
        ]);

        for (board, name) in [(empty, "empty"), (only_center, "only_center")] {
            let scores: HashMap<_, _> = board
                .find_move_scores(Player::Player1, move_calc)
                .map(|(move_, score)| (_2d(move_), score))
                .collect();
            // equal corners on empty board
            let msg = format!("corners should equal to each other for board type: '{name}'");
            assert_eq!(scores.get(&(0, 0)), scores.get(&(0, 2)), "{msg}");
            assert_eq!(scores.get(&(0, 0)), scores.get(&(2, 0)), "{msg}");
            assert_eq!(scores.get(&(0, 0)), scores.get(&(2, 2)), "{msg}");

            // equal middles-of-the-sides
            let msg =
                format!("middles-of-the-sides should equal to each other for board type: '{name}'");
            assert_eq!(scores.get(&(1, 0)), scores.get(&(0, 1)), "{msg}");
            assert_eq!(scores.get(&(1, 0)), scores.get(&(1, 2)), "{msg}");
            assert_eq!(scores.get(&(1, 0)), scores.get(&(2, 1)), "{msg}");
        }
    }

    #[test]
    fn test_move_scores_symmetric_vertically_mirrored() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Free, Free, Free],
            [Player1, Player2, Player1],
            [Free, Free, Free],
        ]);
        let move_calc = &mut BoardMoveFinder::new();

        let scores: HashMap<_, _> = board
            .find_move_scores(Player::Player2, move_calc)
            .map(|(move_, score)| (_2d(move_), score))
            .collect();
        // equal corners on empty board
        assert_eq!(scores.get(&(0, 0)), scores.get(&(0, 2)));
        assert_eq!(scores.get(&(0, 0)), scores.get(&(2, 0)));
        assert_eq!(scores.get(&(0, 0)), scores.get(&(2, 2)));
        // equal middle-of-the-sides
        assert_eq!(scores.get(&(0, 1)), scores.get(&(2, 1)));
    }

    #[test]
    fn test_move_scores_symmetric_horizontally_mirrored() {
        use CellState::{Free, Player1, Player2};

        let board0 = Board::from_matrix([
            [Free, Free, Free],
            [Player1, Player2, Free],
            [Free, Free, Free],
        ]);
        let board1 = Board::from_matrix([
            [Free, Free, Free],
            [Player1, Player2, Player2],
            [Free, Free, Free],
        ]);
        let board2 = Board::from_matrix([
            [Player2, Free, Free],
            [Player1, Player2, Player1],
            [Player2, Free, Free],
        ]);
        let board3 = Board::from_matrix([
            [Player2, Free, Player1],
            [Player1, Player1, Player2],
            [Player2, Free, Player1],
        ]);

        let move_calc = &mut BoardMoveFinder::new();
        for (idx, board) in [board0, board1, board2, board3].iter().enumerate() {
            let scores: HashMap<_, _> = board
                .find_move_scores(Player::Player1, move_calc)
                .map(|(move_, score)| (_2d(move_), score))
                .collect();
            let get_row =
                |row: Index| [0, 1, 2].map(|col| scores.get(&(row, col)).unwrap_or(&Score::MIN));

            let upper = get_row(0);
            let lower = get_row(2);
            assert_eq!(
                upper, lower,
                "upper and lower row should equal to each other for board horizontally symmetric board with index '{idx}'"
            );
        }
    }
}
