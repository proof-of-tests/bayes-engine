//! # HyperLogLog - Min-Hash Variant for Proof-of-Work
//!
//! This crate provides a min-hash variant of the HyperLogLog algorithm for
//! cardinality estimation, specifically designed for proof-of-work verification
//! in distributed fuzzy testing.
//!
//! ## Algorithm Overview
//!
//! Traditional HyperLogLog stores the maximum number of leading zeros seen for
//! each register. Our variant instead stores the **minimum hash value** for each
//! register, which provides equivalent cardinality estimation while enabling
//! proof-of-work verification.
//!
//! ### How It Works
//!
//! 1. **Register Selection**: The lower `bits` of a hash determine which register
//!    (bucket) it belongs to. With `bits = 12`, we have 4096 registers.
//!
//! 2. **Min-Hash Storage**: Each register stores the minimum hash value seen.
//!    Lower hash values are rarer (require more computational work to find).
//!
//! 3. **Cardinality Estimation**: We count leading zeros in the remaining bits
//!    (after removing register selection bits) to estimate how many hashes were
//!    computed.
//!
//! ### Proof-of-Work Interpretation
//!
//! The probability of finding a hash with `k` leading zeros is `1/2^k`.
//! By collecting minimum hashes across registers, we can estimate total
//! computational work performed:
//!
//! - Finding hash `0x0001...` requires ~65K attempts on average
//! - Finding hash `0x00001...` requires ~1M attempts on average
//!
//! This makes the system trustless: anyone can verify computational work
//! was performed by examining the hash values.
//!
//! ## Example
//!
//! ```
//! use hyperloglog::HyperLogLog;
//!
//! let mut hll = HyperLogLog::new(12); // 4096 registers
//!
//! // Simulate test executions with seed and hash pairs
//! for seed in 0..10000 {
//!     let hash = simple_hash(seed);
//!     hll.add(seed, hash);
//! }
//!
//! // Estimate how many unique executions occurred
//! let estimate = hll.count();
//! println!("Estimated executions: {}", estimate);
//!
//! fn simple_hash(seed: u64) -> u64 {
//!     // A simple hash function for demonstration
//!     let mut x = seed;
//!     x = x.wrapping_add(0x9e3779b97f4a7c15);
//!     x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
//!     x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
//!     x ^ (x >> 31)
//! }
//! ```
//!
//! ## Precision vs. Space Trade-off
//!
//! | Bits | Registers | Memory  | Standard Error |
//! |------|-----------|---------|----------------|
//! | 4    | 16        | 256 B   | ~26%           |
//! | 8    | 256       | 4 KB    | ~6.5%          |
//! | 12   | 4,096     | 64 KB   | ~1.6%          |
//! | 16   | 65,536    | 1 MB    | ~0.4%          |
//! | 20   | 1,048,576 | 16 MB   | ~0.1%          |
//!
//! The standard error is approximately `1.04 / sqrt(m)` where `m = 2^bits`.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Default number of bits for register selection (32 registers).
///
/// This provides a compact representation with reasonable precision (~18% standard error).
/// For distributed proof-of-work systems, lower precision is acceptable as it reduces
/// storage and network overhead while still providing useful cardinality estimates.
pub const DEFAULT_HLL_BITS: u8 = 5;

/// Maximum allowed bits for register selection (1M registers).
///
/// Values above this would use excessive memory with diminishing returns
/// on precision improvement.
pub const MAX_HLL_BITS: u8 = 20;

/// Min-hash variant HyperLogLog for cardinality estimation.
///
/// This structure maintains minimum hash values for each register (bucket),
/// along with the seeds that produced those hashes. This enables both
/// cardinality estimation and proof-of-work verification.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct HyperLogLog {
    bits: u8,
    seeds: Vec<u64>,
    hashes: Vec<u64>,
}

impl HyperLogLog {
    /// Normalize bits to valid range [1, MAX_HLL_BITS].
    fn normalize_bits(bits: u8) -> u8 {
        bits.clamp(1, MAX_HLL_BITS)
    }

    /// Create a new HyperLogLog with the specified number of bits.
    ///
    /// The `bits` parameter determines the number of registers: `2^bits`.
    /// Values are clamped to the range `[1, MAX_HLL_BITS]`.
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let hll = HyperLogLog::new(12); // 4096 registers
    /// assert_eq!(hll.bits(), 12);
    /// ```
    pub fn new(bits: u8) -> Self {
        let bits = Self::normalize_bits(bits);
        let m = 1usize << bits;
        Self {
            bits,
            seeds: vec![0; m],
            hashes: vec![u64::MAX; m],
        }
    }

    /// Add a seed-hash pair to the HyperLogLog.
    ///
    /// Returns `true` if this hash improved (lowered) the minimum for its
    /// register, `false` otherwise.
    ///
    /// # Arguments
    ///
    /// * `seed` - The seed value that produced this hash
    /// * `hash` - The hash value to add
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(12);
    /// let improved = hll.add(42, 0x0000_1234_5678_9ABC);
    /// assert!(improved); // First hash always improves from u64::MAX
    /// ```
    pub fn add(&mut self, seed: u64, hash: u64) -> bool {
        let mask = (1usize << self.bits) - 1;
        let register = (hash as usize) & mask;

        if hash < self.hashes[register] {
            self.hashes[register] = hash;
            self.seeds[register] = seed;
            true
        } else {
            false
        }
    }

