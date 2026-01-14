use crate::{consts, types::BoardState};

/// contains one bit per cell = 'is occupied by this/a player'
/// on a simple tic-tac-toe board (col-major)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneBitBoard(BoardState);

impl OneBitBoard {
    const MASK: BoardState = 0b1_1111_1111;
    pub const fn new(state: BoardState) -> Self {
        OneBitBoard(state & Self::MASK)
    }
    #[cfg(test)]
    pub const fn new_full() -> Self {
        OneBitBoard(Self::MASK)
    }
    pub fn has_won(&self) -> bool {
        // TODO MERBUG: clippy heuristic for manual_contains is incorrect, maybe open a PR/bug
        // report
        consts::WINNER_MASKS_1BIT
            .iter()
            .any(|mask| *mask & self.0 == *mask)
    }
    pub const fn set_cell(&mut self, cell: u8) {
        debug_assert!(cell < consts::N_CELLS as u8);
        self.0 |= 1 << cell;
    }
    pub const fn get(&self) -> BoardState {
        self.0
    }
}

#[cfg(test)]
mod test {
    use crate::board::one_bit::OneBitBoard;

    #[test]
    fn has_won_horizontal_top_row() {
        let board = OneBitBoard::new(0b001_001_001);
        assert!(board.has_won());
        let board = OneBitBoard::new(0b010_010_010);
        assert!(board.has_won());
        let board = OneBitBoard::new(0b100_100_100);
        assert!(board.has_won());
    }

    #[test]
    fn has_won_vertical_left_column() {
        let board = OneBitBoard::new(0b000_000_111);
        assert!(board.has_won());
        let board = OneBitBoard::new(0b000_111_000);
        assert!(board.has_won());
        let board = OneBitBoard::new(0b111_000_000);
        assert!(board.has_won());
    }

    #[test]
    fn has_won_diagonal() {
        let board = OneBitBoard::new(0b100_010_001);
        assert!(board.has_won());
        let board = OneBitBoard::new(0b001_010_100);
        assert!(board.has_won());
    }

    #[test]
    fn has_not_won() {
        let board = OneBitBoard::new(0b000_001_001);
        assert!(!board.has_won());
        let board = OneBitBoard::new(0b010_010_001);
        assert!(!board.has_won());
        let board = OneBitBoard::new(0b000_000_000);
        assert!(!board.has_won());
    }

    #[test]
    fn has_won_with_extra_bits_set() {
        // top row (0,3,6) plus noise bits
        let board = OneBitBoard::new(0b101_001_001);
        assert!(board.has_won());
    }
}
