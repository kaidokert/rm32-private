//! Effective high-duty wait, local to binz's COM A/B.
//! Mirrors AM32: advance=(ci*level)>>6; wait=(ci>>1)-advance.
pub const LEVEL: u32 = if cfg!(feature = "bench-reverse-advance26-override") { 26 } else { 24 };
pub const fn wait(ci: u32) -> u32 {
    (ci >> 1).saturating_sub((ci * LEVEL) >> 6)
}

#[cfg(test)]
mod tests {
    use super::{wait, LEVEL};

    #[test]
    fn matches_reference_arithmetic_over_timer_interval_range() {
        for ci in 0..=65_535u32 {
            assert_eq!(wait(ci), (ci >> 1).saturating_sub((ci * LEVEL) >> 6));
            assert!(wait(ci) <= ci >> 1);
        }
        assert_eq!(wait(160), if LEVEL == 26 { 15 } else { 20 });
    }
}
