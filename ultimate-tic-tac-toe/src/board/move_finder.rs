use std::mem::MaybeUninit;

use crate::{
    board::Board,
    consts,
    types::{BoardState, Index, Move},
};

pub(crate) fn get_availble_bits_contiguous(board_state: BoardState) -> BoardState {
    // 'compress' the board layout so we only have every second bit (= !is_available) left
    //  and placed contiguously at the start of the result
    if is_x86_feature_detected!("bmi2") {
        // safety:
        // - check this feature is available in main
        // - no memory accesses or unexpected mutations just bit magic on `board_state`
        let not_available_bits_contiguous =
            unsafe { core::arch::x86_64::_pext_u32(board_state, consts::ALL_CELLS_OCCUPIED_MASK) };
        !not_available_bits_contiguous & 0b1_1111_1111
    } else {
        let mut occupied_bits = board_state & consts::ALL_CELLS_OCCUPIED_MASK;
        debug_assert_eq!(
            size_of_val(&occupied_bits),
            4,
            "bit magic is only implemented for up to 32bit ints"
        );
        // (1010_1010_1010_1010 | 0101_0101_0101_0101) & 0011_0011_0011_0011 = 0011_0011_0011_0011
        occupied_bits = (occupied_bits | (occupied_bits >> 1)) & 0x33333333;
        // (0011_0011_0011_0011 | 0000_1100_1100_1100) & 0000_1111_0000_1111 = 0000_1111_0000_1111
        occupied_bits = (occupied_bits | (occupied_bits >> 2)) & 0x0f0f0f0f;
        // (0000_1111_0000_1111 | 0000_0000_1111_0000) & 0000_0000_1111_1111 = 0000_0000_1111_1111
        occupied_bits = (occupied_bits | (occupied_bits >> 4)) & 0x00ff00ff;
        // (0000_0000_1111_1111 | 0000_0000_0000_0000) & 1111_1111_1111_1111 = 0000_0000_1111_1111
        occupied_bits = (occupied_bits | (occupied_bits >> 8)) & 0x0000ffff;

        occupied_bits
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoardMoveFinder {
    moves_buf: [MaybeUninit<Move>; consts::N_CELLS],
}

impl Default for BoardMoveFinder {
    fn default() -> Self {
        Self {
            moves_buf: [MaybeUninit::uninit(); consts::N_CELLS],
        }
    }
}

impl BoardMoveFinder {
    pub fn new() -> Self {
        Self::default()
    }

    // credit to https://www.chessprogramming.org/BMI2
    pub fn available_moves(&mut self, board_state: BoardState) -> &[Move] {
        let mut available_bits_contiguous = get_availble_bits_contiguous(board_state);
        let mut found_moves_idx = 0;
        // almost branchless come @ me :)
        while available_bits_contiguous != 0 {
            //   0bX100   & 0bX011   = 0bX000
            //   0bXY01   & 0bXY00   = 0bXY00
            //   0bXY10   & 0bXY01   = 0bXY00
            //   0bXY11   & 0bXY10   = 0bXY10
            //
            // example:
            //   0b0_0000_1010 -> trailing zeroes = 1
            // & 0b0_0000_1001
            // = 0b0_0000_1000 -> trailing zeroes = 3
            // & 0b0_0000_0111
            // = 0b0_0000_0000 -> finished
            let available_cell_index = available_bits_contiguous.trailing_zeros();
            self.moves_buf[found_moves_idx] =
                MaybeUninit::new(Board::to_2d_idx(available_cell_index as Index));

            found_moves_idx += 1;
            available_bits_contiguous &= available_bits_contiguous - 1;
        }
        // safety: all items up to `found_moves_idx` have been initialised in the while
        unsafe { std::mem::transmute(&self.moves_buf[..found_moves_idx]) }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        board::{Board, move_finder::BoardMoveFinder},
        types::CellState,
    };

    #[test]
    fn test_available_moves() {
        use CellState::{Free, Player1, Player2};
        let mut move_iter = BoardMoveFinder::new();
        let board = Board::from_matrix([
            [Free, Free, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], (0, 0));
        assert_eq!(moves[1], (0, 1));

        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 9);
        for (idx, move_) in moves.iter().enumerate() {
            assert_eq!(*move_, Board::to_2d_idx(idx))
        }

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 0);

        let board = Board::from_matrix([
            [Player1, Free, Free],
            [Free, Free, Free],
            [Free, Free, Free],
        ]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 8);
        assert!(moves.iter().all(|move_| *move_ != (0, 0)));
    }
}
