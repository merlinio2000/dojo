use rand::Rng;

mod internal {
    use crate::consts;
    use rand::SeedableRng;

    const SEED: u64 = 0xfeebdaed_deadbeef;
    use std::cell::UnsafeCell;
    thread_local! {
        static MOVE_DISTRIBUTION: rand::distr::Uniform<u8> = rand::distr::Uniform::new_inclusive(0, consts::N_CELLS_NESTED as u8 - 1).unwrap();
        static RNG: UnsafeCell<rand::rngs::SmallRng> = UnsafeCell::new(rand::rngs::SmallRng::seed_from_u64(SEED))
    }

    /// it is forbidden, and impossible in safe rust, to smuggle a (mutable) reference to the rng
    /// out of this function
    pub fn do_with_rng<T, F: FnOnce(&mut rand::rngs::SmallRng) -> T>(f: F) -> T {
        // safety: mutable references to RNG can't escape anywhere and is only read temporarily
        RNG.with(|rng| f(unsafe { rng.get().as_mut().unwrap() }))
    }
}

pub fn rand_in_move_range_exclusive(max_exclusive: u8) -> u8 {
    internal::do_with_rng(|rng| rng.random_range(0..max_exclusive))
}
