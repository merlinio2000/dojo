use crate::{
    board::Board,
    consts,
    types::{BoardState, Index, Move},
};

#[derive(Debug, Clone, Copy)]
pub struct BoardMoveFinder {
    is_available_bitset: BoardState,
    index: Index,
}

impl Iterator for BoardMoveFinder {
    type Item = Move;

    fn next(&mut self) -> Option<Self::Item> {
        while self.is_available_bitset != 0 {
            let is_available = self.is_available_bitset & 1 == 1;
            let curr_index = self.index;
            self.is_available_bitset >>= 1;
            self.index += 1;
            if is_available {
                return Some(Board::to_2d_idx(curr_index));
            }
        }
        None
    }
}

impl BoardMoveFinder {
    const AVAILABLE_MASKS: [u32; consts::N_CELLS] = {
        let mut masks = [0; consts::N_CELLS];
        let mut idx = 0;
        while idx != masks.len() {
            masks[idx] = 0b10 << (idx * consts::CELL_BITS);
            idx += 1;
        }

        masks
    };

    pub fn new(board_state: BoardState) -> Self {
        let mut is_available_bitset: BoardState = 0;

        for is_available in Self::AVAILABLE_MASKS
            .iter()
            .rev()
            .map(|mask| (mask & board_state) == 0)
        {
            is_available_bitset = (is_available_bitset << 1) + is_available as u32;
        }

        Self {
            index: 0,
            is_available_bitset,
        }
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
        let board = Board::from_matrix([
            [Free, Free, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let move_iter = BoardMoveFinder::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 2);
        assert_eq!(moves[0], (0, 0));
        assert_eq!(moves[1], (0, 1));

        let board =
            Board::from_matrix([[Free, Free, Free], [Free, Free, Free], [Free, Free, Free]]);
        let move_iter = BoardMoveFinder::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 9);
        for (idx, move_) in moves.iter().enumerate() {
            assert_eq!(*move_, Board::to_2d_idx(idx))
        }

        let board = Board::from_matrix([
            [Player2, Player1, Player1],
            [Player2, Player1, Player2],
            [Player1, Player1, Player2],
        ]);
        let move_iter = BoardMoveFinder::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 0);

        let board = Board::from_matrix([
            [Player1, Free, Free],
            [Free, Free, Free],
            [Free, Free, Free],
        ]);
        let move_iter = BoardMoveFinder::new(board.0);
        let moves = move_iter.collect::<Vec<_>>();
        assert_eq!(moves.len(), 8);
        assert!(moves.iter().all(|move_| *move_ != (0, 0)));
    }
}
