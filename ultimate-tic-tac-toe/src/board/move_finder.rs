use crate::{
    board::Board,
    consts,
    types::{BoardState, Move},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct BoardMoveFinder {
    // TODO PERF: probably 0 pad for SIMD
    moves_buf: [Move; consts::N_CELLS],
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

    pub fn new() -> Self {
        Self::default()
    }

    pub fn available_moves(&mut self, board_state: BoardState) -> &[Move] {
        let is_available_results = Self::AVAILABLE_MASKS.map(|mask| (mask & board_state) == 0);
        let mut available_moves_idx = 0;
        for (cell_index, is_available) in is_available_results.into_iter().enumerate() {
            if is_available {
                self.moves_buf[available_moves_idx] = Board::to_2d_idx(cell_index);
                available_moves_idx += 1;
            }
        }
        &self.moves_buf[..available_moves_idx]
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
