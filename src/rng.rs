//! A small, fast, seedable random stream: xoshiro256++ seeded through
//! SplitMix64.  Every run gets its own stream from (seed, run index), so a
//! result never depends on how the runs were spread over threads.

pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// The stream for run `index` of a batch started from `seed`.
    pub fn for_run(seed: u64, index: u64) -> Self {
        let mut x = seed.wrapping_mul(1_000_003).wrapping_add(index);
        Rng { s: [splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x), splitmix64(&mut x)] }
    }

    fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = (s[0].wrapping_add(s[3])).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in 0..n.
    pub fn below(&mut self, n: usize) -> usize {
        (self.unit() * n as f64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_run_has_its_own_repeatable_stream() {
        let draws = |seed, run| {
            let mut rng = Rng::for_run(seed, run);
            (0..5).map(|_| rng.unit()).collect::<Vec<_>>()
        };
        assert_eq!(draws(7, 3), draws(7, 3));
        assert_ne!(draws(7, 3), draws(7, 4));
        let mut rng = Rng::for_run(1, 0);
        assert!((0..10_000).map(|_| rng.below(12)).all(|r| r < 12));
        assert!((0..10_000).map(|_| rng.unit()).all(|u| (0.0..1.0).contains(&u)));
    }
}
