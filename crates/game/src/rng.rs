//! Small seedable PRNG for the simulation (animal wandering, fleeing, drop
//! counts), so a run is replayable from a seed.
//! See docs/design/basic-gameplay.md section 1 rule 6 and section 8.3.
//!
//! Algorithm: a 64-bit counter state advanced by a fixed odd increment, with
//! each output produced by a two-round xor-shift/multiply mix of the state.
//! The increment and mix constants below are this project's own choices. The
//! stream is stable: changing any constant changes every replay.

/// Odd state increment per step.
const STEP: u64 = 0xA24B_AED4_963E_E407;
/// Seed whitening, so small seeds (0, 1, 2...) start far apart.
const SEED_MIX: u64 = 0x9FB2_1C65_1E98_DF25;
const MIX_A: u64 = 0xD6E8_FEB8_6659_FD93;
const MIX_B: u64 = 0xC2B2_AE3D_27D4_EB4F;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            state: seed.wrapping_mul(SEED_MIX) ^ (seed >> 29) ^ STEP,
        }
    }

    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(STEP);
        let mut z = self.state;
        z = (z ^ (z >> 32)).wrapping_mul(MIX_A);
        z = (z ^ (z >> 29)).wrapping_mul(MIX_B);
        (z ^ (z >> 32)) as u32
    }

    /// Uniform integer in `min..=max`. Swapped bounds are reordered.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
        let span = (hi as i64 - lo as i64 + 1) as u64; // 1..=2^32
        if span > u32::MAX as u64 {
            return self.next_u32() as i32; // the full i32 range
        }
        let span = span as u32;
        // Rejection sampling avoids modulo bias.
        let zone = u32::MAX - (u32::MAX % span + 1) % span;
        loop {
            let v = self.next_u32();
            if v <= zone {
                return (lo as i64 + (v % span) as i64) as i32;
            }
        }
    }

    /// True with probability 1 in `n`. `n` of 0 or 1 is always true.
    pub fn chance(&mut self, n: u32) -> bool {
        // Modulo bias is under n / 2^32, irrelevant for gameplay odds.
        n <= 1 || self.next_u32().is_multiple_of(n)
    }

    /// Direction angle in radians, in `[0, TAU)`.
    pub fn angle(&mut self) -> f32 {
        // 24 bits fit an f32 mantissa exactly, so the result stays below TAU.
        let unit = (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32;
        unit * std::f32::consts::TAU
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn different_seeds_differ() {
        for s in 0..50u64 {
            let mut a = Rng::new(s);
            let mut b = Rng::new(s + 1);
            let xs: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
            let ys: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
            assert_ne!(xs, ys, "seeds {s} and {}", s + 1);
        }
    }

    #[test]
    fn range_is_inclusive_and_in_bounds() {
        let mut r = Rng::new(7);
        let mut seen = [false; 4];
        for _ in 0..1000 {
            let v = r.range(2, 5);
            assert!((2..=5).contains(&v));
            seen[(v - 2) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "both ends and middle reached");
        assert_eq!(r.range(3, 3), 3);
        assert!((2..=5).contains(&r.range(5, 2)));
        for _ in 0..100 {
            let v = r.range(-3, 3);
            assert!((-3..=3).contains(&v));
            r.range(i32::MIN, i32::MAX);
            let w = r.range(i32::MAX - 1, i32::MAX);
            assert!(w >= i32::MAX - 1);
        }
    }

    #[test]
    fn range_is_roughly_uniform() {
        let mut r = Rng::new(99);
        let mut counts = [0u32; 6];
        let n = 60_000;
        for _ in 0..n {
            counts[r.range(0, 5) as usize] += 1;
        }
        let expect = n / 6;
        for c in counts {
            assert!(
                c > expect * 95 / 100 && c < expect * 105 / 100,
                "{counts:?}"
            );
        }
    }

    #[test]
    fn chance_one_in_n() {
        let mut r = Rng::new(5);
        assert!(r.chance(0) && r.chance(1));
        let hits = (0..40_000).filter(|_| r.chance(4)).count();
        assert!((9_000..11_000).contains(&hits), "{hits}");
    }

    #[test]
    fn angle_in_circle() {
        let mut r = Rng::new(1);
        for _ in 0..10_000 {
            let a = r.angle();
            assert!((0.0..std::f32::consts::TAU).contains(&a), "{a}");
        }
    }
}
