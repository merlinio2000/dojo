pub type BoardState = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellState {
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
pub enum Player {
    Player1 = 0b0,
    Player2 = 0b1,
}
impl Player {
    // PERF: could technically be just a `| 0b10`
    // asm shows this is already the case
    pub fn cell_state(&self) -> CellState {
        match self {
            Player::Player1 => CellState::Player1,
            Player::Player2 => CellState::Player2,
        }
    }
    // PERF: could technically be just a bitflip
    // asm shows this is already the case
    pub fn other(&self) -> Player {
        match self {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        }
    }
}

pub type Score = i64;

pub type Index = usize;
pub type Move = (Index, Index);
