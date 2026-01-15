use crate::{
    bitmagic,
    board::one_bit::OneBitBoard,
    consts, rng,
    tree::{MonteCarloScore, NodeState, node_state::NodeScore},
    types::Player,
    util::BoardMajorBitset,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationState {
    player_boards: [BoardMajorBitset; 2],
    /// contains information on who won which sub-board
    super_boards: [OneBitBoard; 2],
    active_player: Player,
    forced_board: u8,
    predetermined_score_favoring_previous_player: NodeScore,
}

impl SimulationState {
    pub const fn new(
        player_boards: [BoardMajorBitset; 2],
        super_boards: [OneBitBoard; 2],
        active_player: Player,
        forced_board: u8,
        predetermined_score_favoring_previous_player: NodeScore,
    ) -> Self {
        Self {
            player_boards,
            super_boards,
            active_player,
            forced_board,
            predetermined_score_favoring_previous_player,
        }
    }

    const fn player1_occupied(&self) -> BoardMajorBitset {
        self.player_boards[Player::Player1 as usize]
    }
    const fn player2_occupied(&self) -> BoardMajorBitset {
        self.player_boards[Player::Player2 as usize]
    }

    fn has_won(&self, player: Player) -> bool {
        self.super_boards[player as usize].has_won()
    }

    #[must_use]
    /// Applys a move and correctly changes the metadata, active player and won boards
    /// # Returns
    /// - new node state with move applied (and board bits won if board was won)
    /// - true if the active player won using this move
    pub fn apply_move(self, board_col_major_idx: u8) -> (Self, bool) {
        let mut child_state = self;
        let player = self.active_player;

        let board_idx = board_col_major_idx / consts::N_CELLS;

        child_state.player_boards[player as usize].apply_move(board_col_major_idx);

        let has_won_subboard = child_state.player_boards[player as usize]
            .get_sub_board(board_idx)
            .has_won();

        child_state.active_player = player.other();
        child_state.forced_board = board_col_major_idx % consts::N_CELLS;

        if has_won_subboard {
            // block all cells in that board (simpler logic for available moves)
            child_state.player_boards[player as usize].fill_board(board_idx);
            // track wins in super board
            child_state.super_boards[player as usize].set_cell(board_idx);
        }

        let won_game = if has_won_subboard {
            child_state.has_won(player)
        } else {
            false
        };

        (child_state, won_game)
    }

    pub fn available_in_board_or_fallback(&self) -> BoardMajorBitset {
        // TODO PERF: this code has an unnecessary '& GRID_MASK', check the asm
        let is_occupied = self.player1_occupied() | self.player2_occupied();
        let is_available = !is_occupied;

        let forced_board = self.forced_board;

        if forced_board == NodeState::NO_MOVE_FORCED {
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

    const fn decide_draw(&self, in_favor_of: Player) -> MonteCarloScore {
        let won_boards_favored_player =
            bitmagic::count_ones_u32(self.super_boards[in_favor_of as usize].get());
        let won_boards_other_player =
            bitmagic::count_ones_u32(self.super_boards[in_favor_of.other() as usize].get());
        // TODO PERF: check if this branches / is optimal
        if won_boards_favored_player > won_boards_other_player {
            1
        } else if won_boards_favored_player == won_boards_other_player {
            0
        } else {
            -1
        }
    }

    /// NOTE: favors the player that player before this move
    /// # Returns
    /// -  1 if the initially active player loses (favored wins)
    /// -  0 for a draw
    /// - -1 if the initally active player wins (favored loses)
    pub fn simulate_random(mut self) -> MonteCarloScore {
        if self.predetermined_score_favoring_previous_player != NodeScore::Indeterminate {
            return self
                .predetermined_score_favoring_previous_player
                .as_monte_carlo_score();
        }

        let mut has_won = false;
        let inital_player = self.active_player;
        let mut available_moves = self.available_in_board_or_fallback();
        debug_assert!(
            !available_moves.is_empty(),
            "can not simulate from a terminal state"
        );

        while !(has_won || available_moves.is_empty()) {
            let n_moves = bitmagic::count_ones_u128(available_moves.get()) as u8;
            let rand_nth_setbit = rng::rand_in_move_range_exclusive(n_moves);
            let rand_move =
                bitmagic::index_of_nth_setbit(available_moves.get(), rand_nth_setbit) as u8;
            (self, has_won) = self.apply_move(rand_move);

            available_moves = self.available_in_board_or_fallback();
        }

        if has_won {
            let looser = self.active_player;
            if looser == inital_player { 1 } else { -1 }
        } else {
            self.decide_draw(inital_player.other())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn simulation_decide_draw_win() {
        let state = SimulationState::new(
            // SAFETY: 0 is a valid bitset (empty board)
            unsafe {
                [
                    BoardMajorBitset::new_unchecked(0),
                    BoardMajorBitset::new_unchecked(0),
                ]
            },
            [OneBitBoard::new(0b000011011), OneBitBoard::new(0b100100100)], // P1: 5 boards, P2: 3 boards
            Player::Player1,
            NodeState::NO_MOVE_FORCED,
            NodeScore::Indeterminate,
        );

        assert_eq!(state.decide_draw(Player::Player1), 1);
        assert_eq!(state.decide_draw(Player::Player2), -1);
    }

    #[test]
    fn simulation_decide_draw_loss() {
        let state = SimulationState::new(
            // SAFETY: 0 is a valid bitset (empty board)
            unsafe {
                [
                    BoardMajorBitset::new_unchecked(0),
                    BoardMajorBitset::new_unchecked(0),
                ]
            },
            [OneBitBoard::new(0b000000011), OneBitBoard::new(0b100100100)], // P1: 2 boards, P2: 3 boards
            Player::Player1,
            NodeState::NO_MOVE_FORCED,
            NodeScore::Indeterminate,
        );

        assert_eq!(state.decide_draw(Player::Player1), -1);
        assert_eq!(state.decide_draw(Player::Player2), 1);
    }

    #[test]
    fn simulation_decide_draw_draw() {
        let state = SimulationState::new(
            // SAFETY:  < 2^81 0 is a valid bitset
            unsafe {
                [
                    // make sure that ones in the board are not counted
                    BoardMajorBitset::new_unchecked(0b101),
                    BoardMajorBitset::new_unchecked(0b001),
                ]
            },
            [OneBitBoard::new(0b000000111), OneBitBoard::new(0b000111000)], // P1: 3 boards, P2: 3 boards
            Player::Player1,
            NodeState::NO_MOVE_FORCED,
            NodeScore::Indeterminate,
        );

        // Equal boards, both should return 0 (draw)
        assert_eq!(state.decide_draw(Player::Player1), 0);
        assert_eq!(state.decide_draw(Player::Player2), 0);
    }
}
