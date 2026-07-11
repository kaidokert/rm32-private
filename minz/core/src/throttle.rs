//! Throttle slew limiter (roadmap item 5): keys set a TARGET; the
//! applied duty walks toward it at 1 % per `STEP_TICKS` drive ticks,
//! so the loop never sees a throttle step (step transients at
//! 1,300 Hz pulled 85 ms current surges past even 3.8× the healthy
//! envelope).
//!
//! Critical companion rule, learned at the cost of 4/4 failed
//! engages: **arming snaps applied = target**. The slew is for
//! changes while running — an abort at amp 50 left the applied duty
//! slewing down for two seconds while the next engage's open-loop
//! spin-up ran at ~45 % duty, seeding the estimator with chaos.

/// Drive ticks (6 kHz) per 1 % step → 50 ms per percent.
pub const STEP_TICKS: u32 = 300;

/// One slew step: move `applied` one percent toward `target`.
/// (Pure form for firmware call sites that keep their state in
/// atomics; [`Slew`] wraps the same logic for host tests.)
pub fn step(applied: u8, target: u8) -> u8 {
    if applied < target {
        applied + 1
    } else if applied > target {
        applied - 1
    } else {
        applied
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slew {
    pub applied: u8,
    pub target: u8,
}

impl Slew {
    pub const fn new(pct: u8) -> Self {
        Self {
            applied: pct,
            target: pct,
        }
    }

    /// Key handler: set where the throttle should end up.
    pub fn set_target(&mut self, pct: u8) {
        self.target = pct;
    }

    /// Arm/re-arm: snap — no slewing across a disarm.
    pub fn snap(&mut self, pct: u8) {
        self.applied = pct;
        self.target = pct;
    }

    /// Drive-tick stepper: call every tick with the tick counter;
    /// moves `applied` one percent toward `target` every
    /// `STEP_TICKS`.
    pub fn tick(&mut self, tick: u32) {
        if !tick.is_multiple_of(STEP_TICKS) {
            return;
        }
        self.applied = step(self.applied, self.target);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_ticks(s: &mut Slew, from: u32, n: u32) -> u32 {
        for t in from..from + n {
            s.tick(t);
        }
        from + n
    }

    #[test]
    fn steps_one_percent_per_step_ticks() {
        let mut s = Slew::new(15);
        s.set_target(18);
        run_ticks(&mut s, 1, STEP_TICKS * 3 + 10);
        assert_eq!(s.applied, 18);
    }

    #[test]
    fn slews_down_too() {
        let mut s = Slew::new(50);
        s.set_target(47);
        run_ticks(&mut s, 1, STEP_TICKS * 5);
        assert_eq!(s.applied, 47);
    }

    #[test]
    fn no_overshoot_or_oscillation() {
        let mut s = Slew::new(20);
        s.set_target(22);
        run_ticks(&mut s, 0, STEP_TICKS * 50);
        assert_eq!(s.applied, 22);
    }

    #[test]
    fn regression_snap_on_arm_2026_07_11() {
        // The 4/4 failed-engage bug: abort at amp 50, then re-arm at
        // 15. Without snap, `applied` is still ~50 during the
        // open-loop spin-up. With snap, spin-up sees 15 immediately.
        let mut s = Slew::new(15);
        s.set_target(50);
        run_ticks(&mut s, 0, STEP_TICKS * 40); // reach 50
        assert_eq!(s.applied, 50);
        // Kill + re-arm:
        s.snap(15);
        assert_eq!(s.applied, 15);
        assert_eq!(s.target, 15);
    }

    #[test]
    fn mid_slew_retarget_converges() {
        let mut s = Slew::new(15);
        s.set_target(30);
        run_ticks(&mut s, 0, STEP_TICKS * 5); // partway up
        assert!(s.applied > 15 && s.applied < 30);
        s.set_target(10); // reverse mid-flight
        run_ticks(&mut s, STEP_TICKS * 5, STEP_TICKS * 30);
        assert_eq!(s.applied, 10);
    }
}
