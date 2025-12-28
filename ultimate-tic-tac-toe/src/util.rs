use crate::{board::move_iter::BoardMoveIterU128, consts, types::Index};

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
