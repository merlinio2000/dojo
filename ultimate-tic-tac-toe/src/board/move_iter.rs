use crate::{
    bitmagic,
    types::{BoardState, Index},
};

#[derive(Debug, Clone, Copy)]
pub struct BoardMoveIter {
    is_available_bitset: BoardState,
}

impl Iterator for BoardMoveIter {
    type Item = Index;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_available_bitset == 0 {
            None
        } else {
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
            let available_cell_index = bitmagic::trailing_zeros(self.is_available_bitset);

            self.is_available_bitset &= self.is_available_bitset - 1;
            Some(available_cell_index as Index)
        }
    }
}

impl BoardMoveIter {
    pub fn new(board_state: BoardState) -> Self {
        let available_bits_contiguous = bitmagic::get_availble_bits_contiguous(board_state);
        Self {
            is_available_bitset: available_bits_contiguous,
        }
    }
}

#[cfg(test)]
mod board_move_iter_test {
    use crate::{
        board::{Board, move_iter::BoardMoveIter},
        types::{CellState, Index},
    };

    #[test]
    fn test_available_moves() {
        use CellState::{Free, Player1, Player2};
        let board = Board::from_matrix([
            [Free, Free, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let move_iter = BoardMoveIter::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], 0);
        assert_eq!(moves[1], 3);

        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        let move_iter = BoardMoveIter::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 9);
        for (idx, move_) in moves.iter().enumerate() {
            assert_eq!(*move_, idx as Index)
        }

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let move_iter = BoardMoveIter::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 0);

        let board = Board::from_matrix([
            [Player1, Free, Free],
            [Free, Free, Free],
            [Free, Free, Free],
        ]);
        let move_iter = BoardMoveIter::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 8);
        assert!(moves.iter().all(|move_| *move_ != 0));
    }
}
