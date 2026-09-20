//! One-shot normal-startup restart policy. No gate or timer authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NotTracking,
    Used,
    TooLate,
    TooSoon,
}

/// Ordinary open-loop startup assumes a stopped rotor. The retained 250 ms
/// coast dump was insufficient: two of three restarts hit current protection
/// while trying to catch a still-moving rotor. Keep all outputs disabled for
/// one second from the tracking stop before starting the normal path.
pub const SETTLE_US: u32 = 1_000_000;
pub const RESUME_PERIOD_US: u32 = 2_000_000;
pub const RESUME_STEP_TENTHS: u32 = 50;

/// A live upward request during normal-start restoration changes the target,
/// never the already-published low duty. A lower request may still take effect
/// immediately through the guarded live writer.
pub const fn defer_upward(restarting: bool, current: u32, requested: u32) -> bool {
    restarting && requested > current
}

/// Foreground-only restoration of the last accepted live request. Startup and
/// the fresh BEMF handoff stay at their proven low duties; only after powered
/// ownership is established do we walk back toward the request. This policy
/// owns no timer or gate and cannot bypass the normal live-duty writer.
pub struct Resume {
    target: u32,
    current: u32,
    next_us: u32,
    steps: u32,
}
impl Resume {
    pub fn new(target: u32, current: u32, now_us: u32) -> Option<Self> {
        if !super::duty_envelope::contains(target) || !super::duty_envelope::contains(current) {
            return None;
        }
        Some(Self {
            target,
            current,
            next_us: now_us.wrapping_add(RESUME_PERIOD_US),
            steps: 0,
        })
    }
    pub fn due(&self, now_us: u32) -> Option<u32> {
        if self.current == self.target || now_us.wrapping_sub(self.next_us) >= 0x8000_0000 {
            return None;
        }
        Some(if self.target < self.current {
            self.target
        } else {
            self.current
                .saturating_add(RESUME_STEP_TENTHS)
                .min(self.target)
        })
    }
    pub fn applied(&mut self, duty: u32, now_us: u32) -> bool {
        if self.due(now_us) != Some(duty) {
            return false;
        }
        self.current = duty;
        self.steps += 1;
        self.next_us = now_us.wrapping_add(RESUME_PERIOD_US);
        true
    }
    /// Preserve the established two-second cadence and current rung when a
    /// host asks for a different upper target during restoration.
    pub fn retarget(&mut self, target: u32, published_current: u32) -> bool {
        if !super::duty_envelope::contains(target) || self.current != published_current {
            return false;
        }
        self.target = target;
        true
    }
    pub fn target(&self) -> u32 {
        self.target
    }
    pub fn current(&self) -> u32 {
        self.current
    }
    pub fn steps(&self) -> u32 {
        self.steps
    }
}

pub struct Policy {
    end_us: u32,
    used: bool,
}
impl Policy {
    pub fn new(powered_start_us: u32, window_us: u32) -> Option<Self> {
        if !(20_000..=600_000_000).contains(&window_us) {
            return None;
        }
        Some(Self {
            end_us: powered_start_us.checked_add(window_us)?,
            used: false,
        })
    }
    /// Consume the single attempt even on refusal. Reserve the complete
    /// autonomous startup plus200us shutdown/dispatch before the old deadline.
    pub fn restart(&mut self, now_us: u32, reason: u32, startup_us: u32) -> Result<u32, Refusal> {
        if self.used {
            return Err(Refusal::Used);
        }
        self.used = true;
        if reason != 8 {
            return Err(Refusal::NotTracking);
        }
        if startup_us < 4_700_000 {
            return Err(Refusal::TooSoon);
        }
        let powered_again = now_us
            .checked_add(startup_us)
            .and_then(|n| n.checked_add(200))
            .ok_or(Refusal::TooLate)?;
        self.end_us
            .checked_sub(powered_again)
            .filter(|&n| n >= 20_000)
            .ok_or(Refusal::TooLate)
    }
    pub fn used(&self) -> bool {
        self.used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_tracking_once_and_original_deadline() {
        let mut p = Policy::new(5_000_000, 30_000_000).unwrap();
        assert_eq!(p.restart(8_000_000, 8, 4_700_000), Ok(22_299_800));
        assert_eq!(p.restart(8_000_001, 8, 4_700_000), Err(Refusal::Used));
        assert!(p.used());
    }
    #[test]
    fn electrical_and_time_refusals_consume_attempt() {
        for (now, reason, startup, want) in [
            (8_000_000, 5, 4_700_000, Refusal::NotTracking),
            (8_000_000, 8, 4_699_999, Refusal::TooSoon),
            (30_290_000, 8, 4_700_000, Refusal::TooLate),
        ] {
            let mut p = Policy::new(5_000_000, 30_000_000).unwrap();
            assert_eq!(p.restart(now, reason, startup), Err(want));
            assert!(p.used());
        }
    }
    #[test]
    fn invalid_or_wrapping_session_refuses() {
        assert!(Policy::new(0, 19_999).is_none());
        assert!(Policy::new(u32::MAX - 10, 20_000).is_none());
    }
    #[test]
    fn live_request_restores_in_five_percent_steps_after_handoff() {
        let mut r = Resume::new(200, 70, 1_000_000).unwrap();
        assert_eq!(r.due(2_999_999), None);
        for (now, want) in [(3_000_000, 120), (5_000_000, 170), (7_000_000, 200)] {
            assert_eq!(r.due(now), Some(want));
            assert!(r.applied(want, now));
        }
        assert_eq!((r.target(), r.current(), r.steps()), (200, 200, 3));
        assert_eq!(r.due(9_000_000), None);
    }
    #[test]
    fn lower_request_is_restored_in_one_safe_reduction() {
        let start = u32::MAX - 600_000;
        let mut r = Resume::new(40, 70, start).unwrap();
        let due = start.wrapping_add(RESUME_PERIOD_US);
        assert_eq!(r.due(due), Some(40));
        assert!(r.applied(40, due));
        assert_eq!((r.current(), r.steps()), (40, 1));
        assert!(Resume::new(39, 70, 0).is_none());
        assert!(Resume::new(70, crate::duty_envelope::MAX + 1, 0).is_none());
    }
    #[test]
    fn live_upward_request_during_restart_cannot_jump_the_applied_duty() {
        assert!(defer_upward(true, 100, 450));
        assert!(!defer_upward(true, 100, 100));
        assert!(!defer_upward(true, 100, 70));
        assert!(!defer_upward(false, 100, 450));

        let mut r = Resume::new(200, 100, 1_000_000).unwrap();
        assert!(r.retarget(250, 100));
        assert_eq!((r.target(), r.current(), r.steps()), (250, 100, 0));
        assert_eq!(r.due(2_999_999), None);
        assert_eq!(r.due(3_000_000), Some(150));
        assert!(r.applied(150, 3_000_000));
        assert!(r.retarget(300, 150));
        assert_eq!(r.due(4_999_999), None);
        assert_eq!(r.due(5_000_000), Some(200));
        assert_eq!((r.target(), r.current(), r.steps()), (300, 150, 1));
        assert!(!r.retarget(250, 100));
        assert!(!r.retarget(crate::duty_envelope::MAX + 1, 150));
    }
}
