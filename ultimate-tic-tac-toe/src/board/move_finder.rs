use crate::{
    board::Board,
    consts,
    types::{BoardState, Index, Move},
};

pub(crate) fn get_availble_bits_contiguous(board_state: BoardState) -> BoardState {
    // 'compress' the board layout so we only have every second bit (= !is_available) left
    //  and placed contiguously at the start of the result
    // safety:
    // - check this feature is available in main
    // - no memory accesses or unexpected mutations just bit magic on `board_state`
    let not_available_bits_contiguous =
        unsafe { core::arch::x86_64::_pext_u32(board_state, consts::ALL_CELLS_OCCUPIED_MASK) };
    !not_available_bits_contiguous & 0b1_1111_1111
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BoardMoveFinder {
    moves_buf: [Move; consts::N_CELLS],
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
            self.moves_buf[found_moves_idx] = Board::to_2d_idx(available_cell_index as Index);

            found_moves_idx += 1;
            available_bits_contiguous &= available_bits_contiguous - 1;
        }
        &self.moves_buf[..found_moves_idx]
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
