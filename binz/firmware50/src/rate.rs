//! Comparator-storm cutoff: at most `LIMIT` COMP interrupts per 1 ms bucket.
//!
//! Transcribed from `binz/examples/support/irq_dispatch.rs::Rate` and its use
//! in the qualified image's closed-loop COMP handler
//! (`core_bench.rs:4062-4107`, `bench-lean-core`): more than
//! `reverse_rate_limit()` calls in one fixed 1 ms bucket calls
//! `powered_timer::irq_storm()`, which removes the bridge. The oracle carries
//! `bench-reverse-irq-cap64`, so its limit is 64, and its qualified runs never
//! reached it.
//!
//! Why firmware50 needs it now (LAB_NOTEBOOK E061): the comparator's
//! hysteresis goes back to the reference's 0, and E051 showed what HYST 0 does
//! to an unlocked loop -- an interrupt storm that starved the protection scan.
//! The reference's answer to that is this cutoff, not hysteresis.
//!
//! The reference keeps the count **report-only** during its driven stage
//! (`driven_irq_live.rs`: "A short burst at 65/ms previously killed an
//! otherwise bounded normal restart"), so the caller chooses `observe` there
//! and `hit` in the closed loop.

/// Calls per bucket above which the closed loop stops (`bench-reverse-irq-cap64`).
pub const LIMIT: u16 = 64;

/// Handover window during which the count is telemetry, not a stop (E107).
///
/// binz's own answer to 65/ms bursts at startup (`AGENTS.md` E724 "Still
/// stopped 65 dispatches/ms/ratecap64" -> E724d "startup rate observe/report
/// only ... per-call 50us watchdogs retained" -> E767 "cap is report-only in
/// the normal lean path ... Real 50us handler-overrun stop and independent
/// command/feedback/watchdogs remain"), and in code
/// (`examples/support/driven_irq_live.rs:216-222`): "Dispatch count is
/// telemetry, not a safety boundary. A short burst at 65/ms previously killed
/// an otherwise bounded normal restart. Actual runaway/latency protection is the
/// measured 50us handler budget below, plus the independent
/// command/feedback/watchdog deadlines." binz bounds its startup window at
/// 40 ms ("Caller bounds the complete drive to40ms", `driven_seed.rs:106`).
/// firmware50's handover storms all fell 3-10 ms after transfer (E093-E105).
/// After the window the count cutoff applies unchanged.
pub const HANDOVER_OBSERVE_US: u32 = 40_000;

/// Is the count cutoff enforced `since_transfer_us` after the handover?
#[must_use]
pub const fn cap_enforced(since_transfer_us: u32) -> bool {
    since_transfer_us >= HANDOVER_OBSERVE_US
}

/// Per-call COMP handler budget, µs (binz `driven_irq_live.rs:310-318`:
/// `if elapsed > 50 { overruns += 1; stop(); }`). Always enforced.
pub const HANDLER_BUDGET_US: u32 = 50;

/// Did one COMP call overrun its budget?
#[must_use]
pub const fn handler_overrun(elapsed_us: u32) -> bool {
    elapsed_us > HANDLER_BUDGET_US
}
/// Buckets after which `settled_peak` starts counting (E094): the transfer
/// transient is over by then, so the two peaks separate "at handover" from
/// "in steady running". Report-only; the cutoff ignores it.
pub const SETTLE_BUCKETS: u16 = 50;
/// Bucket length in microseconds of the 16-bit raw timeline.
pub const BUCKET_US: u16 = 1_000;

/// Fixed 1 ms buckets, not a lifetime quota. Once the limit is exceeded the
/// verdict latches: a storm is a stop, not a warning that clears itself.
///
/// `LOCATE` (E095) selects whether the bucket that set the peak and the
/// post-transfer "settled" peak are tracked. It is a type-level policy so the
/// closed-loop COMP root can use `Rate<false>` and pay nothing for it (E094's
/// always-on version grew COMP's longest path from 1230 to 1253 cycles, over
/// its 1245 ceiling); the cutoff itself is identical in both.
#[derive(Copy, Clone, Debug)]
pub struct Rate<const LOCATE: bool = false> {
    start: u16,
    count: u16,
    peak: u16,
    failed: bool,
    /// Buckets opened so far (the first call opens bucket 1), saturating.
    buckets: u16,
    /// The bucket in which `peak` was last raised.
    peak_bucket: u16,
    /// Peak over buckets after `SETTLE_BUCKETS`.
    settled_peak: u16,
}

