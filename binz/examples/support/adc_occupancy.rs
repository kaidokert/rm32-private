//! Bounded early-window ADC launch coverage, not aperture or current accuracy.
//! Feed only physical TIM1 counter brackets around ADSTART, with a monotonic
//! commutation generation and TIM17 elapsed time spanning those same reads.
//! Counter reset/wrap, preemption, and bin-straddling launches remain rejected.
//! Marginal PWM bins do NOT prove joint sector coverage or long-run uniformity.
pub const LIMIT: u8 = 192;
pub const PERIOD: u16 = 6400;
pub const BINS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Coverage {
    pub bins: [[u8; BINS]; 3],
    pub attempted: [u8; 3],
    pub rejected: [u8; 3],
}
impl Coverage {
    pub const fn new() -> Self {
        Self {
            bins: [[0; BINS]; 3],
            attempted: [0; 3],
            rejected: [0; 3],
        }
    }
    /// Fixed first LIMIT attempts per channel; no replacement of rejected data.
    /// `elapsed_us` must bracket counter reads, not merely the ADC register write.
    /// `generation_before/after` must bracket that same interval.
    /// Launches can subsequently fail conversion or cross a guard stop; these
    /// counts must never be substituted for successful current samples.
    pub fn push(
        &mut self,
        phase: usize,
        before: u16,
        after: u16,
        elapsed_us: u16,
        generation_before: u32,
        generation_after: u32,
    ) -> bool {
        if phase >= 3 || self.attempted[phase] >= LIMIT {
            return false;
        }
        self.attempted[phase] += 1;
        let valid = before < PERIOD
            && after < PERIOD
            && after >= before
            && elapsed_us <= 2
            && after - before <= 128
            && generation_before == generation_after
            && before / 800 == after / 800;
        if !valid {
            self.rejected[phase] += 1;
            return false;
        }
        self.bins[phase][before as usize / 800] += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fits_existing_static_budget_and_never_wraps_counts() {
        assert_eq!(core::mem::size_of::<Coverage>(), 30);
        let mut c = Coverage::new();
        for _ in 0..1000 {
            c.push(0, 1, 10, 1, 0, 0);
        }
        assert_eq!(c.attempted[0], 192);
        assert_eq!(c.bins[0][0], 192);
        assert_eq!(c.rejected, [0; 3]);
    }
    #[test]
    fn rejects_reset_wrap_preemption_boundary_and_bad_phase() {
        let mut c = Coverage::new();
        for (a, b, us, g0, g1) in [
            (1, 10, 1, 0, 1),
            (6390, 4, 1, 0, 0),
            (1, 10, 101, 0, 0),
            (799, 801, 1, 0, 0),
            (1, 200, 2, 0, 0),
            (6400, 6400, 0, 0, 0),
        ] {
            assert!(!c.push(1, a, b, us, g0, g1));
        }
        assert_eq!(c.attempted[1], 6);
        assert_eq!(c.rejected[1], 6);
        let saved = c;
        assert!(!c.push(3, 1, 10, 1, 0, 0));
        assert_eq!(c, saved);
    }
    #[test]
    fn retains_bias_and_refusals_instead_of_filling_holes() {
        let mut c = Coverage::new();
        for i in 0..192 {
            c.push(0, 10, 20, 1, 0, 0);
            c.push(1, (i % 8) * 800, (i % 8) * 800 + 10, 1, 0, 0);
            c.push(2, 799, 801, 1, 0, 0);
        }
        assert_eq!(c.bins[0], [192, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(c.bins[1], [24; 8]);
        assert_eq!(c.rejected[2], 192);
        assert!(!c.push(2, 1, 10, 1, 0, 0));
        assert_eq!(c.bins[2], [0; 8]);
    }
}
