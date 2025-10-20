use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct HyperLogLog {
    bits: u8,
    seeds: Vec<u64>,
    hashes: Vec<u64>,
}

impl HyperLogLog {
    pub fn new(bits: u8) -> Self {
        let m = 1 << bits; // 2^bits registers
        Self {
            bits,
            seeds: vec![0; m],
            hashes: vec![u64::MAX; m],
        }
    }

    pub fn add(&mut self, seed: u64, hash: u64) {
        let m = 1 << self.bits;
        let mask = m - 1;
        let register = (hash & mask) as usize;

        if hash < self.hashes[register] {
            self.hashes[register] = hash;
            self.seeds[register] = seed;
        }
    }

    pub fn count(&self) -> f64 {
        let m = (1 << self.bits) as f64;
        let alpha = match self.bits {
            4 => 0.673,
            5 => 0.697,
            6 => 0.709,
            _ => 0.7213 / (1.0 + 1.079 / m),
        };

        let sum: f64 = self
            .hashes
            .iter()
            .map(|&hash| {
                if hash == u64::MAX {
                    0.0
                } else {
                    // Remove the bits used for register selection
                    let remaining = hash >> self.bits;
                    // Count leading zeros in the remaining bits and add 1 for rho (1-indexed position)
                    // We subtract self.bits because remaining is still in a 64-bit value with extra leading zeros
                    let rho = remaining.leading_zeros() - self.bits as u32 + 1;
                    2_f64.powi(-(rho as i32))
                }
            })
            .sum();

        if sum == 0.0 {
            0.0
        } else {
            alpha * m * m / sum
        }
    }
}
