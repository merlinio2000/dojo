use crate::{types::BoardState, util::const_concat};

pub const COLS: u8 = 3;
pub const ROWS: u8 = COLS;
pub const N_CELLS: u8 = COLS * ROWS;
pub const N_BOARDS: u8 = N_CELLS;
pub const N_CELLS_NESTED: u8 = N_CELLS * N_BOARDS;

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

pub const MASK_COL_1BIT: BoardState = 0b111;
pub const MASKS_COL_1BIT: [BoardState; COLS as usize] =
    build_3_col_shifted_masks::<1>(MASK_COL_1BIT);

///                                        6   3  0
pub const MASK_ROW_1BIT: BoardState = 0b0_0100_1001;
pub const MASKS_ROW_1BIT: [BoardState; ROWS as usize] =
    build_3_cell_shifted_masks::<1>(MASK_ROW_1BIT);

//                                                   6 4  2
pub const MASK_DIAG_POSITIVE_1BIT: BoardState = 0b0_0101_0100;

//                                                8    4    0
pub const MASK_DIAG_NEGATIVE_1BIT: BoardState = 0b1_0001_0001;

pub const WINNER_MASKS_1BIT: [BoardState; (COLS + ROWS + 1 + 1) as usize] = const_concat(
    const_concat::<{ COLS as usize }, { ROWS as usize }, { (COLS + ROWS) as usize }>(
        MASKS_COL_1BIT,
        MASKS_ROW_1BIT,
    ),
    [MASK_DIAG_POSITIVE_1BIT, MASK_DIAG_NEGATIVE_1BIT],
);
