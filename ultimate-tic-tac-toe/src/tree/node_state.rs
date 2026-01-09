use crate::{
    board::one_bit::OneBitBoard,
    consts,
    tree::{NO_MOVE_FORCED, simulation::SimulationState},
    types::{BoardState, Player},
    util::BoardMajorBitset,
};

/// TODO MERBUG: is it possible to reach the same state but with a different active player?
/// NOTE: NodeState::default() is not a valid node state and more of a placeholder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) struct NodeState {
    /// # bits[0]
    /// bitset indicating is_occupied for player 1
    /// upper 32 bits are reserved for meta data
    /// [127:119] = "super board" / board containing is_won for subboards for player 1
    ///
    /// # bits[1] (including meta)
    /// [80:0] = bitset indicating is_occupied for player 2
    /// upper 32 bits are reserved for meta data
    /// MSB(127) = 0 -> no move forced
    /// [99:96] = forced board idx (9 = no forced board)
    /// [112:112] = active player as u8, see [`Player`]
    /// [127:119] = "super board" / board containing is_won for subboards for player 2
    bits: [u128; 2],
}

impl NodeState {
    const META_OFFSET: u8 = (128 - 32);
    const PLAYER_OFFSET_IN_META: u8 = 16;
    const SUPER_BOARD_OFFSET_IN_META: u8 = 23;
    const META_MASK: u32 = u32::MAX;
    pub(super) const fn empty() -> Self {
        Self {
            bits: [0, (NO_MOVE_FORCED as u128) << Self::META_OFFSET],
        }
    }
    const fn player1_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[0])
    }
    const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[1])
    }

    const fn meta_player(&self, player: Player) -> u32 {
        (self.bits[player as usize] >> Self::META_OFFSET) as u32
    }
    /// TODO better name
    const fn meta_player2(&self) -> u32 {
        self.meta_player(Player::Player2)
    }
    const fn forced_board(&self) -> u8 {
        self.meta_player2() as u8
    }
    const fn active_player(&self) -> Player {
        Player::from_is_player2((self.meta_player2() >> Self::PLAYER_OFFSET_IN_META) & 0b1 != 0)
    }

    const fn super_board_for_player(&self, player: Player) -> BoardState {
        self.meta_player(player) >> Self::SUPER_BOARD_OFFSET_IN_META
    }

    const fn get_player_board(&self, player: Player, board_idx: u8) -> OneBitBoard {
        OneBitBoard::new(
            (self.bits[player as usize] >> (board_idx as u32 * consts::N_CELLS)) as BoardState,
        )
    }
    pub(super) fn available_in_board_or_fallback(&self) -> BoardMajorBitset {
        // TODO PERF: this code has an unnecessary '& GRID_MASK', check the asm
        let is_occupied = self.player1_occupied() | self.player2_occupied();
        let is_available = !is_occupied;

        let forced_board = self.forced_board();

        if forced_board == NO_MOVE_FORCED {
            return is_available;
        }

        let board_mask = BoardMajorBitset::new_full_board(forced_board);
        let is_available_for_board = board_mask & is_available;

        // no moves are available for the board, return the whole grid as it is
        if is_available_for_board.is_empty() {
            is_available
        } else {
            is_available_for_board
        }
    }

    fn has_won(&self, player: Player) -> bool {
        OneBitBoard::new(self.super_board_for_player(player)).has_won()
    }

    #[must_use]
    /// Applys a move and correctly changes the metadata, active player and won boards
    /// # Returns
    /// - new node state with move applied (and board bits won if board was won)
    /// - true if the active player won using this move
    pub(super) fn apply_move(&self, board_col_major_idx: u8) -> (NodeState, bool) {
        let mut child_state = *self;
        let player = self.active_player();

        let board_idx = board_col_major_idx / consts::N_CELLS as u8;

        child_state.bits[player as usize] |= 0b1 << board_col_major_idx;

        let has_won_subboard = child_state.get_player_board(player, board_idx).has_won();
        let new_general_meta: u32 = ((player.other() as u32) << Self::PLAYER_OFFSET_IN_META)
            | (board_col_major_idx % consts::N_CELLS as u8) as u32;

        if has_won_subboard {
            // block all cells in that board (simpler logic for available moves)
            child_state.bits[player as usize] |= 0b1_1111_1111 << board_idx;
            // track wins in super board (specific to each player, not in general meta)
            child_state.bits[player as usize] |=
                1 << (Self::META_OFFSET + Self::SUPER_BOARD_OFFSET_IN_META + board_idx);
        }

        // clear meta bits before setting
        child_state.bits[Player::Player2 as usize] &=
            !((Self::META_MASK as u128) << Self::META_OFFSET);
        child_state.bits[Player::Player2 as usize] |=
            (new_general_meta as u128) << Self::META_OFFSET;
        let won_game = if has_won_subboard {
            child_state.has_won(player)
        } else {
            false
        };

        (child_state, won_game)
    }

    pub(super) fn into_simulation(self) -> SimulationState {
        SimulationState::new(
            self.bits.map(BoardMajorBitset::new_truncated),
            [Player::Player1, Player::Player2]
                .map(|player| OneBitBoard::new(self.super_board_for_player(player))),
            self.active_player(),
            self.forced_board(),
        )
    }
}
