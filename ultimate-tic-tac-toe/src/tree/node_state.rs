use crate::{
    bitmagic,
    board::one_bit::OneBitBoard,
    consts,
    tree::{MonteCarloScore, simulation::SimulationState},
    types::{BoardState, Player},
    util::BoardMajorBitset,
};

const NODE_SCORE_INDETERMINATE: u8 = 0;
const NODE_SCORE_LOSS: u8 = 1;
const NODE_SCORE_DRAW: u8 = 2;
const NODE_SCORE_WIN: u8 = 3;

/// from the perspective of the person that applied the move leading to this node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum NodeScore {
    #[default]
    Indeterminate = NODE_SCORE_INDETERMINATE,
    Loss = NODE_SCORE_LOSS,
    Draw = NODE_SCORE_DRAW,
    Win = NODE_SCORE_WIN,
}

impl NodeScore {
    /// # Safety:
    /// `score` must be a valid value of [`NodeScore`]
    pub const unsafe fn from_u8_unchecked(score: u8) -> Self {
        match score {
            NODE_SCORE_INDETERMINATE => NodeScore::Indeterminate,
            NODE_SCORE_LOSS => NodeScore::Loss,
            NODE_SCORE_DRAW => NodeScore::Draw,
            NODE_SCORE_WIN => NodeScore::Win,
            #[allow(unused_unsafe, reason = "only unsafe in release mode")]
            _ => unsafe {
                #[cfg(debug_assertions)]
                {
                    panic!("not a valid NodeScore");
                }
                #[cfg(not(debug_assertions))]
                std::hint::unreachable_unchecked();
            },
        }
    }

    pub const fn as_monte_carlo_score(&self) -> MonteCarloScore {
        match *self {
            NodeScore::Indeterminate => 0,
            NodeScore::Loss => -1,
            NodeScore::Draw => 0,
            NodeScore::Win => 1,
        }
    }
}

/// TODO MERBUG: is it possible to reach the same state but with a different active player?
/// NOTE: NodeState::default() is not a valid node state and more of a placeholder
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(align(32))]
pub struct NodeState {
    /// # bits[0]
    /// bitset indicating is_occupied for player 1
    /// upper 32 bits are reserved for meta data
    /// [127:119] = "super board" / board containing is_won for subboards for player 1
    ///
    /// # bits[1] (including meta)
    /// [80:0] = bitset indicating is_occupied for player 2
    /// upper 32 bits are reserved for meta data
    /// [99:96] = forced board idx (9 = no forced board)
    /// [105:104] = node score
    /// [112:112] = active player, see [`Player`]
    /// [127:119] = "super board" / board containing is_won for subboards for player 2
    bits: [u128; 2],
}

