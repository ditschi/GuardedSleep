//! Seeded PRNG (splitmix64). The only source of randomness in a simulation run, so a seed fully
//! determines latencies, jitter and fault timing.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ 0x5851_F42D_4C95_7F2D)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.next_u64() % (hi - lo + 1)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        if p >= 1.0 {
            return true;
        }
        if p <= 0.0 {
            return false;
        }
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }

    /// 128 random bits as 32 lowercase hex characters (an attempt id).
    pub fn aid(&mut self) -> String {
        format!("{:016x}{:016x}", self.next_u64(), self.next_u64())
    }
}