impl<const LOCATE: bool> Default for Rate<LOCATE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const LOCATE: bool> Rate<LOCATE> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            start: 0,
            count: 0,
            peak: 0,
            failed: false,
            buckets: 0,
            peak_bucket: 0,
            settled_peak: 0,
        }
    }

    /// Count one call at raw time `now` without judging it.
    pub fn observe(&mut self, now: u16) {
        if now.wrapping_sub(self.start) >= BUCKET_US || (LOCATE && self.buckets == 0) {
            self.start = now;
            self.count = 0;
            if LOCATE {
                self.buckets = self.buckets.saturating_add(1);
            }
        }
        self.count = self.count.saturating_add(1);
        if self.count > self.peak {
            self.peak = self.count;
            if LOCATE {
                self.peak_bucket = self.buckets;
            }
        }
        if LOCATE && self.buckets > SETTLE_BUCKETS && self.count > self.settled_peak {
            self.settled_peak = self.count;
        }
    }

    /// Count one call and judge it. False once the limit has been exceeded,
    /// and false forever after.
    pub fn hit(&mut self, now: u16) -> bool {
        if self.failed {
            return false;
        }
        self.observe(now);
        self.failed = self.count > LIMIT;
        !self.failed
    }

    #[must_use]
    pub const fn peak(&self) -> u16 {
        self.peak
    }

    #[must_use]
    pub const fn failed(&self) -> bool {
        self.failed
    }

    /// The bucket (1 = the first) in which the peak was reached (0 unless
    /// `LOCATE`). In the closed loop every millisecond has calls, so this is ~ms
    /// since the reset.
    #[must_use]
    pub const fn peak_bucket(&self) -> u16 {
        self.peak_bucket
    }

    /// Peak over buckets after the first `SETTLE_BUCKETS` (0 unless `LOCATE`).
    #[must_use]
    pub const fn settled_peak(&self) -> u16 {
        self.settled_peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference `report_only_rate_does_not_latch_or_wrap`.
    #[test]
    fn observing_never_latches_or_wraps() {
        let mut r = Rate::<false>::new();
        for _ in 0..70_000 {
            r.observe(0);
        }
        assert_eq!(r.peak(), u16::MAX);
        assert!(!r.failed());
        assert!(r.hit(1_000));
    }

    /// Reference `sustained_normal_rate_has_no_lifetime_quota`: one call per
    /// 70 µs forever is fine.
    #[test]
    fn a_sustained_normal_rate_has_no_quota() {
        let mut r = Rate::<false>::new();
        for n in 0..2_000u32 {
            assert!(r.hit((n * 70) as u16));
        }
        assert!(r.peak() <= 15);
    }

    /// Reference `storm_stops_and_latches_across_bucket_boundary`.
    #[test]
    fn a_storm_stops_and_stays_stopped() {
        let mut r = Rate::<false>::new();
        for _ in 0..64 {
            assert!(r.hit(65_530));
        }
        assert!(!r.hit(65_531));
        assert!(!r.hit(1_500));
        assert_eq!(r.peak(), 65);
    }

    /// Exactly the limit is allowed; one more is the stop.
    #[test]
    fn the_limit_is_inclusive() {
        let mut r = Rate::<false>::new();
        for k in 0..LIMIT {
            assert!(r.hit(10 + k), "call {k}");
        }
        assert!(!r.hit(10 + LIMIT));
    }

    /// A new bucket starts the count again (before any latch).
    #[test]
    fn buckets_reset_the_count() {
        let mut r = Rate::<false>::new();
        for _ in 0..60 {
            assert!(r.hit(0));
        }
        for _ in 0..60 {
            assert!(r.hit(1_000));
        }
        assert!(!r.failed());
        assert_eq!(r.peak(), 60);
    }

    /// The first call opens bucket 1 wherever the raw clock is (it used to
    /// open at raw 0 implicitly, which is the same unless `now` < 1000).
    #[test]
    fn the_peak_is_located_and_the_settled_peak_is_separate() {
        let mut r = Rate::<true>::new();
        // Bucket 1 (the transfer): 40 calls. Buckets 2..=60: 10 calls each,
        // except bucket 55 with 20.
        for _ in 0..40 {
            assert!(r.hit(5_000));
        }
        for b in 2..=60u16 {
            let t = 5_000u16.wrapping_add((b - 1) * 1_000);
            let n = if b == 55 { 20 } else { 10 };
            for _ in 0..n {
                assert!(r.hit(t));
            }
        }
        assert_eq!(r.peak(), 40);
        assert_eq!(r.peak_bucket(), 1);
        assert_eq!(r.settled_peak(), 20);
        assert!(!r.failed());
    }

    /// The unlocated policy tracks nothing extra but cuts off identically.
    #[test]
    fn the_unlocated_policy_cuts_off_identically() {
        let mut a = Rate::<false>::new();
        let mut b = Rate::<true>::new();
        for k in 0..200u16 {
            let t = 3_000 + (k / 70) * 1_000;
            assert_eq!(a.hit(t), b.hit(t), "call {k}");
        }
        assert_eq!(a.peak(), b.peak());
        assert_eq!(a.failed(), b.failed());
        assert_eq!(a.peak_bucket(), 0);
        assert_eq!(a.settled_peak(), 0);
    }

    /// binz's startup policy: observe for exactly the window, enforce after.
    #[test]
    fn the_count_is_observed_during_the_handover_window_then_enforced() {
        assert!(!cap_enforced(0));
        assert!(!cap_enforced(HANDOVER_OBSERVE_US - 1));
        assert!(cap_enforced(HANDOVER_OBSERVE_US));
        assert!(cap_enforced(u32::MAX));
        assert_eq!(HANDOVER_OBSERVE_US, 40_000);
    }

    /// binz `finish_duration`: 50 is inside the budget, 51 is an overrun
    /// (its own RATESTOP self-test cases 0, 49, 50, 51, 65535).
    #[test]
    fn the_handler_budget_is_the_references() {
        for (elapsed, over) in [(0u32, false), (49, false), (50, false), (51, true), (65_535, true)] {
            assert_eq!(handler_overrun(elapsed), over, "{elapsed}");
        }
        assert_eq!(HANDLER_BUDGET_US, 50);
    }

    #[test]
    fn constants_are_the_references() {
        assert_eq!(LIMIT, 64);
        assert_eq!(BUCKET_US, 1_000);
    }
}
