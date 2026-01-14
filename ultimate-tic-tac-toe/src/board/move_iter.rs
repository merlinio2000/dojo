use crate::{bitmagic, util::BoardMajorBitset};

#[derive(Debug, Clone, Copy)]
pub struct BoardMoveIterU128 {
    is_available_bitset: BoardMajorBitset,
}

impl Iterator for BoardMoveIterU128 {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_available_bitset.is_empty() {
            None
        } else {
            let available_cell_index =
                bitmagic::trailing_zeros_u128(self.is_available_bitset.get());

            self.is_available_bitset.unset_least_signifiact_one();
            Some(available_cell_index as u8)
        }
    }
}

impl BoardMoveIterU128 {
    pub fn new(is_available_bitset: BoardMajorBitset) -> Self {
        Self {
            is_available_bitset,
        }
    }
}