impl NodeState {
    pub const NO_MOVE_FORCED: u8 = 9;
    const META_OFFSET: u8 = (128 - 32);
    const SCORE_OFFSET_IN_META: u8 = 8;
    const PLAYER_OFFSET_IN_META: u8 = 16;
    const SUPER_BOARD_OFFSET_IN_META: u8 = 32 - consts::N_BOARDS;
    /// node_score is ok to be theoretically cleared as after it is set there shouldn't be any more
    /// moves
    ///                       player -|   forced_board -|:|
    const META_BITS_TO_CLEAR: u32 = 0b1_1111_1111_1111_1111;
    pub const fn empty() -> Self {
        Self {
            bits: [0, (Self::NO_MOVE_FORCED as u128) << Self::META_OFFSET],
        }
    }
    pub const fn player1_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[0])
    }
    pub const fn player2_occupied(&self) -> BoardMajorBitset {
        BoardMajorBitset::new_truncated(self.bits[1])
    }

    const fn meta_player(&self, player: Player) -> u32 {
        (self.bits[player as usize] >> Self::META_OFFSET) as u32
    }
    /// TODO better name
    const fn meta_player2(&self) -> u32 {
        self.meta_player(Player::Player2)
    }
    pub const fn forced_board(&self) -> u8 {
        let forced_board = self.meta_player2();
        debug_assert!(forced_board as u8 <= Self::NO_MOVE_FORCED);
        forced_board as u8
    }
    pub const fn active_player(&self) -> Player {
        Player::from_is_player2((self.meta_player2() >> Self::PLAYER_OFFSET_IN_META) & 0b1 != 0)
    }

    pub const fn node_score_favoring_previous_player(&self) -> NodeScore {
        // safety: we fully controll the bits set here and this is always guaranteed to be valid
        unsafe {
            NodeScore::from_u8_unchecked((self.meta_player2() >> Self::SCORE_OFFSET_IN_META) as u8)
        }
    }

    pub const fn super_board_for_player(&self, player: Player) -> BoardState {
        self.meta_player(player) >> Self::SUPER_BOARD_OFFSET_IN_META
    }

    pub const fn get_player_board(&self, player: Player, board_idx: u8) -> OneBitBoard {
        OneBitBoard::new(
            (self.bits[player as usize] >> (board_idx * consts::N_CELLS)) as BoardState,
        )
    }
    pub fn available_in_board_or_fallback(&self) -> BoardMajorBitset {
        // TODO PERF: this code has an unnecessary '& GRID_MASK', check the asm
        let is_occupied = self.player1_occupied() | self.player2_occupied();
        let is_available = !is_occupied;

        let forced_board = self.forced_board();

        if forced_board == Self::NO_MOVE_FORCED {
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

    const fn decide_draw(&self, in_favor_of: Player) -> NodeScore {
        let won_boards_favored_player =
            bitmagic::count_ones_u32(self.super_board_for_player(in_favor_of));
        let won_boards_other_player =
            bitmagic::count_ones_u32(self.super_board_for_player(in_favor_of.other()));
        // TODO PERF: check if this branches / is optimal
        if won_boards_favored_player > won_boards_other_player {
            NodeScore::Loss
        } else if won_boards_favored_player == won_boards_other_player {
            NodeScore::Draw
        } else {
            NodeScore::Win
        }
    }

    fn has_won(&self, player: Player) -> bool {
        OneBitBoard::new(self.super_board_for_player(player)).has_won()
    }

    #[must_use]
    /// Applys a move and correctly changes the metadata, active player and won boards
    /// # Returns
    /// - new node state with move applied (and board bits won if board was won)
    /// - the amount of children the new node has (notably 0 if it is a terminal node)
    pub fn apply_move(&self, board_col_major_idx: u8) -> (NodeState, u8, NodeScore) {
        debug_assert_eq!(
            self.node_score_favoring_previous_player(),
            NodeScore::Indeterminate
        );
        let mut child_state = *self;
        let favored_player = self.active_player();

        let board_idx = board_col_major_idx / consts::N_CELLS;

        child_state.bits[favored_player as usize] |= 0b1 << board_col_major_idx;

        let has_won_subboard = child_state
            .get_player_board(favored_player, board_idx)
            .has_won();
        let new_general_meta: u32 = ((favored_player.other() as u32)
            << Self::PLAYER_OFFSET_IN_META)
            | (board_col_major_idx % consts::N_CELLS) as u32;

        if has_won_subboard {
            // block all cells in that board (simpler logic for available moves)
            child_state.bits[favored_player as usize] |=
                0b1_1111_1111 << (board_idx * consts::N_CELLS);
            // track wins in super board (specific to each player, not in general meta)
            child_state.bits[favored_player as usize] |=
                1 << (Self::META_OFFSET + Self::SUPER_BOARD_OFFSET_IN_META + board_idx);
        }

        // clear meta bits before setting
        child_state.bits[Player::Player2 as usize] &=
            !((Self::META_BITS_TO_CLEAR as u128) << Self::META_OFFSET);
        child_state.bits[Player::Player2 as usize] |=
            (new_general_meta as u128) << Self::META_OFFSET;

        let available_in_child = child_state.available_in_board_or_fallback();
        let score = if has_won_subboard && child_state.has_won(favored_player) {
            NodeScore::Win
        } else if available_in_child.is_empty() {
            child_state.decide_draw(favored_player)
        } else {
            NodeScore::Indeterminate
        };

        child_state.bits[Player::Player2 as usize] |=
            (score as u128) << (Self::META_OFFSET + Self::SCORE_OFFSET_IN_META);

        let child_count = if score == NodeScore::Indeterminate {
            bitmagic::count_ones_u128(available_in_child.get()) as u8
        } else {
            0
        };

        (child_state, child_count, score)
    }

    pub fn into_simulation(self) -> SimulationState {
        SimulationState::new(
            self.bits.map(BoardMajorBitset::new_truncated),
            [Player::Player1, Player::Player2]
                .map(|player| OneBitBoard::new(self.super_board_for_player(player))),
            self.active_player(),
            self.forced_board(),
        )
    }

    #[cfg(test)]
    pub fn from_boards(
        player_boards: [[OneBitBoard; 9]; 2],
        forced_board: u8,
        active_player: Player,
    ) -> Self {
        let mut result = Self { bits: [0, 0] };
        for (player_idx, boards) in player_boards.into_iter().enumerate() {
            if player_idx == 1 {
                assert_eq!(result.super_board_for_player(Player::Player1), 0b11);
            }
            for (board_idx, board) in boards.into_iter().enumerate() {
                use crate::types::{PLAYER1_U8, PLAYER2_U8};

                result.bits[player_idx] |=
                    (board.get() as u128) << (board_idx * consts::N_CELLS as usize);
                if player_idx as u8 == PLAYER1_U8 {
                    assert_eq!(
                        player_boards[PLAYER2_U8 as usize][board_idx].get() & board.get(),
                        0
                    );
                }
                if board.has_won() {
                    result.bits[player_idx] |= 1
                        << (Self::META_OFFSET + Self::SUPER_BOARD_OFFSET_IN_META + board_idx as u8);
                }
            }
        }
        result.bits[Player::Player2 as usize] |=
            ((active_player as u128) << Self::PLAYER_OFFSET_IN_META | forced_board as u128)
                << Self::META_OFFSET;
        assert_eq!(
            result.bits[Player::Player1 as usize] >> Self::META_OFFSET
                & result.bits[Player::Player2 as usize] >> Self::META_OFFSET,
            0
        );

        if result.has_won(active_player) {
            panic!("can not create loss node")
        } else if result.has_won(active_player.other()) {
            panic!("can not create win node")
        } else if result.available_in_board_or_fallback().is_empty() {
            panic!("can not create terminal node")
        }
        result.bits[Player::Player2 as usize] |=
            (NodeScore::Indeterminate as u128) << (Self::META_OFFSET + Self::SCORE_OFFSET_IN_META);

        result
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
        let (state, child_count, ..) = state.apply_move(0);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b1);
        assert_eq!(state.player2_occupied().get(), 0b0);
        assert_eq!(state.forced_board(), 0);
        assert_eq!(state.available_in_board_or_fallback().get(), 0b1_1111_1110);

        let (state, child_count, ..) = state.apply_move(4);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b0_0001);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let (state, child_count, ..) = state.apply_move(1);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 1);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let cell_idx = 3 * consts::N_CELLS + 4;
        let (state, child_count, ..) = state.apply_move(cell_idx);
        assert_ne!(child_count, 0);

        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000 | (0b1 << cell_idx));
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let (state, child_count, ..) = state.apply_move(2);
        assert_ne!(child_count, 0);

        assert_eq!(state.player1_occupied().get(), 0b1_1111_1111);
        assert_eq!(
            state.player2_occupied().get(),
            0b0_0001_0000 | (0b1 << cell_idx)
        );
        assert_eq!(state.forced_board(), 2);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );
    }

    #[test]
    fn test_win() {
        // ignore that this test disregards rules, we want to reach a sub board win as quickly as
        // possible
        let state = NodeState::empty();
        let (state, child_count, ..) = state.apply_move(0);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b1);
        assert_eq!(state.player2_occupied().get(), 0b0);
        assert_eq!(state.forced_board(), 0);
        assert_eq!(state.available_in_board_or_fallback().get(), 0b1_1111_1110);

        let (state, child_count, ..) = state.apply_move(4);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b0_0001);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let (state, child_count, ..) = state.apply_move(1);
        assert_ne!(child_count, 0);
        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000);
        assert_eq!(state.forced_board(), 1);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let cell_idx = 3 * consts::N_CELLS + 4;
        let (state, child_count, ..) = state.apply_move(cell_idx);
        assert_ne!(child_count, 0);

        assert_eq!(state.player1_occupied().get(), 0b0_0011);
        assert_eq!(state.player2_occupied().get(), 0b1_0000 | (0b1 << cell_idx));
        assert_eq!(state.forced_board(), 4);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );

        let (state, child_count, ..) = state.apply_move(2);
        assert_ne!(child_count, 0);

        assert_eq!(state.player1_occupied().get(), 0b1_1111_1111);
        assert_eq!(
            state.player2_occupied().get(),
            0b0_0001_0000 | (0b1 << cell_idx)
        );
        assert_eq!(state.forced_board(), 2);
        assert_eq!(
            state.available_in_board_or_fallback().get(),
            0b1_1111_1111 << (state.forced_board() * consts::N_CELLS)
        );
    }
}
