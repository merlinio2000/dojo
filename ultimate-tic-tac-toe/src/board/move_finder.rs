use std::mem::MaybeUninit;

use crate::{
    bitmagic, consts,
    types::{BoardState, Index},
};

#[derive(Debug, Clone, Copy)]
pub struct BoardMoveFinder {
    moves_buf: [MaybeUninit<Index>; consts::N_CELLS as usize],
}

impl Default for BoardMoveFinder {
    fn default() -> Self {
        Self {
            moves_buf: [MaybeUninit::uninit(); consts::N_CELLS as usize],
        }
    }
}

impl BoardMoveFinder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_single(&mut self, available_move: Index) -> &[Index] {
        self.moves_buf[0] = MaybeUninit::new(available_move);
        // safety: only the first element is made available which was initialized above
        unsafe { std::mem::transmute::<&[MaybeUninit<Index>], &[Index]>(&self.moves_buf[..1]) }
    }

    // col-major
    pub fn available_moves(&mut self, board_state: BoardState) -> &[Index] {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            self.available_moves_inner_1d_x86_bmi2(board_state)
        }
        #[cfg(not(target_arch = "x86_64"))]
        self.available_moves_inner_1d(board_state)
    }

    /// # Safety
    /// requires x86 extensions
    /// - bmi1
    /// - bmi2
    ///
    /// credit to https://www.chessprogramming.org/BMI2
    /// NOTE: features are enabled here to allow inlining of bitmagic functions actually using them
    #[cfg(target_arch = "x86_64")]
    #[target_feature(enable = "bmi1")]
    #[target_feature(enable = "bmi2")]
    pub unsafe fn available_moves_inner_1d_x86_bmi2(
        &mut self,
        board_state: BoardState,
    ) -> &[Index] {
        self.available_moves_inner_1d(board_state)
    }

    // TODO: verify this is actually inlined, rustc behaves differntly when target_feature is in
    // the mix
    #[inline(always)]
    pub fn available_moves_inner_1d(&mut self, board_state: BoardState) -> &[Index] {
        let mut available_bits_contiguous = bitmagic::get_availble_bits_contiguous(board_state);
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
            let available_cell_index = bitmagic::trailing_zeros(available_bits_contiguous);
            self.moves_buf[found_moves_idx] = MaybeUninit::new(available_cell_index as Index);

            found_moves_idx += 1;
            available_bits_contiguous &= available_bits_contiguous - 1;
        }
        // safety: all items up to `found_moves_idx` have been initialised in the while
        unsafe {
            std::mem::transmute::<&[MaybeUninit<Index>], &[Index]>(
                &self.moves_buf[..found_moves_idx],
            )
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        board::{Board, move_finder::BoardMoveFinder},
        types::{CellState, Index},
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
        assert_eq!(moves[0], 0);
        assert_eq!(moves[1], 3);

        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        let moves = move_iter.available_moves(board.0);
        assert_eq!(moves.len(), 9);
        for (idx, move_) in moves.iter().enumerate() {
            assert_eq!(*move_, idx as Index)
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
        assert!(moves.iter().all(|move_| *move_ != 0));
    }
}
