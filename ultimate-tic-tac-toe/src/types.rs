pub type BoardState = u32;

/// see [`Player`]
pub type PlayerU8 = u8;
pub const PLAYER1_U8: PlayerU8 = 0;
pub const PLAYER2_U8: PlayerU8 = 1;
/// 0 = Player1, 1 = Player2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Player {
    Player1 = PLAYER1_U8,
    Player2 = PLAYER2_U8,
}
impl Player {
    // PERF: could technically be just a bitflip
    // asm shows this is already the case
    pub const fn other(&self) -> Player {
        match self {
            Player::Player1 => Player::Player2,
            Player::Player2 => Player::Player1,
        }
    }

    pub const fn from_is_player2(is_player2: bool) -> Self {
        // TODO PERF check that this is the identity function
        if is_player2 {
            Self::Player2
        } else {
            Self::Player1
        }
    }
}

pub type MonteCarloScore = i32;
