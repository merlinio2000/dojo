use crate::{consts, types::BoardState};

/// contains one bit per cell = 'is occupied by this/a player'
/// on a simple tic-tac-toe board (col-major)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneBitBoard(BoardState);

impl OneBitBoard {
    pub const fn new(state: BoardState) -> Self {
        OneBitBoard(state)
    }
    pub fn has_won(&self) -> bool {
        // TODO MERBUG: clippy heuristic for manual_contains is incorrect, maybe open a PR/bug
        // report
        consts::WINNER_MASKS_1BIT
            .iter()
            .any(|mask| *mask & self.0 == *mask)
    }
    pub fn set_cell(&mut self, cell: u8) {
        debug_assert!(cell < consts::N_CELLS as u8);
        self.0 |= 1 << cell;
    }
}
