const fn const_concat<const A: usize, const B: usize, const C: usize>(
    a: [u32; A],
    b: [u32; B],
) -> [u32; C] {
    let mut both = [0; C];
    let mut i = 0;
    while i != A {
        both[i] = a[i];
        i += 1;
    }
    i = 0;
    while i != B {
        both[A + i] = b[i];
        i += 1;
    }
    both
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum CellState {
    Free = 0b00,
    Player1 = 0b10,
    Player2 = 0b11,
}
const FREE_U8: u8 = CellState::Free as u8;
const PLAYER1_U8: u8 = CellState::Player1 as u8;
const PLAYER2_U8: u8 = CellState::Player2 as u8;

impl TryFrom<u8> for CellState {
    type Error = ();

    // TODO PERF: unsafe transmute for max perf (probably, should check the ASM first)
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            FREE_U8 => Ok(CellState::Free),
            PLAYER1_U8 => Ok(CellState::Player1),
            PLAYER2_U8 => Ok(CellState::Player2),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Player {
    Player1 = 0b0,
    Player2 = 0b1,
}
impl Player {
    // TODO PERF: could technically be just a `| 0b10`
    fn cell_state(&self) -> CellState {
        match self {
            Player::Player1 => CellState::Player1,
            Player::Player2 => CellState::Player2,
        }
    }
    // TODO PERF: could technically be just a bitflip
    fn other(&self) -> Player {
        match self {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        }
    }
}

type Score = i64;
const SCORE_LOSE: Score = -1;
const SCORE_WIN: Score = 1;

type Move = (usize, usize);

struct BoardMoveCalc {
    // TODO PERF: probably 0 pad for SIMD
    moves_buf: [Move; Board::N_CELLS],
}

impl BoardMoveCalc {
    const AVAILABLE_MASKS: [u32; Board::N_CELLS] = {
        let mut masks = [0; Board::N_CELLS];
        let mut idx = 0;
        while idx != masks.len() {
            masks[idx] = 0b10 << (idx * Board::CELL_BITS);
            idx += 1;
        }

        masks
    };

    fn new() -> BoardMoveCalc {
        Self {
            moves_buf: [(0, 0); Board::N_CELLS],
        }
    }

    fn available_moves(&mut self, board_state: BoardState) -> &[Move] {
        let is_available_results = Self::AVAILABLE_MASKS.map(|mask| (mask & board_state) == 0);
        let mut available_moves_idx = 0;
        for (cell_index, is_available) in is_available_results.into_iter().enumerate() {
            if is_available {
                self.moves_buf[available_moves_idx] = Board::to_2d_idx(cell_index);
                available_moves_idx += 1;
            }
        }
        &self.moves_buf[..available_moves_idx]
    }
}

type BoardState = u32;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Board(BoardState);
impl Board {
    #[allow(clippy::unusual_byte_groupings)]
    //                                      8   7 6  5 4  3 2  1 0
    const ALL_CELLS_OCCUPIED_MASK: BoardState = 0b10__1010_1010_1010_1010;
    const CELL_BITS: usize = 2;
    const COLS: usize = 3;
    const ROWS: usize = Self::COLS;
    const N_CELLS: usize = Self::COLS * Self::ROWS;
    fn new() -> Self {
        Self(0)
    }

    #[allow(unused)]
    fn from_matrix(matrix: [[CellState; Self::COLS]; Self::ROWS]) -> Self {
        let mut this = Self::new();
        for (row_idx, row) in matrix.into_iter().enumerate() {
            for (col_idx, cell) in row.into_iter().enumerate() {
                match cell {
                    CellState::Free => {}
                    CellState::Player1 => this.set(row_idx, col_idx, Player::Player1),
                    CellState::Player2 => this.set(row_idx, col_idx, Player::Player2),
                }
            }
        }
        this
    }
    // col major
    fn to_1d_idx(row: usize, col: usize) -> usize {
        debug_assert!(row < Self::ROWS);
        debug_assert!(col < Self::COLS);
        row + (col * Self::ROWS)
    }
    // col major
    fn to_2d_idx(one_d_idx: usize) -> Move {
        debug_assert!(one_d_idx < Self::N_CELLS);

        let row = one_d_idx % Self::COLS;
        let col = one_d_idx / Self::COLS;
        (row, col)
    }
    fn get(&self, row: usize, col: usize) -> CellState {
        let bits = ((self.0 >> (Self::CELL_BITS * Self::to_1d_idx(row, col))) & 0b11) as u8;
        CellState::try_from(bits).expect("invalid bits for CellState")
    }
    fn is_empty(&self) -> bool {
        self.0 == 0
    }
    fn is_full(&self) -> bool {
        self.0 & Self::ALL_CELLS_OCCUPIED_MASK == Self::ALL_CELLS_OCCUPIED_MASK
    }
    fn is_available(&self, row: usize, col: usize) -> bool {
        let mask = 0b10u32 << (Self::CELL_BITS * Self::to_1d_idx(row, col));
        self.0 & mask == 0
    }
    fn set(&mut self, row: usize, col: usize, player: Player) {
        debug_assert_eq!(self.get(row, col), CellState::Free);
        let new_cell_state = player.cell_state();
        self.0 |= (new_cell_state as u32) << (Self::CELL_BITS * Self::to_1d_idx(row, col));
    }

    /// is also the expected '&' result if player2 wins
    ///                        2  1 0
    const MASK_COL: u32 = 0b0011_1111;
    const MASKS_COL: [u32; Self::COLS] = {
        let mut masks = [0u32; Self::COLS];
        let mut i = 0;
        while i != Self::COLS {
            masks[i] = Self::MASK_COL << (i * Self::ROWS * Self::CELL_BITS);
            i += 1;
        }
        masks
    };
    //                                        2  1 0
    const MASK_RESULT_COL_PLAYER1: u32 = 0b0010_1010;
    const MASK_RESULTS_COL_PLAYER1: [u32; Self::COLS] = {
        let mut mask_results = [0u32; Self::COLS];
        let mut i = 0;
        while i != Self::COLS {
            mask_results[i] = Self::MASK_RESULT_COL_PLAYER1 << (i * Self::ROWS * Self::CELL_BITS);
            i += 1;
        }
        mask_results
    };
    /// is also the expected '&' result if player2 wins
    ///                      6  5 4  3 2  1 0
    const MASK_ROW: u32 = 0b11_0000_1100_0011;
    const MASKS_ROW: [u32; Self::ROWS] = {
        let mut masks = [0u32; Self::ROWS];
        let mut i = 0;
        while i != Self::ROWS {
            masks[i] = Self::MASK_ROW << (i * Self::CELL_BITS);
            i += 1;
        }
        masks
    };
    //                                      6  5 4  3 2  1 0
    const MASK_RESULT_ROW_PLAYER1: u32 = 0b10_0000_1000_0010;
    const MASK_RESULTS_ROW_PLAYER1: [u32; Self::ROWS] = {
        let mut mask_results = [0u32; Self::ROWS];
        let mut i = 0;
        while i != Self::ROWS {
            mask_results[i] = Self::MASK_RESULT_ROW_PLAYER1 << (i * Self::CELL_BITS);
            i += 1;
        }
        mask_results
    };
    #[allow(clippy::unusual_byte_groupings)]
    //                                 8   7 6  5 4  3 2  1 0
    const MASK_DIAG_POSITIVE: u32 = 0b00__0011_0011_0011_0000;
    #[allow(clippy::unusual_byte_groupings)]
    //                                                8   7 6  5 4  3 2  1 0
    const MASK_RESULT_DIAG_POSITIVE_PLAYER1: u32 = 0b00__0010_0010_0010_0000;
    #[allow(clippy::unusual_byte_groupings)]
    //                                 8   7 6  5 4  3 2  1 0
    const MASK_DIAG_NEGATIVE: u32 = 0b11__0000_0011_0000_0011;
    #[allow(clippy::unusual_byte_groupings)]
    //                                                8   7 6  5 4  3 2  1 0
    const MASK_RESULT_DIAG_NEGATIVE_PLAYER1: u32 = 0b10__0000_0010_0000_0010;

    /// also the expected '&' result if player2 wins
    const WINNER_MASKS: [u32; Self::COLS + Self::ROWS + 1 + 1] = const_concat(
        const_concat::<{ Self::COLS }, { Self::ROWS }, { Self::COLS + Self::ROWS }>(
            Self::MASKS_COL,
            Self::MASKS_ROW,
        ),
        [Self::MASK_DIAG_POSITIVE, Self::MASK_DIAG_NEGATIVE],
    );

    const MASK_RESULTS_PLAYER1: [u32; Self::COLS + Self::ROWS + 1 + 1] = const_concat(
        const_concat::<{ Self::COLS }, { Self::ROWS }, { Self::COLS + Self::ROWS }>(
            Self::MASK_RESULTS_COL_PLAYER1,
            Self::MASK_RESULTS_ROW_PLAYER1,
        ),
        [
            Self::MASK_RESULT_DIAG_POSITIVE_PLAYER1,
            Self::MASK_RESULT_DIAG_NEGATIVE_PLAYER1,
        ],
    );

    fn calc_winner(&self) -> Option<Player> {
        let winner_masked = Self::WINNER_MASKS.map(|mask| mask & self.0);
        for (i, winner_masked) in winner_masked.iter().enumerate() {
            if *winner_masked == Self::WINNER_MASKS[i] {
                return Some(Player::Player2);
            } else if *winner_masked == Self::MASK_RESULTS_PLAYER1[i] {
                return Some(Player::Player1);
            }
        }
        None
    }

    #[inline]
    pub(crate) fn find_move_scores(
        self,
        move_calc: &mut BoardMoveCalc,
        player: Player,
    ) -> impl ExactSizeIterator<Item = (Move, Score)> {
        move_calc
            .available_moves(self.0)
            .iter()
            .map(move |curr_move| {
                let curr_move_score = self.evaluate_move(curr_move.0, curr_move.1, player);
                (*curr_move, curr_move_score)
            })
    }

    #[inline]
    pub(crate) fn find_best_move_score(
        self,
        move_calc: &mut BoardMoveCalc,
        player: Player,
    ) -> (Move, Score) {
        Self::find_move_scores(self, move_calc, player).fold(
            // i32 for compatibility with other languages that parse to probably int
            ((i32::MAX as usize, i32::MAX as usize), Score::MIN),
            |(best_move, best_move_score), (curr_move, curr_move_score)| {
                if curr_move_score > best_move_score {
                    (curr_move, curr_move_score)
                } else {
                    (best_move, best_move_score)
                }
            },
        )
    }

    fn find_best_move(self, player: Player) -> Move {
        debug_assert_eq!(
            self.calc_winner(),
            None,
            "there should be no winner when finding best move"
        );
        debug_assert!(!self.is_full(), "cannot find move on full board");

        // start case is fixed, only choose the center cell
        // (negamax would arrive to the same conclusion)
        if self.is_empty() {
            return (Self::ROWS / 2, Self::COLS / 2);
        }

        // TODO PERF: extract for re-use
        let mut board_move_calc = BoardMoveCalc::new();

        let (best_move, _best_move_score) =
            Self::find_best_move_score(self, &mut board_move_calc, player);

        best_move
    }

    fn evaluate_move(mut self, row: usize, col: usize, player: Player) -> Score {
        debug_assert_eq!(self.get(row, col), CellState::Free);
        self.set(row, col, player);

        match self.calc_winner() {
            Some(our_player) if our_player == player => SCORE_WIN,
            Some(_other_player) => SCORE_LOSE,
            None if self.is_full() => 0,
            _ => {
                let other_player = player.other();
                -BoardMoveCalc::new()
                    .available_moves(self.0)
                    .iter()
                    .map(|(next_row, next_col)| {
                        self.evaluate_move(*next_row, *next_col, other_player)
                    })
                    .sum::<Score>()
            }
        }
    }
}

fn main() {
    let mut board = Board::new();
    let mut input = String::new();
    let read_line_buffered = |buf: &mut String| {
        buf.clear();
        std::io::stdin().read_line(buf).unwrap();
    };
    let mut my_player = Player::Player2;
    loop {
        read_line_buffered(&mut input);
        let (opp_row, opp_col) = input
            .trim_end()
            .split_once(' ')
            .expect("opponent input should have a space");
        let (opp_row, opp_col) = (
            opp_row.parse::<i32>().expect("opp_row is not usize"),
            opp_col.parse::<i32>().expect("opp_col is not usize"),
        );

        // read and discard available inputs
        read_line_buffered(&mut input);
        let n_available = input
            .trim_end()
            .parse::<usize>()
            .expect("n_available is not a usize");
        for _ in 0..n_available {
            read_line_buffered(&mut input);
        }

        if opp_row == -1 {
            my_player = Player::Player1;
        } else {
            board.set(opp_row as usize, opp_col as usize, my_player.other());
        }

        let (row, col) = board.find_best_move(my_player);
        board.set(row, col, my_player);
        println!("{row} {col}");
    }
}

/// TODO: the symmetric tests could potentially greatly reduce the number of combinations to try
#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{Board, BoardMoveCalc, CellState, Player, Score};

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
    fn test_board_empty() {
        let board = Board::new();
        for row in 0..Board::ROWS - 1 {
            for col in 0..Board::COLS - 1 {
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
        for row in 0..Board::ROWS - 1 {
            for col in 0..Board::COLS - 1 {
                let player = if (row + col) % 2 == 0 {
                    Player::Player1
                } else {
                    Player::Player2
                };
                board.set(row, col, player);
            }
        }
        for row in 0..Board::ROWS - 1 {
            for col in 0..Board::COLS - 1 {
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
            [Player2, Player2, Player2],
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
    fn test_available_moves() {
        use CellState::{Free, Player1, Player2};
        let mut move_iter = BoardMoveCalc::new();
        let board = Board::from_matrix([
            [Free, Free, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], (0, 0));
        assert_eq!(moves[1], (0, 1));

        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 9);
        for (idx, move_) in moves.iter().enumerate() {
            assert_eq!(*move_, Board::to_2d_idx(idx))
        }

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 0);

        let board = Board::from_matrix([
            [Player1, Free, Free],
            [Free, Free, Free],
            [Free, Free, Free],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 8);
        assert!(moves.iter().all(|move_| *move_ != (0, 0)));
    }

    #[test]
    fn test_best_move_1_left() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Free, Player1, Player1],
            [Player1, Player2, Player2],
            [Player2, Player1, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player1);
        assert_eq!(best_move, (0, 0));

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player1, Free, Player2],
            [Player2, Player1, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player1);
        assert_eq!(best_move, (1, 1));
    }

    #[test]
    fn test_best_move_2_left() {
        use CellState::{Free, Player1, Player2};

        let board = Board::from_matrix([
            [Player1, Player2, Player1],
            [Player1, Free, Player1],
            [Player2, Free, Player2],
        ]);
        let best_move = board.find_best_move(Player::Player2);
        assert_eq!(best_move, (2, 1));
    }

    #[test]
    fn test_move_scores_symmetric_vertically_and_horizontally_mirrored() {
        use CellState::{Free, Player1, Player2};

        let empty = Board::new();
        let move_calc = &mut BoardMoveCalc::new();
        let (best_move, best_score) = empty.find_best_move_score(move_calc, Player::Player1);
        assert_eq!(best_move, (1, 1));
        assert!(best_score >= 0);

        let only_center = Board::from_matrix([
            [Free, Free, Free],
            [Free, Player2, Free],
            [Free, Free, Free],
        ]);

        for (board, name) in [(empty, "empty"), (only_center, "only_center")] {
            let scores: HashMap<_, _> =
                board.find_move_scores(move_calc, Player::Player1).collect();
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
        let move_calc = &mut BoardMoveCalc::new();

        let scores: HashMap<_, _> = board.find_move_scores(move_calc, Player::Player2).collect();
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

        let move_calc = &mut BoardMoveCalc::new();
        for (idx, board) in [board0, board1, board2, board3].iter().enumerate() {
            let scores: HashMap<_, _> =
                board.find_move_scores(move_calc, Player::Player1).collect();
            let get_row =
                |row: usize| [0, 1, 2].map(|col| scores.get(&(row, col)).unwrap_or(&Score::MIN));

            let upper = get_row(0);
            let lower = get_row(2);
            assert_eq!(
                upper, lower,
                "upper and lower row should equal to each other for board horizontally symmetric board with index '{idx}'"
            );
        }
    }
}
