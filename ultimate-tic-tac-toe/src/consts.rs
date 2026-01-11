use std::num::NonZeroU8;

use crate::{
    types::{BoardState, Index, Move, Score},
    util::{const_concat, repeat_bitpattern},
};

pub const SCORE_LOSE: Score = -1;
pub const SCORE_WIN: Score = 1;

pub const CELL_BITS: usize = 2;
pub const COLS: Index = 3;
pub const ROWS: Index = COLS;
pub const N_CELLS: Index = COLS * ROWS;
pub const N_BOARDS: Index = N_CELLS;
pub const N_CELLS_NESTED: Index = N_CELLS * N_BOARDS;

const fn repeat_board_mask(mask: u32) -> u128 {
    repeat_bitpattern(
        mask,
        NonZeroU8::new(N_CELLS as u8).unwrap(),
        NonZeroU8::new(N_CELLS as u8).unwrap(),
    )
}

pub const LOOKUP_1D_TO_2D: [Move; N_CELLS as usize] = {
    let mut lookup = [(0, 0); N_CELLS as usize];
    let mut one_d_idx = 0;
    while one_d_idx != N_CELLS {
        let row = one_d_idx % COLS;
        let col = one_d_idx / COLS;
        lookup[one_d_idx as usize] = (row, col);
        one_d_idx += 1;
    }
    lookup
};

const fn build_3_col_shifted_masks<const BIT_WIDTH: usize>(
    mask: BoardState,
) -> [BoardState; COLS as usize] {
    let mut masks = [0; COLS as usize];
    let mut i = 0;
    while i != 3 {
        masks[i] = mask << (i * COLS as usize * BIT_WIDTH);
        i += 1;
    }
    masks
}

const fn build_3_cell_shifted_masks<const BIT_WIDTH: usize>(
    mask: BoardState,
) -> [BoardState; COLS as usize] {
    let mut masks = [0; COLS as usize];
    let mut i = 0;
    while i != 3 {
        masks[i] = mask << (i * BIT_WIDTH);
        i += 1;
    }
    masks
}

#[allow(clippy::unusual_byte_groupings)]
//                                                 8   7 6  5 4  3 2  1 0
pub const ALL_CELLS_OCCUPIED_MASK: BoardState = 0b10__1010_1010_1010_1010;

/// is also the expected '&' result if player2 wins
///                                   2  1 0
pub const MASK_COL: BoardState = 0b0011_1111;
pub const MASKS_COL: [BoardState; COLS as usize] = build_3_col_shifted_masks::<CELL_BITS>(MASK_COL);
//                                             210
pub(crate) const MASK_COL_1BIT: BoardState = 0b111;
pub(crate) const MASKS_COL_1BIT: [BoardState; COLS as usize] =
    build_3_col_shifted_masks::<1>(MASK_COL_1BIT);

//                                                   2  1 0
pub const MASK_RESULT_COL_PLAYER1: BoardState = 0b0010_1010;
pub const MASK_RESULTS_COL_PLAYER1: [BoardState; COLS as usize] =
    build_3_col_shifted_masks::<CELL_BITS>(MASK_RESULT_COL_PLAYER1);

/// is also the expected '&' result if player2 wins
///                                 6  5 4  3 2  1 0
pub const MASK_ROW: BoardState = 0b11_0000_1100_0011;
pub const MASKS_ROW: [BoardState; ROWS as usize] =
    build_3_cell_shifted_masks::<CELL_BITS>(MASK_ROW);

///                                        6   3  0
pub const MASK_ROW_1BIT: BoardState = 0b0_0100_1001;
pub const MASKS_ROW_1BIT: [BoardState; ROWS as usize] =
    build_3_cell_shifted_masks::<1>(MASK_ROW_1BIT);

//                                                 6  5 4  3 2  1 0
pub const MASK_RESULT_ROW_PLAYER1: BoardState = 0b10_0000_1000_0010;
pub const MASK_RESULTS_ROW_PLAYER1: [BoardState; ROWS as usize] =
    build_3_cell_shifted_masks::<CELL_BITS>(MASK_RESULT_ROW_PLAYER1);

#[allow(clippy::unusual_byte_groupings)]
//                                            8   7 6  5 4  3 2  1 0
pub const MASK_DIAG_POSITIVE: BoardState = 0b00__0011_0011_0011_0000;

//                                                   6 4  2
pub const MASK_DIAG_POSITIVE_1BIT: BoardState = 0b0_0101_0100;

#[allow(clippy::unusual_byte_groupings)]
//                                                           8   7 6  5 4  3 2  1 0
pub const MASK_RESULT_DIAG_POSITIVE_PLAYER1: BoardState = 0b00__0010_0010_0010_0000;

#[allow(clippy::unusual_byte_groupings)]
//                                            8   7 6  5 4  3 2  1 0
pub const MASK_DIAG_NEGATIVE: BoardState = 0b11__0000_0011_0000_0011;

//                                                8    4    0
pub const MASK_DIAG_NEGATIVE_1BIT: BoardState = 0b1_0001_0001;

#[allow(clippy::unusual_byte_groupings)]
//                                                           8   7 6  5 4  3 2  1 0
pub const MASK_RESULT_DIAG_NEGATIVE_PLAYER1: BoardState = 0b10__0000_0010_0000_0010;

/// also the expected '&' result if player2 wins
pub const WINNER_MASKS: [BoardState; (COLS + ROWS + 1 + 1) as usize] = const_concat(
    const_concat::<{ COLS as usize }, { ROWS as usize }, { (COLS + ROWS) as usize }>(
        MASKS_COL, MASKS_ROW,
    ),
    [MASK_DIAG_POSITIVE, MASK_DIAG_NEGATIVE],
);

pub const WINNER_MASKS_1BIT: [BoardState; (COLS + ROWS + 1 + 1) as usize] = const_concat(
    const_concat::<{ COLS as usize }, { ROWS as usize }, { (COLS + ROWS) as usize }>(
        MASKS_COL_1BIT,
        MASKS_ROW_1BIT,
    ),
    [MASK_DIAG_POSITIVE_1BIT, MASK_DIAG_NEGATIVE_1BIT],
);

pub const MASK_RESULTS_PLAYER1: [BoardState; (COLS + ROWS + 1 + 1) as usize] = const_concat(
    const_concat::<{ COLS as usize }, { ROWS as usize }, { (COLS + ROWS) as usize }>(
        MASK_RESULTS_COL_PLAYER1,
        MASK_RESULTS_ROW_PLAYER1,
    ),
    [
        MASK_RESULT_DIAG_POSITIVE_PLAYER1,
        MASK_RESULT_DIAG_NEGATIVE_PLAYER1,
    ],
);
