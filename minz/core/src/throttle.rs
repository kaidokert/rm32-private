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
/// PROBED 2026-07-19: 15 ticks (0.4 %/ms, toward AM32's 2-16 %/ms
/// di/dt clamp) — 1 % rungs stayed trapped (punch too small), 10 %
/// rungs punched inconsistently, and at amp 40+ the fast slew went
/// regen-unstable (bus pumped to 11 V, sag kills). The throttle-
/// dynamics lever needs the full R1 duty_slew architecture, not a
/// cadence constant; reverted to the proven 300.
pub const STEP_TICKS: u32 = 300;

/// Duty-scaled slew cadence (2026-07-19 top-transit fold): a 10 %
/// target step at amp ~85+ (2,000 Hz, prop ω³ load) at the flat
/// 1 %/50 ms slew drags the bus to 6.9 V — deeper than AM32 at 100 %
/// throttle — and sag-kills. Acceleration power scales with speed,
/// so the slew slows where the transit power lives: 1 %/50 ms below
/// 70 % duty (the proven cadence), 1 %/100 ms to 85 %, 1 %/150 ms
/// above. A full-range 20→100 % sweep only gains ~3 s.
pub fn step_ticks_for(applied: u8) -> u32 {
    if applied >= 85 {
        STEP_TICKS * 3
    } else if applied >= 70 {
        STEP_TICKS * 2
    } else {
        STEP_TICKS
    }
}

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

/// SAG-AWARE slew hold (2026-07-19 transit-surge defect): a sustained
/// upward slew at speed (the 10 %-step ladder = 500 ms of continuous
/// +1 %/50 ms) drags a lagging-BEMF surge current the whole transit
/// and collapsed the bench PSU to 6.2 V from a HEALTHY 1,200 Hz lock
/// — four out of four runs, all with clean accepts right up to the
/// sag kill (the supply is never the wall; the loop-created surge
/// is). AM32 rides the same PSU to 4.15 A because its throttle
/// respects the electrical state. Rule: while the bus reads below the
/// soft floor, upward slewing PAUSES (holds, never cuts — downward
/// slew always proceeds). The hard sag kill stays at −15 %/6.9 V; the
/// soft floor sits above it so the hold engages before the kill can.
pub const SAG_HOLD_FLOOR_RAW: u16 = 973; // ≈7.30 V on the 7507 µV/count scale
pub fn step_sag_aware(applied: u8, target: u8, vbat_raw: u16) -> u8 {
    if applied < target && vbat_raw < SAG_HOLD_FLOOR_RAW {
        applied // hold: don't deepen the surge
    } else {
        step(applied, target)
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
    fn duty_scaled_cadence_slows_at_the_top() {
        assert_eq!(step_ticks_for(15), STEP_TICKS);
        assert_eq!(step_ticks_for(69), STEP_TICKS);
        assert_eq!(step_ticks_for(70), STEP_TICKS * 2);
        assert_eq!(step_ticks_for(84), STEP_TICKS * 2);
        assert_eq!(step_ticks_for(85), STEP_TICKS * 3);
        assert_eq!(step_ticks_for(96), STEP_TICKS * 3);
    }

    #[test]
    fn sag_hold_pauses_up_allows_down() {
        // Below the soft floor: upward slew holds...
        assert_eq!(step_sag_aware(50, 60, SAG_HOLD_FLOOR_RAW - 1), 50);
        // ...downward slew still proceeds (never trap duty high)...
        assert_eq!(step_sag_aware(50, 40, SAG_HOLD_FLOOR_RAW - 1), 49);
        // ...and a healthy bus slews up normally.
        assert_eq!(step_sag_aware(50, 60, SAG_HOLD_FLOOR_RAW + 10), 51);
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
