fn main() {
    println!("Hello, world!");
}

fn scramble_less_branching(haystack: &[u8], target: &[u8]) -> bool {
    assert!(haystack.iter().all(|hay| hay.is_ascii_lowercase()));
    let mut lookup = [0u32; 26];
    for lower_target in target {
        lookup[(*lower_target - b'a') as usize] += 1;
    }
    for lower_hay in haystack {
        let lookup_ref = &mut lookup[(*lower_hay - b'a') as usize];
        *lookup_ref = u32::saturating_sub(*lookup_ref, 1);
        // TODO: check how this is SIMD'd
        if lookup.iter().all(|count| *count == 0) {
            return true;
        }
    }
    false
}

// I expect the sum of 26 4byte ints to be basically instant thanks to SIMD,
// lets see the tradeoff with the extra branching
fn scramble_track_found(haystack: &[u8], target: &[u8]) -> bool {
    assert!(haystack.iter().all(|hay| hay.is_ascii_lowercase()));
    let mut lookup = [0u32; 26];
    for lower_target in target {
        lookup[(*lower_target - b'a') as usize] += 1;
    }
    let mut found = 0usize;
    for lower_hay in haystack {
        let lookup_ref = &mut lookup[(*lower_hay - b'a') as usize];
        if *lookup_ref != 0 {
            *lookup_ref -= 1;
            found += 1;
        }
        if found == target.len() {
            return true;
        }
    }
    false
}
