//! Corroborating low-bus evidence for the foreground current foldback actuator.
//! The independent full-block bus guard remains authoritative for stopping.
pub struct Evidence<const N: u32, const EARLY: u32> {
    scans: u32,
    lows: u32,
    early_sent: bool,
}

impl<const N: u32, const EARLY: u32> Evidence<N, EARLY> {
    pub const fn new() -> Self {
        assert!(N > 0 && EARLY > 0 && EARLY <= N);
        Self {
            scans: 0,
            lows: 0,
            early_sent: false,
        }
    }
    /// Returns (early reduction request, completed-block low count).
    pub fn observe(&mut self, low: bool) -> (bool, Option<u32>) {
        self.scans += 1;
        self.lows += low as u32;
        let early = !self.early_sent && self.lows >= EARLY;
        self.early_sent |= early;
        if self.scans == N {
            let lows = self.lows;
            *self = Self::new();
            (early, Some(lows))
        } else {
            (early, None)
        }
    }
    pub fn applied_reduction(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_once_and_exact_block_count() {
        let mut e = Evidence::<5, 3>::new();
        assert_eq!(e.observe(true), (false, None));
        assert_eq!(e.observe(false), (false, None));
        assert_eq!(e.observe(true), (false, None));
        assert_eq!(e.observe(true), (true, None));
        assert_eq!(e.observe(true), (false, Some(4)));
        assert_eq!(e.observe(false), (false, None));
    }
    #[test]
    fn reduction_discards_mixed_duty_evidence() {
        let mut e = Evidence::<5, 3>::new();
        assert_eq!(e.observe(true), (false, None));
        assert_eq!(e.observe(true), (false, None));
        e.applied_reduction();
        assert_eq!(e.observe(true), (false, None));
        assert_eq!(e.observe(false), (false, None));
    }
}
