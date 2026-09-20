//! Pure admission policy for one level-sensitive retry of a missed COMP edge.

#[derive(Clone, Copy)]
pub struct Inputs {
    pub owner: bool,
    pub active: bool,
    pub coast_reference: bool,
    pub software_masked: bool,
    pub hardware_enabled: bool,
    pub average_half_us: u32,
    pub interval_half_us: u32,
    pub pending: bool,
    pub post_level: bool,
    pub same_step_retried: bool,
    pub allow_high_speed: bool,
}

#[inline]
pub const fn admit(v: Inputs) -> bool {
    v.owner
        && v.active
        && v.coast_reference
        && !v.software_masked
        && v.hardware_enabled
        && v.average_half_us >= 64
        && (v.allow_high_speed || v.average_half_us >= 1000)
        && v.interval_half_us > (v.average_half_us >> 1)
        && !v.pending
        && v.post_level
        && !v.same_step_retried
}

/// Opt-in reverse A/B boundary; does not affect the normal admission rule.
pub const fn below_high_duty_cutoff(duty_tenths: u32) -> bool {
    below_duty_cutoff(duty_tenths, 350)
}

/// Foreground-published duty boundary for a matched high-duty A/B.
pub const fn below_duty_cutoff(duty_tenths: u32, cutoff_tenths: u32) -> bool {
    duty_tenths < cutoff_tenths
}

#[cfg(test)]
mod cutoff_tests {
    use super::{below_duty_cutoff, below_high_duty_cutoff};

    #[test]
    fn reverse_revisit_cutoff_is_exactly_35_percent() {
        assert!(below_high_duty_cutoff(349));
        assert!(!below_high_duty_cutoff(350));
        assert!(!below_high_duty_cutoff(500));
        assert!(below_duty_cutoff(499, 500));
        assert!(!below_duty_cutoff(500, 500));
        assert!(!below_duty_cutoff(501, 500));
        assert!(below_duty_cutoff(479, 480));
        assert!(!below_duty_cutoff(480, 480));
    }
}
