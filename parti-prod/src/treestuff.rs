use std::fmt::Display;

use im::{vector, Vector};
use itertools::Itertools;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Parti {
    values: Vector<u64>,
    remaining: u64,
}

impl Display for Parti {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parti {{ values: {:?}, remaining: {} }}",
            self.values, self.remaining
        )
    }
}

impl Parti {
    fn from(target_n: u64, value: u64) -> Self {
        debug_assert!(target_n >= value);
        Self {
            values: vector![value],
            remaining: target_n - value,
        }
    }

    fn prod(&self) -> u64 {
        self.values.iter().product()
    }

    fn take(&self, value: u64) -> Self {
        debug_assert!(self.fits(value));
        let mut new_vals = self.values.clone();
        new_vals.push_back(value);
        Self {
            remaining: self.remaining - value,
            values: new_vals,
        }
    }

    fn fits(&self, value: u64) -> bool {
        debug_assert_ne!(value, 0);
        value <= self.remaining
    }
}

pub fn parti_prod(n: u64) -> (Vec<Vec<u64>>, u64) {
    let mut partis = Vec::new();
    let mut candidates: Vec<Parti>;
    for i in (1..=n).rev() {
        partis.push(Parti::from(n, i));

        // TODO: re-use the Vector
        candidates = partis
            .iter()
            .cloned()
            .filter(|parti| parti.fits(i))
            .collect();

        while !candidates.is_empty() {
            // TODO: re-use the Vector
            let mut next_candidates = Vec::new();
            for cand in &candidates {
                next_candidates.push(cand.take(i));
            }
            partis.extend(next_candidates.clone());
            candidates = next_candidates
                .into_iter()
                .filter(|parti| parti.fits(i))
                .collect();
        }
    }

    let partis = partis
        .iter()
        .filter(|p| p.remaining == 0)
        .sorted_unstable_by(|left, right| right.values.cmp(&left.values));

    // for ele in partis {
    //     println!("{}", ele);
    // }

    let mut max_partis = partis.max_set_by_key(|parti| parti.prod());
    max_partis.sort_unstable_by_key(|parti| parti.values.len());

    (
        max_partis
            .iter()
            .map(|parti| parti.values.iter().cloned().collect_vec())
            .collect_vec(),
        max_partis.get(0).map(|p| p.prod()).unwrap_or(n),
    )
}
