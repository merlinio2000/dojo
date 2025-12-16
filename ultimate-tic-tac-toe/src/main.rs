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

    // TODO: unsafe transmute for max perf (probably, should check the ASM first)
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
enum Player {
    Player1,
    Player2,
}
impl Player {
    fn cell_state(&self) -> CellState {
        match self {
            Player::Player1 => CellState::Player1,
            Player::Player2 => CellState::Player2,
        }
    }
}

// 2 bits per cell
// bit1: is occupied (bool)
// bit0: player1 = 0, player2 = 1
struct Board(u32);
impl Board {
    const CELL_SHIFT: usize = 2;
    const ROWS: usize = 3;
    const COLS: usize = 3;
    fn new() -> Self {
        Self(0)
    }
    // col major
    fn to_1d_idx(row: usize, col: usize) -> usize {
        debug_assert!(row < Self::ROWS);
        debug_assert!(col < Self::COLS);
        col + (row * Self::COLS)
    }
    fn get(&self, row: usize, col: usize) -> CellState {
        let bits = ((self.0 >> (Self::CELL_SHIFT * Self::to_1d_idx(row, col))) & 0b11) as u8;
        CellState::try_from(bits).expect("invalid bits for CellState")
    }
    fn set(&mut self, row: usize, col: usize, player: Player) {
        debug_assert_eq!(self.get(row, col), CellState::Free);
        let new_cell_state = player.cell_state();
        self.0 |= (new_cell_state as u32) << (Self::CELL_SHIFT * Self::to_1d_idx(row, col));
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use crate::{Board, CellState, Player};

    #[test]
    fn test_board_empty() {
        let board = Board::new();
        for col in 0..Board::COLS - 1 {
            for row in 0..Board::ROWS - 1 {
                assert_eq!(board.get(row, col), CellState::Free)
            }
        }
    }
    #[test]
    fn test_board_fill() {
        let mut board = Board::new();
        for col in 0..Board::COLS - 1 {
            for row in 0..Board::ROWS - 1 {
                let player = if (col + row) % 2 == 0 {
                    Player::Player1
                } else {
                    Player::Player2
                };
                board.set(row, col, player);
            }
        }
        for col in 0..Board::COLS - 1 {
            for row in 0..Board::ROWS - 1 {
                let player = if (col + row) % 2 == 0 {
                    Player::Player1
                } else {
                    Player::Player2
                };
                assert_eq!(board.get(row, col), player.cell_state())
            }
        }
    }
}
