use crate::{
    bitmagic,
    board::one_bit::OneBitBoard,
    consts,
    tree::{MonteCarloScore, NO_MOVE_FORCED, simulation::SimulationState},
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
    const SUPER_BOARD_OFFSET_IN_META: u8 = 32 - consts::N_BOARDS as u8;
    //                        player -|   forced_board -|:|
    const META_BITS_TO_CLEAR: u32 = 0b1_1111_1111_1111_1111;
    pub(super) const fn empty() -> Self {
        Self {
            bits: [0, (NO_MOVE_FORCED as u128) << Self::META_OFFSET],
        }
    }
    pub(super) const fn player1_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[0])
    }
    pub(super) const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[1])
    }

    const fn meta_player(&self, player: Player) -> u32 {
        (self.bits[player as usize] >> Self::META_OFFSET) as u32
    }
    /// TODO better name
    const fn meta_player2(&self) -> u32 {
        self.meta_player(Player::Player2)
    }
    pub(super) const fn forced_board(&self) -> u8 {
        self.meta_player2() as u8
    }
    pub(crate) const fn active_player(&self) -> Player {
        Player::from_is_player2((self.meta_player2() >> Self::PLAYER_OFFSET_IN_META) & 0b1 != 0)
    }

    pub(super) const fn super_board_for_player(&self, player: Player) -> BoardState {
        self.meta_player(player) >> Self::SUPER_BOARD_OFFSET_IN_META
    }

    pub(super) const fn get_player_board(&self, player: Player, board_idx: u8) -> OneBitBoard {
        OneBitBoard::new(
            (self.bits[player as usize] >> (board_idx * consts::N_CELLS as u8)) as BoardState,
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

    pub(crate) fn decide_draw(&self, in_favor_of: Player) -> MonteCarloScore {
        let won_board_favored_player =
            bitmagic::count_ones_u32(self.super_board_for_player(in_favor_of));
        let won_board_other_player =
            bitmagic::count_ones_u32(self.super_board_for_player(in_favor_of.other()));
        // TODO PERF: check if this branches / is optimal
        match Ord::cmp(&won_board_favored_player, &won_board_other_player) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
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
            child_state.bits[player as usize] |=
                0b1_1111_1111 << (board_idx * consts::N_CELLS as u8);
            // track wins in super board (specific to each player, not in general meta)
            child_state.bits[player as usize] |=
                1 << (Self::META_OFFSET + Self::SUPER_BOARD_OFFSET_IN_META + board_idx);
        }

        // clear meta bits before setting
        child_state.bits[Player::Player2 as usize] &=
            !((Self::META_BITS_TO_CLEAR as u128) << Self::META_OFFSET);
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

#[cfg(test)]
mod test {
    use crate::{consts, tree::NodeState};

    #[test]
    fn test_apply_move() {
        // ignore that this test disregards rules, we want to reach a sub board win as quickly as
        // possible
        let state = NodeState::empty();
        let (state, won) = state.apply_move(0);
        assert!(!won);
        assert_eq!(state.player1_occupied().get(), 0b1);
        assert_eq!(state.player2_occupied().get(), 0b0);
        assert_eq!(state.forced_board(), 0);
        assert_eq!(state.available_in_board_or_fallback().get(), 0b1_1111_1110);

        let (state, won) = state.apply_move(4);
        assert!(!won);
        assert_eq!(state.player1_occupied().get(), 0b0_0001);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS as u8)
        );

        let (state, won) = state.apply_move(1);
        assert!(!won);
        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 1);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS as u8)
        );

        let cell_idx = 3 * consts::N_CELLS as u8 + 4;
        let (state, won) = state.apply_move(cell_idx);
        assert!(!won);
        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000 | (0b1 << cell_idx));
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS as u8)
        );

        let (state, won) = state.apply_move(2);
        assert!(!won);
        assert_eq!(state.player1_occupied().get(), 0b1_1111_1111);
        assert_eq!(
            state.player2_occupied().get(),
            0b0_0001_0000 | (0b1 << cell_idx)
        );
        assert_eq!(state.forced_board(), 2);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS as u8)
        );
    }
}
