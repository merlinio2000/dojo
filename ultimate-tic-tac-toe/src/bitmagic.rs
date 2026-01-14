use crate::{board::move_iter::BoardMoveIterU128, util::BoardMajorBitset};

pub const fn trailing_zeros_u128(bits: u128) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        trailing_zeros_u128_x86_bmi1(bits)
    }
    #[cfg(not(target_arch = "x86_64"))]
    trailing_zeros_u128_fallback(bits)
}

/// safety: make sure this is run on x86-64 with bmi1 enabled
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi1")]
const unsafe fn trailing_zeros_u128_x86_bmi1(bits: u128) -> u32 {
    // NOTE: the bmi1 feature ensures this is compiled to tzcnt
    bits.trailing_zeros()
}

#[allow(unused)]
const fn trailing_zeros_u128_fallback(bits: u128) -> u32 {
    bits.trailing_zeros()
}

pub const fn count_ones_u128(bits: u128) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        count_ones_u128_x86_popcnt(bits)
    }
    #[cfg(not(target_arch = "x86_64"))]
    bits.count_ones()
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi1,bmi2,sse3,sse4.2,popcnt")]
const fn count_ones_u128_x86_popcnt(bits: u128) -> u32 {
    bits.count_ones()
}

pub const fn count_ones_u32(bits: u32) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        count_ones_u32_x86_popcnt(bits)
    }
    #[cfg(not(target_arch = "x86_64"))]
    bits.count_ones()
}
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi1,bmi2,sse3,sse4.2,popcnt")]
const fn count_ones_u32_x86_popcnt(bits: u32) -> u32 {
    bits.count_ones()
}

pub fn index_of_nth_setbit(x: u128, n: u8) -> u32 {
    // safety: this may only be run on modern x86 cpus, main asserts feature is available
    #[cfg(target_arch = "x86_64")]
    unsafe {
        index_of_nth_setbit_x64_bmi(x, n)
    }
    #[cfg(not(target_arch = "x86_64"))]
    index_of_nth_setbit_fallback(x, n)
}

/// # Safety
/// requires cpu features popcnt,bmi,bmi2
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "popcnt,bmi1,bmi2")]
pub unsafe fn index_of_nth_setbit_x64_bmi(x: u128, n: u8) -> u32 {
    let lo: u64 = x as u64;
    let hi: u64 = (x >> 64) as u64;

    let lo_count = core::arch::x86_64::_popcnt64(lo as i64) as u8;

    let (part, base, n) = if n < lo_count {
        (lo, 0u32, n)
    } else {
        (hi, 64u32, n - lo_count)
    };

    let sel = core::arch::x86_64::_pdep_u64(1u64 << n, part);

    base + core::arch::x86_64::_tzcnt_u64(sel) as u32
}
pub fn index_of_nth_setbit_fallback(x: u128, n: u8) -> u32 {
    // safety: right here we actually dont care about the validity of the board
    BoardMoveIterU128::new(unsafe { BoardMajorBitset::new_unchecked(x) })
        .nth(n as usize)
        .expect("not enough bits set to get the n-th index") as u32
}

#[cfg(test)]
mod test {
    use crate::bitmagic::index_of_nth_setbit;

    #[test]
    fn test_index_of_nth_setbit() {
        for shift in [0, 16, 32, 48, 64, 80, 96, 112] {
            let msg = format!("with a shift of {shift}");
            let bits = 0b1u128;
            assert_eq!(index_of_nth_setbit(bits, 0), 0, "{msg}");

            let bits = 0b10u128;
            assert_eq!(index_of_nth_setbit(bits, 0), 1, "{msg}");

            let bits = 0b11u128;
            assert_eq!(index_of_nth_setbit(bits, 0), 0, "{msg}");
            assert_eq!(index_of_nth_setbit(bits, 1), 1, "{msg}");

            let bits = 0b110u128;
            assert_eq!(index_of_nth_setbit(bits, 0), 1, "{msg}");
            assert_eq!(index_of_nth_setbit(bits, 1), 2, "{msg}");

            let bits = 0b1101u128;
            assert_eq!(index_of_nth_setbit(bits, 0), 0, "{msg}");
            assert_eq!(index_of_nth_setbit(bits, 1), 2, "{msg}");
            assert_eq!(index_of_nth_setbit(bits, 2), 3, "{msg}");
        }

        let bits = 0b1 | (0b1 << 64);
        assert_eq!(index_of_nth_setbit(bits, 0), 0);
        assert_eq!(index_of_nth_setbit(bits, 1), 64);
    }
}
