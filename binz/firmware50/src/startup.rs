//! The scripted open-loop run: align, catch, ramp, hold.
//!
//! A 1 kHz foreground tick walks this script. It produces a frequency and a
//! duty for every tick; the target binary turns those into a rotating sine
//! vector. Nothing here touches hardware, so the whole schedule is host-testable.
//!
//! The stages, and why each exists:
//!
//! * **ALIGN** — a *static* vector (zero frequency) at a very low duty, just
//!   long enough to pull the rotor to a known electrical position. Without it
//!   the catch stage starts from an unknown angle and may drag the rotor
//!   backwards.
//! * **CATCH** — a fixed, slow rotation at a higher duty. The rotor has to
//!   lock to this before anything else can work.
//! * **RAMP** — a linear walk of both frequency and duty from the catch values
//!   to the targets. Linear in *both* is what keeps volts-per-hertz roughly
//!   constant; ramping frequency alone over-fluxes at the bottom and pulls
//!   several amps.
//! * **HOLD** — the targets, held. The closed-loop handoff happens inside this
//!   stage, leaving a tail of scripted drive after it.
//!
//! Division-free: the ramp's `/RAMP_TICKS` is a reciprocal multiply.

use crate::fixed::div_2000;

/// Foreground tick rate.
pub const CONTROL_HZ: u32 = 1_000;

pub const ALIGN_TICKS: u32 = 20;
pub const CATCH_TICKS: u32 = 980;
pub const RAMP_TICKS: u32 = 2_000;
pub const HOLD_TICKS: u32 = 2_000;

/// Total scripted run length in *control* ticks: 5000 = 5.0 s at 1 kHz.
///
/// Named `SCRIPT_TICKS` to keep it distinct from `duty::RUN_PERIOD_TICKS`,
/// which is a TIM1 period in timer ticks.
pub const SCRIPT_TICKS: u32 = ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS + HOLD_TICKS;

/// Duty during align, in tenths of a percent (1.0%).
pub const ALIGN_DUTY_TENTHS: u16 = 10;
/// Duty during catch, in tenths of a percent (7.0%).
pub const CATCH_DUTY_TENTHS: u16 = 70;
/// Catch frequency in whole eHz.
pub const CATCH_EHZ: u32 = 100;

/// Highest electrical frequency the script will accept as a target.
pub const MAX_EHZ: u32 = 250;

/// Closed-loop handoff instant, in microseconds from the start of the run.
///
/// `(SCRIPT_TICKS - 300) * 1000` leaves a 300 ms scripted tail after the handoff.
pub const HANDOFF_US: u32 = (SCRIPT_TICKS - 300) * 1_000;

/// Which stage the script is in. Values match the reference's telemetry codes.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Stage {
    /// Not running the script (fixed-frequency diagnostic drive).
    Fixed = 0,
    Align = 1,
    Catch = 2,
    Ramp = 3,
    Hold = 4,
}

impl Stage {
    #[inline]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// What the script wants at one tick.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Setpoint {
    /// Electrical frequency in centihertz (hundredths of a hertz).
    pub freq_chz: u32,
    /// Duty in tenths of a percent.
    pub duty_tenths: u16,
    pub stage: Stage,
}

/// Linear interpolation from `start` to `end` over `RAMP_TICKS`.
///
/// Signed in the span so a downward ramp works, and `t` is clamped to the
/// duration so the endpoint is exact rather than overshooting.
#[inline]
pub const fn ramp(start: u32, end: u32, t: u32) -> u32 {
    let t = if t > RAMP_TICKS { RAMP_TICKS } else { t };
    if end >= start {
        let span = end - start;
        start + div_2000(span.saturating_mul(t))
    } else {
        let span = start - end;
        start - div_2000(span.saturating_mul(t))
    }
}

/// The scripted run, parameterized by its targets.
///
/// The targets are fields rather than const generics because they are what the
/// operator sets per run; the *schedule* (the tick counts and the stage
/// structure) is what is fixed, and that is compiled in.
#[derive(Copy, Clone, Debug)]
pub struct Script {
    target_ehz: u32,
    target_duty_tenths: u16,
    catch_duty_tenths: u16,
    align_duty_tenths: u16,
}

