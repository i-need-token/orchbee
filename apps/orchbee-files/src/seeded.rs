//! The deterministic generator this crate's property tests draw their cases from, and the seed a
//! variable may replace, so a failure reproduces from the seed it prints.

/// SplitMix64, so reproducing a case needs no external generator.
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
    pub(crate) const fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// A value below `bound`; 0 for a bound of 0.
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }
}

/// The seed the environment variable `variable` states, in decimal or as `0x`-prefixed
/// hexadecimal; otherwise `default`.
pub(crate) fn seed(variable: &str, default: u64) -> u64 {
    std::env::var(variable)
        .ok()
        .and_then(|value| {
            let value = value.trim();
            value.strip_prefix("0x").map_or_else(
                || value.parse().ok(),
                |hex| u64::from_str_radix(hex, 16).ok(),
            )
        })
        .unwrap_or(default)
}