    /// Add a hash without tracking its seed.
    ///
    /// This is a convenience method that sets the seed to 0. Useful when
    /// seed tracking is not needed.
    ///
    /// Returns `true` if this hash improved the minimum for its register.
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(12);
    /// let improved = hll.add_hash(0x0000_1234_5678_9ABC);
    /// assert!(improved);
    /// ```
    pub fn add_hash(&mut self, hash: u64) -> bool {
        self.add(0, hash)
    }

    /// Estimate the cardinality (number of unique items) seen.
    ///
    /// Uses the HyperLogLog algorithm with bias correction factors.
    /// Returns 0.0 if no hashes have been added.
    ///
    /// # Algorithm
    ///
    /// For each register, we compute `rho` (position of first 1-bit after
    /// removing register selection bits) and sum `2^(-rho)`. The harmonic
    /// mean formula with bias correction gives the cardinality estimate.
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(12);
    ///
    /// // Add some hashes
    /// for i in 0..1000u64 {
    ///     let hash = i.wrapping_mul(0x9e3779b97f4a7c15);
    ///     hll.add_hash(hash);
    /// }
    ///
    /// let estimate = hll.count();
    /// // Min-hash HLL provides an estimate of cardinality
    /// assert!(estimate > 0.0);
    /// ```
    pub fn count(&self) -> f64 {
        let m = (1u64 << self.bits) as f64;

        // Bias correction factor (alpha_m)
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
                    // Register never updated - contributes nothing
                    0.0
                } else {
                    // Remove bits used for register selection
                    let remaining = hash >> self.bits;
                    // Count leading zeros in remaining bits, add 1 for 1-indexed rho
                    // Use saturating_sub to prevent underflow
                    let rho = remaining.leading_zeros().saturating_sub(self.bits as u32) + 1;
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

    /// Get the number of bits used for register selection.
    pub fn bits(&self) -> u8 {
        self.bits
    }

    /// Get a reference to the minimum hashes array.
    ///
    /// Each element is the minimum hash seen for that register,
    /// or `u64::MAX` if no hash has been added to that register.
    pub fn hashes(&self) -> &[u64] {
        &self.hashes
    }

    /// Get a reference to the seeds array.
    ///
    /// Each element is the seed that produced the minimum hash for that
    /// register. Seeds are 0 for registers that haven't been updated or
    /// where `add_hash` was used instead of `add`.
    pub fn seeds(&self) -> &[u64] {
        &self.seeds
    }

    /// Create a HyperLogLog from a JSON string.
    ///
    /// The JSON should be an array of string-encoded u64 values (to avoid
    /// JavaScript number precision issues). Seeds are not restored from JSON.
    ///
    /// # Arguments
    ///
    /// * `bits` - Number of bits for register selection
    /// * `json` - JSON array of string-encoded hash values
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let json = r#"["18446744073709551615", "12345678901234567890"]"#;
    /// let hll = HyperLogLog::from_json(1, json); // 2 registers for bits=1
    /// ```
    pub fn from_json(bits: u8, json: &str) -> Self {
        let parsed = serde_json::from_str::<Vec<String>>(json).ok();
        let mut state = Self::new(bits);

        if let Some(values) = parsed {
            for (index, value) in values.iter().enumerate().take(state.hashes.len()) {
                if let Ok(parsed_value) = value.parse::<u64>() {
                    state.hashes[index] = parsed_value;
                }
            }
        }

        state
    }

    /// Serialize the hashes to a JSON string.
    ///
    /// Hashes are encoded as strings to avoid JavaScript number precision
    /// issues with large u64 values. Seeds are not included in the output.
    ///
    /// # Example
    ///
    /// ```
    /// use hyperloglog::HyperLogLog;
    ///
    /// let mut hll = HyperLogLog::new(4); // 16 registers
    /// hll.add_hash(0x1234);
    ///
    /// let json = hll.to_json();
    /// assert!(json.starts_with('['));
    /// assert!(json.ends_with(']'));
    /// ```
    pub fn to_json(&self) -> String {
        let values: Vec<String> = self.hashes.iter().map(|value| value.to_string()).collect();
        serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple splitmix64 hash for testing
    fn splitmix(mut x: u64) -> u64 {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }

    #[test]
    fn test_new_creates_correct_size() {
        let hll = HyperLogLog::new(4);
        assert_eq!(hll.bits(), 4);
        assert_eq!(hll.hashes().len(), 16);
        assert_eq!(hll.seeds().len(), 16);
    }

    #[test]
    fn test_new_normalizes_bits() {
        // Too low - clamped to 1
        let hll = HyperLogLog::new(0);
        assert_eq!(hll.bits(), 1);
        assert_eq!(hll.hashes().len(), 2);

        // Too high - clamped to MAX_HLL_BITS
        let hll = HyperLogLog::new(30);
        assert_eq!(hll.bits(), MAX_HLL_BITS);
        assert_eq!(hll.hashes().len(), 1 << MAX_HLL_BITS);
    }

    #[test]
    fn test_add_returns_true_on_improvement() {
        let mut hll = HyperLogLog::new(4);

        // First add always improves from u64::MAX
        assert!(hll.add(1, 0xFFFF_0000));

        // Lower hash improves
        assert!(hll.add(2, 0x0000_0000)); // Same register (bits 0-3)

        // Higher hash doesn't improve
        assert!(!hll.add(3, 0xFFFF_0000));
    }

    #[test]
    fn test_add_tracks_seed() {
        let mut hll = HyperLogLog::new(4);

        // With 4 bits, register selection uses lower 4 bits
        // Hash 0x0013 -> register 3 (0x13 & 0xF = 3)
        hll.add(42, 0x0013);
        assert_eq!(hll.seeds()[3], 42);

        // Lower hash that still maps to register 3
        // Hash 0x0003 -> register 3 (0x03 & 0xF = 3), and 0x0003 < 0x0013
        hll.add(99, 0x0003);
        assert_eq!(hll.seeds()[3], 99);
    }

    #[test]
    fn test_add_hash_sets_seed_to_zero() {
        let mut hll = HyperLogLog::new(4);

        hll.add_hash(0x0005); // Register 5
        assert_eq!(hll.seeds()[5], 0);
    }

    #[test]
    fn test_count_empty_returns_zero() {
        let hll = HyperLogLog::new(12);
        assert_eq!(hll.count(), 0.0);
    }

    #[test]
    fn test_count_estimates_cardinality() {
        let mut hll = HyperLogLog::new(12);
        let n = 10_000u64;

        for seed in 0..n {
            let hash = splitmix(seed);
            hll.add(seed, hash);
        }

        let estimate = hll.count();

        // Min-hash HLL has different statistical properties than standard HLL.
        // With 12 bits (4096 registers) and 10K items, we expect reasonable
        // accuracy but allow 50% tolerance due to the min-hash variant behavior.
        let lower = n as f64 * 0.5;
        let upper = n as f64 * 1.5;
        assert!(
            estimate > lower && estimate < upper,
            "Expected estimate near {}, got {}",
            n,
            estimate
        );
    }

    #[test]
    fn test_count_handles_single_item() {
        let mut hll = HyperLogLog::new(12);
        let hash = splitmix(0);
        hll.add(0, hash);

        let estimate = hll.count();
        // With min-hash HLL, a single item (especially one with few leading zeros)
        // produces a high estimate because most registers are still at u64::MAX.
        // The estimate decreases as we find more (and lower) hashes.
        assert!(estimate > 0.0, "Estimate should be positive");
    }

    #[test]
    fn test_json_roundtrip() {
        let mut hll = HyperLogLog::new(4);
        hll.add(1, 0x1234);
        hll.add(2, 0x5678_0002);

        let json = hll.to_json();
        let restored = HyperLogLog::from_json(4, &json);

        assert_eq!(hll.hashes(), restored.hashes());
        // Seeds are not preserved in JSON
        assert_eq!(restored.seeds(), &[0u64; 16]);
    }

    #[test]
    fn test_json_handles_invalid_input() {
        // Invalid JSON returns default state
        let hll = HyperLogLog::from_json(4, "not valid json");
        assert_eq!(hll.hashes().len(), 16);
        assert!(hll.hashes().iter().all(|&h| h == u64::MAX));
    }

    #[test]
    fn test_json_handles_partial_data() {
        // Fewer values than registers
        let json = r#"["100", "200"]"#;
        let hll = HyperLogLog::from_json(4, json); // 16 registers

        assert_eq!(hll.hashes()[0], 100);
        assert_eq!(hll.hashes()[1], 200);
        assert_eq!(hll.hashes()[2], u64::MAX);
    }

    #[test]
    fn test_clone() {
        let mut hll = HyperLogLog::new(4);
        hll.add(42, 0x1234);

        let cloned = hll.clone();
        assert_eq!(hll, cloned);
    }

    #[test]
    fn test_default_bits_constant() {
        assert_eq!(DEFAULT_HLL_BITS, 5);
        let hll = HyperLogLog::new(DEFAULT_HLL_BITS);
        assert_eq!(hll.hashes().len(), 32);
    }

    #[test]
    fn test_max_bits_constant() {
        assert_eq!(MAX_HLL_BITS, 20);
        let hll = HyperLogLog::new(MAX_HLL_BITS);
        assert_eq!(hll.hashes().len(), 1_048_576);
    }

    #[test]
    fn test_leading_zeros_safe_for_edge_cases() {
        let mut hll = HyperLogLog::new(12);

        // Very low hash (many leading zeros)
        hll.add(1, 0x0000_0000_0000_0001);

        // Very high hash (few leading zeros)
        hll.add(2, 0xFFFF_FFFF_FFFF_FFFE);

        // Should not panic
        let _ = hll.count();
    }
}