impl Script {
    /// Build a script, clamping the targets into the admissible ranges.
    ///
    /// Clamping rather than rejecting is safe here because both ranges are
    /// bounded below by something that cannot damage anything, and the duty
    /// envelope has already refused anything above its ceiling.
    #[inline]
    pub const fn new(target_ehz: u32, target_duty_tenths: u16) -> Self {
        let target_ehz = if target_ehz == 0 {
            1
        } else if target_ehz > MAX_EHZ {
            MAX_EHZ
        } else {
            target_ehz
        };
        Self {
            target_ehz,
            target_duty_tenths,
            catch_duty_tenths: CATCH_DUTY_TENTHS,
            align_duty_tenths: ALIGN_DUTY_TENTHS,
        }
    }

    #[inline]
    pub const fn target_ehz(&self) -> u32 {
        self.target_ehz
    }
    #[inline]
    pub const fn target_duty_tenths(&self) -> u16 {
        self.target_duty_tenths
    }

    /// Override the catch duty (the reference exposes this as `catchdu`).
    #[inline]
    pub fn set_catch_duty(&mut self, tenths: u16) {
        self.catch_duty_tenths = tenths;
    }

    /// The setpoint at `tick`, or `None` once the scripted run is over.
    pub fn at(&self, tick: u32) -> Option<Setpoint> {
        if tick >= SCRIPT_TICKS {
            return None;
        }
        let target_chz = self.target_ehz * 100;
        let catch_chz = CATCH_EHZ * 100;

        if tick < ALIGN_TICKS {
            // A static vector: zero frequency, and never more duty than the
            // run is going to use anyway.
            let duty = if self.align_duty_tenths > self.target_duty_tenths {
                self.target_duty_tenths
            } else {
                self.align_duty_tenths
            };
            return Some(Setpoint {
                freq_chz: 0,
                duty_tenths: duty,
                stage: Stage::Align,
            });
        }
        if tick < ALIGN_TICKS + CATCH_TICKS {
            return Some(Setpoint {
                freq_chz: catch_chz,
                duty_tenths: self.catch_duty_tenths,
                stage: Stage::Catch,
            });
        }
        if tick < ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS {
            let elapsed = tick - ALIGN_TICKS - CATCH_TICKS;
            return Some(Setpoint {
                freq_chz: ramp(catch_chz, target_chz, elapsed),
                duty_tenths: ramp(self.catch_duty_tenths as u32, self.target_duty_tenths as u32, elapsed) as u16,
                stage: Stage::Ramp,
            });
        }
        Some(Setpoint {
            freq_chz: target_chz,
            duty_tenths: self.target_duty_tenths,
            stage: Stage::Hold,
        })
    }

    /// Whether the closed-loop handoff is due at this elapsed time.
    #[inline]
    pub const fn handoff_due(elapsed_us: u32) -> bool {
        elapsed_us >= HANDOFF_US
    }
}

// ---------------------------------------------------------------------------
// The qualified image's startup profile: 50 eHz catch, then a staircase.
// ---------------------------------------------------------------------------

/// Catch frequency of the reference's staircase profile, whole eHz
/// (`catch=50Hz` in its `RUN:` banner).
pub const STAIR_CATCH_EHZ: u32 = 50;
/// Tick at which the staircase leaves the catch frequency.
pub const STAIR_START_TICK: u32 = 3_200;
/// Frequency after the last step, whole eHz (`target=200Hz`).
pub const STAIR_TARGET_EHZ: u32 = 200;

