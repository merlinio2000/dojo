use crate::{consts, types::BoardState};

/// contains one bit per cell = 'is occupied by this/a player'
/// on a simple tic-tac-toe board (col-major)
pub struct OneBitBoard(BoardState);

impl OneBitBoard {
    pub const fn new(state: BoardState) -> Self {
        OneBitBoard(state)
    }
    pub fn has_won(&self) -> bool {
        consts::WINNER_MASKS_1BIT
            .iter()
            .any(|mask| *mask & self.0 == *mask)
    }
}
