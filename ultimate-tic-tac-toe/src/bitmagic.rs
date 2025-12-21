use crate::consts;

pub(crate) const fn trailing_zeros(bits: u32) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        trailing_zeros_x86_bmi1(bits)
    }
    #[cfg(not(target_arch = "x86_64"))]
    trailing_zeros_fallback(board_state)
}

/// safety: make sure this is run on x86-64 with bmi1 enabled
// #[target_feature(enable = "bmi1")]
#[cfg(target_arch = "x86_64")]
#[inline(always)]
const unsafe fn trailing_zeros_x86_bmi1(bits: u32) -> u32 {
    // NOTE: the bmi1 feature ensures this is compiled to tzcnt
    bits.trailing_zeros()
}

#[allow(unused)]
const fn trailing_zeros_fallback(bits: u32) -> u32 {
    bits.trailing_zeros()
}

/// safety: make sure this is run on x86-64 with bmi2 enabled
// #[target_feature(enable = "bmi2")]
#[cfg(target_arch = "x86_64")]
#[inline(always)]
unsafe fn get_availble_bits_contiguous_x86_bmi2(board_state: u32) -> u32 {
    use crate::consts;

    debug_assert!(is_x86_feature_detected!("bmi2"));
    unsafe { core::arch::x86_64::_pext_u32(!board_state, consts::ALL_CELLS_OCCUPIED_MASK) }
}

#[allow(unused)]
pub fn get_availble_bits_contiguous_fallback(board_state: u32) -> u32 {
    let mut available_bits = !board_state & consts::ALL_CELLS_OCCUPIED_MASK;
    debug_assert_eq!(
        size_of_val(&available_bits),
        4,
        "bit magic is only implemented for up to 32bit ints"
    );
    available_bits >>= 1;
    // (01_0101_0101_0101_0101 | 00_1010_1010_1010_1010) & 11_0011_0011_0011_0011 = 01_0011_0011_0011_0011
    available_bits = (available_bits | (available_bits >> 1)) & 0x33333333;
    // (01_0011_0011_0011_0011 | 00_1100_1100_1100_1100) & 11_0000_1111_0000_1111 = 01_0000_1111_0000_1111
    available_bits = (available_bits | (available_bits >> 2)) & 0x0f0f0f0f;
    // (01_0000_1111_0000_1111 | 00_0001_0000_1111_0000) & 11_0000_0000_1111_1111 = 01_0000_0000_1111_1111
    available_bits = (available_bits | (available_bits >> 4)) & 0x00ff00ff;
    // (01_0000_0000_1111_1111 | 00_0000_0001_0000_0000) & 00_1111_1111_1111_1111 = 00_0000_0001_1111_1111
    available_bits = (available_bits | (available_bits >> 8)) & 0x0000ffff;

    available_bits
}

// 'compress' the board layout so we only have every second bit (= !is_available) left
//  and placed contiguously at the start of the result
pub(crate) fn get_availble_bits_contiguous(board_state: u32) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        get_availble_bits_contiguous_x86_bmi2(board_state)
    }
    #[cfg(not(target_arch = "x86_64"))]
    get_availble_bits_contiguous_fallback(board_state)
}

#[cfg(test)]
mod test {
    use crate::bitmagic::get_availble_bits_contiguous_fallback;

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_bit_compress_x86() {
        use crate::bitmagic::get_availble_bits_contiguous_x86_bmi2;
        let bits = unsafe { get_availble_bits_contiguous_x86_bmi2(0) };
        assert_eq!(bits, 0b1_1111_1111);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = unsafe { get_availble_bits_contiguous_x86_bmi2(0b10) };
        assert_eq!(bits, 0b1_1111_1110);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = unsafe { get_availble_bits_contiguous_x86_bmi2(0b10__1010_1010_1010_1010) };
        assert_eq!(bits, 0);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = unsafe { get_availble_bits_contiguous_x86_bmi2(0b10__1010_1010_1010_1000) };
        assert_eq!(bits, 0b1);
    }

    #[test]
    fn test_bit_compress_fallback() {
        let bits = get_availble_bits_contiguous_fallback(0);
        assert_eq!(bits, 0b1_1111_1111);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = get_availble_bits_contiguous_fallback(0b10);
        assert_eq!(bits, 0b1_1111_1110);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = get_availble_bits_contiguous_fallback(0b10__1010_1010_1010_1010);
        assert_eq!(bits, 0);

        #[allow(clippy::unusual_byte_groupings)]
        let bits = get_availble_bits_contiguous_fallback(0b10__1010_1010_1010_1000);
        assert_eq!(bits, 0b1);
    }
}