/// The reference's startup frequency at control tick `tick`, whole eHz.
///
/// Transcribed from `binz/examples/support/campaign.rs::staircase_target`
/// (`bench-startup-staircase`, in the oracle's feature closure): 50 eHz until
/// tick 3200, then fifteen 10 eHz steps whose deadlines are
/// `3200 + floor(i * 1200 / 14)`, reaching 200 eHz at tick 4400. "Replay the
/// successful host trajectory in the 1kHz foreground envelope." No runtime
/// division: the deadlines are the reference's own literal table.
///
/// E065 is why this exists here: firmware50's `Script` catches at 100 eHz and
/// ramps linearly, and a coast taken at its handover instant showed a rotor
/// that was not turning at all.
#[must_use]
pub const fn staircase_ehz(tick: u32) -> u32 {
    let mut index = 0u32;
    while index < 15 {
        let deadline = STAIR_START_TICK
            + match index {
                0 => 0,
                1 => 85,
                2 => 171,
                3 => 257,
                4 => 342,
                5 => 428,
                6 => 514,
                7 => 600,
                8 => 685,
                9 => 771,
                10 => 857,
                11 => 942,
                12 => 1028,
                13 => 1114,
                _ => 1200,
            };
        if tick < deadline {
            return STAIR_CATCH_EHZ + 10 * index;
        }
        index += 1;
    }
    STAIR_TARGET_EHZ
}

// The reference's own compile-time check on its table, kept verbatim in
// meaning: each step lands exactly on its deadline.
const _: () = {
    let mut index = 0;
    while index < 15 {
        let tick = STAIR_START_TICK + index * 1200 / 14;
        assert!(staircase_ehz(tick) == 60 + 10 * index);
        assert!(staircase_ehz(tick - 1) == 50 + 10 * index);
        index += 1;
    }
};

/// The reference's scripted startup: align, 50 eHz catch, staircase, hold.
///
/// Same stage lengths and handover instant as [`Script`] (the reference's
/// `RUN: align=20ms@1.0% catch=50Hz/980ms@6.2% ramp=2000ms ... hold=2000ms`,
/// handover at 4.7 s under `bench-handoff-early`); only the frequency profile
/// differs. Duty is the catch duty throughout the catch and ramp stages, and
/// the target duty in hold -- the reference's `du62`/`catchdu62` make those
/// the same 6.2%.
#[derive(Copy, Clone, Debug)]
pub struct StaircaseScript {
    catch_duty_tenths: u16,
    target_duty_tenths: u16,
}

impl StaircaseScript {
    #[must_use]
    pub const fn new(catch_duty_tenths: u16, target_duty_tenths: u16) -> Self {
        Self {
            catch_duty_tenths,
            target_duty_tenths,
        }
    }

