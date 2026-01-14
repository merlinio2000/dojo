use std::num::NonZeroU8;

use crate::{
    board::{move_iter::BoardMoveIterU128, one_bit::OneBitBoard},
    consts,
    types::BoardState,
};

pub const fn const_concat<const A: usize, const B: usize, const C: usize>(
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

pub const fn repeat_bitpattern(pattern: u32, width: NonZeroU8, n: NonZeroU8) -> u128 {
    debug_assert!(
        pattern.leading_zeros() as usize >= size_of::<u32>() - width.get() as usize,
        "bits higher than the width of the pattern appear to be set"
    );
    let pattern = pattern as u128;
    let mut result = pattern;
    let mut i = 1;
    while i != n.get() {
        result |= pattern << (i * width.get());
        i += 1;
    }

    result
}

/// see [`BoardMajorBitset`] for the layout
pub fn to_board_col_major_move(row: u8, col: u8) -> u8 {
    let board_idx_row = row / consts::ROWS;
    let row_in_board = row % consts::ROWS;
    let board_idx_col = col / consts::COLS;
    let col_in_board = col % consts::COLS;

    let board_idx = board_idx_row + consts::COLS * board_idx_col;
    let idx_in_board = row_in_board + consts::COLS * col_in_board;

    board_idx * consts::N_CELLS + idx_in_board
}
///
/// see [`BoardMajorBitset`] for the layout
pub fn board_col_major_move_to_2d(board_col_major_move: u8) -> (u8, u8) {
    let board_idx = board_col_major_move / consts::N_BOARDS;
    let idx_in_board = board_col_major_move % consts::N_BOARDS;

    let board_row = board_idx % consts::ROWS;
    let board_col = board_idx / consts::COLS;

    let row_in_board = idx_in_board % consts::ROWS;
    let col_in_board = idx_in_board / consts::COLS;

    let row = board_row * consts::ROWS + row_in_board;
    let col = board_col * consts::COLS + col_in_board;
    (row, col)
}

/// +----+----+----+----+----+----+----+----+----+
/// |  0 |  3 |  6 | 27 | 30 | 33 | 54 | 57 | 60 |
/// +----+----+----+----+----+----+----+----+----+
/// |  1 |  4 |  7 | 28 | 31 | 34 | 55 | 58 | 61 |
/// +----+----+----+----+----+----+----+----+----+
/// |  2 |  5 |  8 | 29 | 32 | 35 | 56 | 59 | 62 |
/// +----+----+----+----+----+----+----+----+----+
/// |  9 | 12 | 15 | 36 | 39 | 42 | 63 | 66 | 69 |
/// +----+----+----+----+----+----+----+----+----+
/// | 10 | 13 | 16 | 37 | 40 | 43 | 64 | 67 | 70 |
/// +----+----+----+----+----+----+----+----+----+
/// | 11 | 14 | 17 | 38 | 41 | 44 | 65 | 68 | 71 |
/// +----+----+----+----+----+----+----+----+----+
/// | 18 | 21 | 24 | 45 | 48 | 51 | 72 | 75 | 78 |
/// +----+----+----+----+----+----+----+----+----+
/// | 19 | 22 | 25 | 46 | 49 | 52 | 73 | 76 | 79 |
/// +----+----+----+----+----+----+----+----+----+
/// | 20 | 23 | 26 | 47 | 50 | 53 | 74 | 77 | 80 |
/// +----+----+----+----+----+----+----+----+----+
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct BoardMajorBitset(u128);

impl BoardMajorBitset {
    const BOARD_FULL_MASK: u128 = 0b1_1111_1111;
    const BITS: u8 = consts::N_CELLS * consts::N_CELLS;
    const GRID_MASK: u128 = 2u128.pow(Self::BITS as u32) - 1;

    /// # Safety
    /// bits above/more significant than bit 80 must be 0
    pub const unsafe fn new_unchecked(board_col_major_indices: u128) -> Self {
        debug_assert!(board_col_major_indices >> Self::BITS == 0);
        Self(board_col_major_indices)
    }
    /// discards any bits not expected
    pub const fn new_truncated(board_col_major_indices: u128) -> Self {
        Self(board_col_major_indices & Self::GRID_MASK)
    }
    pub const fn new_full_board(board_idx: u8) -> Self {
        debug_assert!(board_idx < consts::N_CELLS, "board idx out of range");
        Self(BoardMajorBitset::BOARD_FULL_MASK << (board_idx * consts::N_CELLS))
    }

    pub const fn get(&self) -> u128 {
        self.0
    }

    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub const fn fill_board(&mut self, board_idx: u8) {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 |= board_full_mask;
    }
    pub const fn is_board_full(&self, board_idx: u8) -> bool {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 & board_full_mask == board_full_mask
    }
    pub const fn apply_move(&mut self, move_: u8) {
        self.0 |= 1 << move_;
    }

    pub const fn get_sub_board(&self, board_idx: u8) -> OneBitBoard {
        debug_assert!(board_idx < consts::N_CELLS);
        OneBitBoard::new((self.0 >> (board_idx * consts::N_CELLS)) as BoardState)
    }

    pub const fn unset_least_signifiact_one(&mut self) {
        self.0 &= self.0 - 1
    }

    pub fn iter_moves(&self) -> BoardMoveIterU128 {
        BoardMoveIterU128::new(*self)
    }
}

impl std::ops::Not for BoardMajorBitset {
    type Output = BoardMajorBitset;

    fn not(self) -> Self::Output {
        Self::new_truncated(!self.0)
    }
}
impl std::ops::BitAnd for BoardMajorBitset {
    type Output = BoardMajorBitset;

    fn bitand(self, rhs: Self) -> Self::Output {
        // safety: if one of both sets is valid, their and is valid
        unsafe { Self::new_unchecked(self.0 & rhs.0) }
    }
}
impl std::ops::BitOr for BoardMajorBitset {
    type Output = BoardMajorBitset;

    fn bitor(self, rhs: Self) -> Self::Output {
        // safety: if both sets are valid, their or is valid
        unsafe { Self::new_unchecked(self.0 | rhs.0) }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        consts,
        util::{board_col_major_move_to_2d, to_board_col_major_move},
    };

    #[test]
    fn from_board_col_major_to_2d() {
        assert_eq!(board_col_major_move_to_2d(0), (0, 0));
        assert_eq!(board_col_major_move_to_2d(1), (1, 0));
        assert_eq!(board_col_major_move_to_2d(2), (2, 0));
        assert_eq!(board_col_major_move_to_2d(3), (0, 1));
        assert_eq!(board_col_major_move_to_2d(4), (1, 1));
        assert_eq!(board_col_major_move_to_2d(5), (2, 1));
        assert_eq!(board_col_major_move_to_2d(6), (0, 2));
        assert_eq!(board_col_major_move_to_2d(7), (1, 2));
        assert_eq!(board_col_major_move_to_2d(8), (2, 2));

        assert_eq!(board_col_major_move_to_2d(9), (3, 0));
        assert_eq!(board_col_major_move_to_2d(10), (4, 0));
        assert_eq!(board_col_major_move_to_2d(11), (5, 0));
        assert_eq!(board_col_major_move_to_2d(12), (3, 1));
        assert_eq!(board_col_major_move_to_2d(13), (4, 1));

        assert_eq!(board_col_major_move_to_2d(3 * consts::N_CELLS), (0, 3));
        assert_eq!(board_col_major_move_to_2d(3 * consts::N_CELLS + 1), (1, 3));
        assert_eq!(board_col_major_move_to_2d(3 * consts::N_CELLS + 3), (0, 4));
        assert_eq!(board_col_major_move_to_2d(4 * consts::N_CELLS), (3, 3));

        assert_eq!(
            board_col_major_move_to_2d(consts::N_CELLS_NESTED - 1),
            (8, 8)
        );
    }

    #[test]
    fn to_board_col_major_from_2d() {
        assert_eq!(to_board_col_major_move(0, 0), 0);
        assert_eq!(to_board_col_major_move(1, 0), 1);
        assert_eq!(to_board_col_major_move(2, 0), 2);
        assert_eq!(to_board_col_major_move(0, 1), 3);
        assert_eq!(to_board_col_major_move(1, 1), 4);
        assert_eq!(to_board_col_major_move(2, 1), 5);
        assert_eq!(to_board_col_major_move(0, 2), 6);
        assert_eq!(to_board_col_major_move(1, 2), 7);
        assert_eq!(to_board_col_major_move(2, 2), 8);

        assert_eq!(to_board_col_major_move(3, 0), 9);
        assert_eq!(to_board_col_major_move(4, 0), 10);
        assert_eq!(to_board_col_major_move(5, 0), 11);
        assert_eq!(to_board_col_major_move(3, 1), 12);
        assert_eq!(to_board_col_major_move(4, 1), 13);

        assert_eq!(to_board_col_major_move(0, 3), 3 * consts::N_CELLS);
        assert_eq!(to_board_col_major_move(1, 3), 3 * consts::N_CELLS + 1);
        assert_eq!(to_board_col_major_move(0, 4), 3 * consts::N_CELLS + 3);
        assert_eq!(to_board_col_major_move(3, 3), 4 * consts::N_CELLS);

        assert_eq!(to_board_col_major_move(8, 7), 80 - 3);

        assert_eq!(to_board_col_major_move(8, 8), consts::N_CELLS_NESTED - 1,);
    }
}
