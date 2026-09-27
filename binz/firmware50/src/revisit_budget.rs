//! Generation-owned quota for a future bounded level-revisit scheduler.
//! Not wired into a motor binary. Callers must serialize live admission and
//! reservation with acceptance/stop; this pure state does not provide a lock.

/// One initial request plus the existing four overdue rescue requests.
pub const REQUESTS: u8 = 5;
/// Keep every deadline inside the unambiguous half of a u16 microsecond clock.
pub const MAX_AVERAGE_US: u32 = (0x8000 - 1) / 3;

/// One authoritative instance must own a generation's quota. Do not recreate
/// it on an expiry/refusal; new construction is a new transaction only.
/// ```compile_fail
/// let b = firmware50::revisit_budget::Budget::new(1, 80);
/// let duplicate = b.clone();
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Budget {
    generation: u32,
    average_us: u32,
    used: u8,
    active: bool,
}

impl Budget {
    /// A generation is a transaction identity, not the repeating sector 1..6.
    /// Start only after canceling pending work from the previous generation.
    #[must_use]
    pub const fn new(generation: u32, average_us: u32) -> Self {
        Self {
            generation, average_us, used: 0,
            active: average_us >= crate::revisit::AVERAGE_MIN_US
                && average_us <= MAX_AVERAGE_US,
        }
    }

    pub fn cancel(&mut self) { self.active = false; }

    /// First eligible elapsed microsecond; equality at the original boundary
    /// is refused. A failed live admission does not consume an attempt.
    #[must_use]
    pub const fn next_due(&self, generation: u32) -> Option<u32> {
        if !self.active || generation != self.generation || self.used >= REQUESTS {
            return None;
        }
        let half = self.average_us >> 1;
        Some(if self.used == 0 { half + 1 } else {
            self.average_us + half * self.used as u32 + 1
        })
    }

    /// Reserve under the same exclusion as the *actual* pend. `live_admitted`
    /// must include owner, current generation/mux, line/pending and live level
    /// checks. This never accepts a crossing or commands a commutation.
    /// `elapsed_us` must retain elapsed time, not a pre-wrapped u16 difference:
    /// a full timer wrap cannot be detected after the caller discards it.
    /// Overdue calls may spend multiple eligible requests at one timestamp;
    /// this is the existing quota rule, not a minimum inter-request spacing.
    pub fn reserve(&mut self, generation: u32, elapsed_us: u32, live_admitted: bool) -> bool {
        if !live_admitted || elapsed_us >= 0x8000 { return false; }
        match self.next_due(generation) {
            Some(due) if elapsed_us >= due => { self.used += 1; true }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadlines_preserve_strict_existing_quota_including_odd_intervals() {
        for average in crate::revisit::AVERAGE_MIN_US..=MAX_AVERAGE_US {
            let mut b = Budget::new(9, average);
            let half = average >> 1;
            for n in 0..REQUESTS {
                let boundary = if n == 0 { half } else { average + half * u32::from(n) };
                assert_eq!(b.next_due(9), Some(boundary + 1));
                assert!(!b.reserve(9, boundary, true));
                assert!(!b.reserve(9, boundary + 1, false));
                assert!(b.reserve(9, boundary + 1, true));
            }
            assert_eq!(b.next_due(9), None);
            assert!(!b.reserve(9, 32767, true));
        }
    }

    #[test]
    fn competing_callers_share_one_budget_not_one_each() {
        for first in [false, true] {
            let mut b = Budget::new(12, 80);
            let mut requests = [0u8; 2];
            for time in 0..400 {
                for caller in [usize::from(first), usize::from(!first)] {
                    requests[caller] += u8::from(b.reserve(12, time, true));
                }
            }
            assert_eq!(requests[0] + requests[1], REQUESTS);
        }
    }

    #[test]
    fn stale_generation_stop_and_wrap_cannot_restore_quota() {
        let mut b = Budget::new(u32::MAX, 80);
        assert!(!b.reserve(0, 100, true));
        assert!(b.reserve(u32::MAX, 41, true));
        b.cancel();
        assert!(!b.reserve(u32::MAX, 200, true));
        let mut next = Budget::new(0, 80);
        assert!(!next.reserve(u32::MAX, 200, true));
        assert!(next.reserve(0, 41, true));
        for elapsed in [0x8000, 0x8001, 0x1_0000, u32::MAX] {
            assert!(!next.reserve(0, elapsed, true));
        }
    }

    #[test]
    fn invalid_estimates_fail_closed_without_overflow() {
        for average in [0, 31, MAX_AVERAGE_US + 1, u32::MAX] {
            let mut b = Budget::new(0, average);
            assert_eq!(b.next_due(0), None);
            assert!(!b.reserve(0, 32767, true));
        }
    }

    #[test]
    fn overdue_calls_can_spend_due_slots_but_never_exceed_quota() {
        let mut b = Budget::new(7, 80);
        for _ in 0..REQUESTS { assert!(b.reserve(7, 300, true)); }
        for _ in 0..100 { assert!(!b.reserve(7, 300, true)); }
    }
}