    /// The setpoint at `tick`, or `None` once the script is over.
    #[must_use]
    pub fn at(&self, tick: u32) -> Option<Setpoint> {
        if tick >= SCRIPT_TICKS {
            return None;
        }
        if tick < ALIGN_TICKS {
            let duty = if ALIGN_DUTY_TENTHS > self.target_duty_tenths {
                self.target_duty_tenths
            } else {
                ALIGN_DUTY_TENTHS
            };
            return Some(Setpoint {
                freq_chz: 0,
                duty_tenths: duty,
                stage: Stage::Align,
            });
        }
        let stage = if tick < ALIGN_TICKS + CATCH_TICKS {
            Stage::Catch
        } else if tick < ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS {
            Stage::Ramp
        } else {
            Stage::Hold
        };
        let duty = if stage == Stage::Hold {
            self.target_duty_tenths
        } else {
            self.catch_duty_tenths
        };
        Some(Setpoint {
            freq_chz: staircase_ehz(tick) * 100,
            duty_tenths: duty,
            stage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The run this crate exists to perform: 10.0% at a modest speed.
    fn ten_percent() -> Script {
        Script::new(150, 100)
    }

    // --- the reference's staircase profile (E066) --------------------------

    #[test]
    fn staircase_holds_50_until_tick_3200_then_steps_to_200_by_4400() {
        assert_eq!(staircase_ehz(20), 50);
        assert_eq!(staircase_ehz(3_199), 50);
        assert_eq!(staircase_ehz(3_200), 60);
        assert_eq!(staircase_ehz(4_399), 190);
        assert_eq!(staircase_ehz(4_400), 200);
        assert_eq!(staircase_ehz(u32::MAX), 200);
    }

    #[test]
    fn staircase_is_monotone_in_ten_ehz_steps() {
        let mut prev = staircase_ehz(0);
        let mut steps = 0;
        for t in 1..SCRIPT_TICKS {
            let f = staircase_ehz(t);
            assert!(f == prev || f == prev + 10, "tick {t}: {prev} -> {f}");
            if f != prev {
                steps += 1;
            }
            prev = f;
        }
        assert_eq!(steps, 15);
    }

    #[test]
    fn staircase_script_stages_and_handover_frequency() {
        let s = StaircaseScript::new(62, 62);
        let a = s.at(0).unwrap();
        assert_eq!((a.stage, a.freq_chz, a.duty_tenths), (Stage::Align, 0, 10));
        let c = s.at(500).unwrap();
        assert_eq!((c.stage, c.freq_chz, c.duty_tenths), (Stage::Catch, 5_000, 62));
        let r = s.at(2_000).unwrap();
        assert_eq!((r.stage, r.freq_chz), (Stage::Ramp, 5_000));
        // At the 4.7 s handover the vector is at the reference's 200 eHz.
        let h = s.at(HANDOFF_US / 1_000).unwrap();
        assert_eq!((h.stage, h.freq_chz, h.duty_tenths), (Stage::Hold, 20_000, 62));
        assert!(s.at(SCRIPT_TICKS).is_none());
    }

    // --- structure ---------------------------------------------------------

    #[test]
    fn the_schedule_matches_the_reference_tick_counts() {
        assert_eq!(ALIGN_TICKS, 20);
        assert_eq!(CATCH_TICKS, 980);
        assert_eq!(RAMP_TICKS, 2_000);
        assert_eq!(HOLD_TICKS, 2_000);
        assert_eq!(SCRIPT_TICKS, 5_000, "5.0 s at 1 kHz");
        assert_eq!(HANDOFF_US, 4_700_000);
    }

    #[test]
    fn the_stages_appear_in_order_and_cover_every_tick() {
        let s = ten_percent();
        let mut seen = [0u32; 5];
        for t in 0..SCRIPT_TICKS {
            let sp = s.at(t).expect("every tick inside the run has a setpoint");
            seen[sp.stage.code() as usize] += 1;
        }
        assert_eq!(seen[Stage::Align.code() as usize], ALIGN_TICKS);
        assert_eq!(seen[Stage::Catch.code() as usize], CATCH_TICKS);
        assert_eq!(seen[Stage::Ramp.code() as usize], RAMP_TICKS);
        assert_eq!(seen[Stage::Hold.code() as usize], HOLD_TICKS);
        assert_eq!(seen[Stage::Fixed.code() as usize], 0);
    }

    #[test]
    fn the_run_ends_and_stays_ended() {
        let s = ten_percent();
        assert!(s.at(SCRIPT_TICKS - 1).is_some());
        assert!(s.at(SCRIPT_TICKS).is_none());
        assert!(s.at(SCRIPT_TICKS + 1).is_none());
        assert!(s.at(u32::MAX).is_none());
    }

    // --- align -------------------------------------------------------------

    #[test]
    fn align_holds_a_static_vector_at_low_duty() {
        let s = ten_percent();
        for t in 0..ALIGN_TICKS {
            let sp = s.at(t).unwrap();
            assert_eq!(sp.stage, Stage::Align);
            assert_eq!(sp.freq_chz, 0, "align must not rotate");
            assert_eq!(sp.duty_tenths, ALIGN_DUTY_TENTHS);
        }
    }

    #[test]
    fn align_never_exceeds_the_runs_own_duty() {
        // A run commanded below the align duty must not see a *higher* duty
        // during align than it asked for.
        let s = Script::new(150, 5);
        assert_eq!(s.at(0).unwrap().duty_tenths, 5);
    }

    // --- catch -------------------------------------------------------------

    #[test]
    fn catch_is_a_fixed_slow_rotation() {
        let s = ten_percent();
        for t in ALIGN_TICKS..(ALIGN_TICKS + CATCH_TICKS) {
            let sp = s.at(t).unwrap();
            assert_eq!(sp.stage, Stage::Catch);
            assert_eq!(sp.freq_chz, CATCH_EHZ * 100, "100.0 eHz");
            assert_eq!(sp.duty_tenths, CATCH_DUTY_TENTHS);
        }
    }

    #[test]
    fn the_catch_duty_is_overridable() {
        let mut s = ten_percent();
        s.set_catch_duty(90);
        assert_eq!(s.at(ALIGN_TICKS).unwrap().duty_tenths, 90);
    }

    // --- ramp --------------------------------------------------------------

    #[test]
    fn the_ramp_starts_at_catch_and_ends_at_the_target() {
        let s = ten_percent();
        let first = s.at(ALIGN_TICKS + CATCH_TICKS).unwrap();
        assert_eq!(first.stage, Stage::Ramp);
        assert_eq!(first.freq_chz, CATCH_EHZ * 100);
        assert_eq!(first.duty_tenths, CATCH_DUTY_TENTHS);

        let last = s.at(ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS - 1).unwrap();
        assert_eq!(last.stage, Stage::Ramp);
        // one tick short of the end, so within one step of the target
        assert!(last.freq_chz <= s.target_ehz() * 100);
        assert!(last.duty_tenths <= 100);
    }

    #[test]
    fn the_ramp_is_monotone_in_both_frequency_and_duty() {
        let s = ten_percent();
        let start = ALIGN_TICKS + CATCH_TICKS;
        let mut pf = 0;
        let mut pd = 0;
        for t in start..(start + RAMP_TICKS) {
            let sp = s.at(t).unwrap();
            assert!(sp.freq_chz >= pf, "frequency went backwards at {t}");
            assert!(sp.duty_tenths >= pd, "duty went backwards at {t}");
            pf = sp.freq_chz;
            pd = sp.duty_tenths;
        }
    }

    #[test]
    fn the_ramp_walks_volts_per_hertz_together_not_frequency_alone() {
        // The scar this guards: ramping frequency while holding duty
        // over-fluxes at the bottom and pulls several amps. At the ramp
        // midpoint both quantities must be about halfway.
        let s = ten_percent();
        let mid = ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS / 2;
        let sp = s.at(mid).unwrap();
        let f_frac_pct = (sp.freq_chz - CATCH_EHZ * 100) * 100 / (s.target_ehz() * 100 - CATCH_EHZ * 100);
        let d_frac_pct =
            (sp.duty_tenths - CATCH_DUTY_TENTHS) as u32 * 100 / (s.target_duty_tenths() - CATCH_DUTY_TENTHS) as u32;
        assert!((45..=55).contains(&f_frac_pct), "frequency {f_frac_pct}% through");
        assert!((45..=55).contains(&d_frac_pct), "duty {d_frac_pct}% through");
    }

    #[test]
    fn a_downward_ramp_works_too() {
        // Target slower than the catch frequency, and below the catch duty.
        let s = Script::new(50, 50);
        let start = ALIGN_TICKS + CATCH_TICKS;
        let first = s.at(start).unwrap();
        assert_eq!(first.freq_chz, CATCH_EHZ * 100);
        assert_eq!(first.duty_tenths, CATCH_DUTY_TENTHS);
        let mut prev_f = first.freq_chz;
        let mut prev_d = first.duty_tenths;
        for t in start..(start + RAMP_TICKS) {
            let sp = s.at(t).unwrap();
            assert!(sp.freq_chz <= prev_f, "frequency rose on a down-ramp at {t}");
            assert!(sp.duty_tenths <= prev_d, "duty rose on a down-ramp at {t}");
            prev_f = sp.freq_chz;
            prev_d = sp.duty_tenths;
        }
        let hold = s.at(ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS).unwrap();
        assert_eq!(hold.freq_chz, 5_000);
        assert_eq!(hold.duty_tenths, 50);
    }

    #[test]
    fn ramp_endpoints_are_exact_and_clamped() {
        assert_eq!(ramp(1000, 2000, 0), 1000);
        assert_eq!(ramp(1000, 2000, RAMP_TICKS), 2000);
        assert_eq!(ramp(1000, 2000, RAMP_TICKS * 10), 2000, "t is clamped");
        assert_eq!(ramp(1000, 2000, RAMP_TICKS / 2), 1500);
        // downward
        assert_eq!(ramp(2000, 1000, 0), 2000);
        assert_eq!(ramp(2000, 1000, RAMP_TICKS), 1000);
        assert_eq!(ramp(2000, 1000, RAMP_TICKS / 2), 1500);
        // degenerate
        assert_eq!(ramp(500, 500, 123), 500);
    }

    #[test]
    fn ramp_matches_exact_integer_interpolation() {
        for (start, end) in [(10_000u32, 25_000u32), (7u32, 100u32), (25_000, 10_000)] {
            for t in 0..=RAMP_TICKS {
                let want = if end >= start {
                    start + (end - start) * t / RAMP_TICKS
                } else {
                    start - (start - end) * t / RAMP_TICKS
                };
                assert_eq!(ramp(start, end, t), want, "{start}->{end} at {t}");
            }
        }
    }

    #[test]
    fn ramp_cannot_overflow_at_the_widest_span() {
        // Widest frequency span is catch to MAX_EHZ.
        let _ = ramp(0, MAX_EHZ * 100, RAMP_TICKS);
        let _ = ramp(0, u32::MAX, RAMP_TICKS);
        let _ = ramp(u32::MAX, 0, RAMP_TICKS);
    }

    // --- hold and handoff --------------------------------------------------

    #[test]
    fn hold_sits_exactly_on_the_targets() {
        let s = ten_percent();
        let start = ALIGN_TICKS + CATCH_TICKS + RAMP_TICKS;
        for t in start..SCRIPT_TICKS {
            let sp = s.at(t).unwrap();
            assert_eq!(sp.stage, Stage::Hold);
            assert_eq!(sp.freq_chz, s.target_ehz() * 100);
            assert_eq!(sp.duty_tenths, s.target_duty_tenths());
        }
    }

    #[test]
    fn the_handoff_falls_inside_the_hold_stage_with_a_tail() {
        // Handing off during the ramp would mean the closed loop inherits a
        // moving setpoint; handing off at the very end would leave no
        // scripted drive to fall back on.
        let s = ten_percent();
        let handoff_tick = HANDOFF_US / 1_000;
        let sp = s.at(handoff_tick).unwrap();
        assert_eq!(sp.stage, Stage::Hold, "handoff must happen in hold");
        assert_eq!(SCRIPT_TICKS - handoff_tick, 300, "300 ms of tail remains");
    }

    #[test]
    fn the_handoff_predicate_fires_once_the_instant_is_reached() {
        assert!(!Script::handoff_due(0));
        assert!(!Script::handoff_due(HANDOFF_US - 1));
        assert!(Script::handoff_due(HANDOFF_US));
        assert!(Script::handoff_due(HANDOFF_US + 1));
    }

    // --- target clamping ---------------------------------------------------

    #[test]
    fn targets_are_clamped_into_the_admissible_range() {
        assert_eq!(Script::new(0, 100).target_ehz(), 1, "zero would never rotate");
        assert_eq!(Script::new(MAX_EHZ + 1, 100).target_ehz(), MAX_EHZ);
        assert_eq!(Script::new(u32::MAX, 100).target_ehz(), MAX_EHZ);
        assert_eq!(Script::new(150, 100).target_ehz(), 150);
    }

    #[test]
    fn no_tick_of_any_admissible_script_produces_an_out_of_range_duty() {
        // The script must never hand the bridge more duty than the run asked
        // for, at any tick, for any target.
        for ehz in [1u32, 50, 100, 150, MAX_EHZ] {
            for duty in [10u16, 40, 70, 100, 300] {
                let s = Script::new(ehz, duty);
                let ceiling = duty.max(CATCH_DUTY_TENTHS);
                for t in 0..SCRIPT_TICKS {
                    let sp = s.at(t).unwrap();
                    assert!(
                        sp.duty_tenths <= ceiling,
                        "ehz {ehz} duty {duty} tick {t} -> {}",
                        sp.duty_tenths
                    );
                    assert!(sp.freq_chz <= MAX_EHZ * 100);
                }
            }
        }
    }
}
