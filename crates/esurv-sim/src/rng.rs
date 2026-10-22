//! Seedable random numbers, so a run can be replayed exactly from its seed.
//!
//! The generator is ours (SplitMix64); the rules rely only on the *distributions* it gives,
//! not on its bit stream.


#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform in `[lo, hi)`.
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }

    /// Index chosen in proportion to `weights` (non-negative). `None` when all are zero.
    pub fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            return None;
        }
        let mut roll = self.range(0.0, total);
        for (i, &w) in weights.iter().enumerate() {
            if roll < w {
                return Some(i);
            }
            roll -= w;
        }
        weights.iter().rposition(|&w| w > 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn weighted_never_picks_zero_weight() {
        let mut r = Rng::new(1);
        for _ in 0..1000 {
            assert_ne!(r.weighted(&[1.0, 0.0, 2.0]), Some(1));
        }
        assert_eq!(r.weighted(&[0.0, 0.0]), None);
    }
}
