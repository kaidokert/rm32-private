//! Shared-vector source qualification only; no BEMF filtering or timing policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Dispatch,
    SoftwareMasked,
    HardwareMasked,
    NotPending,
}
/// Deferred startup input can resume only under the same live owner and the
/// reference's strict half-interval gate. Zero average is never admission.
pub fn deferred_ready(owner: bool, deferred: bool, count: u32, average: u32) -> bool {
    owner && deferred && average != 0 && count > (average >> 1)
}
pub fn qualify(software_masked: bool, hardware_enabled: bool, pending: bool) -> Decision {
    if software_masked {
        Decision::SoftwareMasked
    } else if !hardware_enabled {
        Decision::HardwareMasked
    } else if !pending {
        Decision::NotPending
    } else {
        Decision::Dispatch
    }
}
/// Fixed 1ms buckets, not a lifetime call quota. The independent ADC-age and
/// guard-tick deadlines still bound starvation across bucket boundaries.
pub struct Rate {
    start: u16,
    count: u16,
    pub peak: u16,
    failed: bool,
}
impl Rate {
    pub const fn new() -> Self {
        Self {
            start: 0,
            count: 0,
            peak: 0,
            failed: false,
        }
    }
    pub fn observe(&mut self, now: u16) {
        if now.wrapping_sub(self.start) >= 1000 {
            self.start = now;
            self.count = 0;
        }
        self.count = self.count.saturating_add(1);
        self.peak = self.peak.max(self.count);
    }
    pub fn hit(&mut self, now: u16) -> bool {
        self.hit_limit(now, 64)
    }
    pub fn hit_limit(&mut self, now: u16, limit: u16) -> bool {
        if self.failed {
            return false;
        }
        self.observe(now);
        self.failed = self.count > limit;
        !self.failed
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deferred_input_requires_live_owner_and_strict_gate() {
        assert!(!deferred_ready(true, true, 833, 1666));
        assert!(deferred_ready(true, true, 834, 1666));
        assert!(!deferred_ready(false, true, 834, 1666));
        assert!(!deferred_ready(true, false, 834, 1666));
        assert!(!deferred_ready(true, true, 834, 0));
    }
    #[test]
    fn report_only_rate_does_not_latch_or_wrap() {
        let mut r = Rate::new();
        for _ in 0..70000 {
            r.observe(0);
        }
        assert_eq!(r.peak, u16::MAX);
        assert!(!r.failed);
        assert!(r.hit(1000));
    }
    #[test]
    fn only_enabled_pending_source_dispatches() {
        for software in [false, true] {
            for hardware in [false, true] {
                for pending in [false, true] {
                    assert_eq!(
                        qualify(software, hardware, pending) == Decision::Dispatch,
                        !software && hardware && pending
                    );
                }
            }
        }
    }
    #[test]
    fn sustained_normal_rate_has_no_lifetime_quota() {
        let mut r = Rate::new();
        for n in 0..2000 {
            assert!(r.hit((n * 70) as u16));
        }
        assert!(r.peak <= 15);
    }
    #[test]
    fn storm_stops_and_latches_across_bucket_boundary() {
        let mut r = Rate::new();
        for _ in 0..64 {
            assert!(r.hit(65530));
        }
        assert!(!r.hit(65531));
        assert!(!r.hit(1500));
        assert_eq!(r.peak, 65);
    }
    #[test]
    fn low_speed_diagnostic_cap_bounds_pwm_synchronous_dispatches() {
        let mut r = Rate::new();
        for n in 0..12 {
            assert!(r.hit_limit(n * 40, 12));
        }
        assert!(!r.hit_limit(480, 12));
        assert!(!r.hit_limit(1500, 12));
        assert_eq!(r.peak, 13);
    }
    #[test]
    fn rate_limit_is_not_a_guard_service_deadline() {
        // A hypothetical continuously pending, higher-priority handler taking
        // 20us can occupy the core indefinitely at only50calls/ms. This tests
        // the limiter, NOT NVIC scheduling or measured handler duration.
        let mut r = Rate::new();
        for elapsed in (0..100_000u32).step_by(20) {
            assert!(r.hit(elapsed as u16));
        }
        assert_eq!(r.peak, 50);
        // Even a fast burst exceeds the200us guard gap before the65th hit.
        let mut r = Rate::new();
        for elapsed in (0..=210u16).step_by(5) {
            assert!(r.hit(elapsed));
        }
    }
}
