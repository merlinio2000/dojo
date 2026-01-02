use std::num::NonZeroU8;

use crate::{board::move_iter::BoardMoveIterU128, consts, types::Index};

pub(crate) const fn const_concat<const A: usize, const B: usize, const C: usize>(
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

pub(crate) const fn repeat_bitpattern(pattern: u32, width: NonZeroU8, n: NonZeroU8) -> u128 {
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
pub(crate) struct BoardMajorBitset(u128);

impl BoardMajorBitset {
    const BOARD_FULL_MASK: u128 = 0b1_1111_1111;
    const BITS: u32 = consts::N_CELLS * consts::N_CELLS;
    const GRID_MASK: u128 = 2u128.pow(Self::BITS) - 1;

    pub const unsafe fn new_unchecked(board_col_major_indices: u128) -> Self {
        debug_assert!(board_col_major_indices >> Self::BITS == 0);
        Self(board_col_major_indices)
    }
    /// discards any bits not expected
    pub const fn new_truncated(board_col_major_indices: u128) -> Self {
        Self(board_col_major_indices & Self::GRID_MASK)
    }
    pub const fn new_full_board(board_idx: Index) -> Self {
        Self(BoardMajorBitset::BOARD_FULL_MASK << (board_idx * consts::N_CELLS))
    }

    pub const fn get(&self) -> u128 {
        self.0
    }

    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    const fn fill_board(&mut self, board_idx: Index) {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 |= board_full_mask;
    }
    const fn is_board_full(&self, board_idx: Index) -> bool {
        debug_assert!(board_idx < consts::N_CELLS);
        let board_full_mask = Self::BOARD_FULL_MASK << (board_idx * consts::N_CELLS);
        self.0 & board_full_mask == board_full_mask
    }
    const fn apply_move(&mut self, move_: u8) {
        self.0 |= 1 << move_;
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
