//! The duty envelope: what throttle the firmware will actually apply.
//!
//! A commanded duty passes through three independent limits before it reaches a
//! compare register, and all three must be satisfied:
//!
//! 1. the **envelope** — a hard floor, ceiling and step quantization that
//!    bounds what a command is even allowed to ask for;
//! 2. the **foldback governor** — a ratcheting ceiling owned by the current
//!    protection, which only ever moves down;
//! 3. the **compare conversion** — `ticks * duty / 1000`, division-free.
//!
//! The envelope is deliberately the outermost gate: a malformed or hostile host
//! command cannot reach the bridge with a duty the envelope forbids, and
//! anything above the ceiling is a **stop**, not a clamp, so a runaway command
//! fails closed rather than silently running at maximum.

use crate::fixed::duty_to_ccr;
use crate::protection::{FoldbackGovernor, Reason};

/// Lowest duty that will actually turn the motor, in tenths of a percent.
pub const ENVELOPE_MIN: u16 = 40;
/// Command quantization, in tenths of a percent.
pub const ENVELOPE_STEP: u16 = 50;
/// Highest duty the envelope permits. A command above this stops the run.
///
/// This is the reference's **baseline** ceiling (30.0%). The frozen 50%-run
/// image raised it to [`ENVELOPE_MAX_DUTY50`] behind a build feature; this
/// crate targets a 10% hold, so it composes the lower ceiling — a duty the run
/// never needs is a duty the envelope should refuse.
pub const ENVELOPE_MAX: u16 = 300;

/// The raised ceiling the 50% campaign image was built with. Kept as a named
/// constant so the difference is explicit rather than a silently edited number,
/// and so a future 50% personality is a type parameter, not a patch.
pub const ENVELOPE_MAX_DUTY50: u16 = 500;

/// TIM1 period in timer ticks at the running 48 kHz carrier.
///
/// Named `RUN_PERIOD_TICKS`, not `RUN_PERIOD_TICKS`: `startup::RUN_PERIOD_TICKS` is 5000
/// *control* ticks, and the binary imports both. Two public constants a
/// thousand apart, both called `RUN_PERIOD_TICKS`, differing only in unit, is a
/// collision waiting to be miscompiled into a correct-looking expression.
#[cfg(not(feature = "carrier-24k"))]
pub const RUN_PERIOD_TICKS: u32 = 1333;
/// ENV-29 A/B: AM32's 24 kHz carrier (`NOMINAL_PWM 24000`), half as many switching
/// transients inside each crossing window (ENV-28). 64 MHz / 2666 = 24.0 kHz.
#[cfg(feature = "carrier-24k")]
pub const RUN_PERIOD_TICKS: u32 = 2666;
/// TIM1 period in ticks at the 10 kHz startup carrier.
pub const STARTUP_TICKS: u32 = 6400;

/// What the envelope decided about a commanded duty.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Admit {
    /// Accepted at this duty, in tenths of a percent.
    At(u16),
    /// The command is out of bounds; stop the run with this reason.
    Stop(Reason),
}

/// The duty envelope as a policy type.
///
/// The bounds are const generics rather than fields because they are exactly
/// the kind of small bounded numeric parameter the typed-composition rule wants
/// there: a differently-bounded personality is a different type, checked at
/// compile time, not a runtime field someone can write to.
pub struct Envelope<const MIN: u16, const MAX: u16, const STEP: u16>;

/// The envelope this crate composes: the reference baseline, ceiling 30.0%.
pub type ProductionEnvelope = Envelope<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>;

/// The raised-ceiling personality, for reference and for host tests that need
/// to exercise duties the production envelope refuses. Composing this is a
/// deliberate type-level choice.
pub type Duty50Envelope = Envelope<ENVELOPE_MIN, ENVELOPE_MAX_DUTY50, ENVELOPE_STEP>;

impl<const MIN: u16, const MAX: u16, const STEP: u16> Envelope<MIN, MAX, STEP> {
    /// Judge a commanded duty in tenths of a percent.
    ///
    /// * zero is a legitimate "stop driving" command, admitted as-is;
    /// * anything above `MAX` is a **stop**, never a clamp;
    /// * anything below `MIN` (but non-zero) is raised to `MIN`, because a duty
    ///   under the floor cannot turn the rotor and would look like a stall.
    #[inline]
    pub const fn admit(commanded_tenths: u16) -> Admit {
        if commanded_tenths == 0 {
            return Admit::At(0);
        }
        if commanded_tenths > MAX {
            return Admit::Stop(Reason::HostAbort);
        }
        if commanded_tenths < MIN {
            return Admit::At(MIN);
        }
        Admit::At(commanded_tenths)
    }

    /// The ladder of duties a sweep walks: `MIN`, then `MIN + STEP`, ... up to
    /// and including `MAX`. `index` past the end saturates at `MAX`.
    #[inline]
    pub const fn rung(index: u16) -> u16 {
        // STEP == 0 would be an infinite ladder; treat it as a single rung.
        if STEP == 0 {
            return MIN;
        }
        let offset = (index as u32) * (STEP as u32);
        let v = (MIN as u32) + offset;
        if v > MAX as u32 {
            MAX
        } else {
            v as u16
        }
    }

    /// How many rungs the ladder has, including the final `MAX` rung.
    #[inline]
    pub const fn rungs() -> u16 {
        if STEP == 0 || MAX <= MIN {
            return 1;
        }
        // ceil((MAX - MIN) / STEP) + 1, computed without division by walking:
        // the span is at most 1000 tenths and STEP at least 1, so this is
        // bounded and cheap, and it runs once at startup, never in an ISR.
        let mut n = 1u16;
        let mut v = MIN;
        while v < MAX {
            v = match v.checked_add(STEP) {
                Some(next) => next,
                None => break,
            };
            n += 1;
        }
        n
    }
}

/// Envelope admission plus the foldback governor plus the compare conversion,
/// composed once.
///
/// **Scope of the guarantee, stated precisely.** Holding the governor here
/// means nothing that goes *through this type* can reach a compare register
/// without passing the foldback ceiling. It does not mean this type is the only
/// route to a compare register in the firmware: the open-loop bring-up binary
/// drives the bridge from `sine::compares`, clamping through its own
/// `FoldbackGovernor` on the way. An earlier version of this comment claimed
/// sole ownership of that path and was simply false -- worth more than a
/// wrong-comment nit, because a reader would have trusted an invariant nothing
/// enforced.
///
/// Not `Copy`: it owns the ratcheting governor.
#[derive(Clone, Debug)]
pub struct DutyPath {
    governor: FoldbackGovernor,
    ticks: u32,
    applied_tenths: u16,
}

impl DutyPath {
    #[inline]
    pub const fn new(ticks: u32, ceiling_tenths: u16, floor_tenths: u16) -> Self {
        Self {
            governor: FoldbackGovernor::new(ceiling_tenths, floor_tenths),
            ticks,
            applied_tenths: 0,
        }
    }

    #[inline]
    pub const fn governor(&self) -> &FoldbackGovernor {
        &self.governor
    }
    #[inline]
    pub fn governor_mut(&mut self) -> &mut FoldbackGovernor {
        &mut self.governor
    }
    #[inline]
    pub const fn applied(&self) -> u16 {
        self.applied_tenths
    }

    /// Switch carriers (startup 10 kHz -> running 48 kHz). The applied duty is
    /// a percentage, so it survives the change; the compare value does not.
    #[inline]
    pub fn set_ticks(&mut self, ticks: u32) {
        self.ticks = ticks;
    }

    #[inline]
    pub const fn ticks(&self) -> u32 {
        self.ticks
    }

    /// Run a commanded duty through envelope, governor and conversion.
    ///
    /// Returns the compare value to write, or the stop reason.
    pub fn command<const MIN: u16, const MAX: u16, const STEP: u16>(
        &mut self,
        commanded_tenths: u16,
    ) -> Result<u32, Reason> {
        match Envelope::<MIN, MAX, STEP>::admit(commanded_tenths) {
            Admit::Stop(reason) => Err(reason),
            Admit::At(admitted) => {
                let governed = self.governor.clamp(admitted);
                self.applied_tenths = governed;
                Ok(duty_to_ccr(self.ticks, governed as u32))
            }
        }
    }

    /// The compare value for the currently applied duty, e.g. after a carrier
    /// change or a foldback, without re-issuing a command.
    #[inline]
    pub fn recompute(&mut self) -> u32 {
        let governed = self.governor.clamp(self.applied_tenths);
        self.applied_tenths = governed;
        duty_to_ccr(self.ticks, governed as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protection::Reduction;

    type E = ProductionEnvelope;

    // --- envelope admission ------------------------------------------------

    #[test]
    fn zero_is_a_legitimate_stop_driving_command() {
        assert_eq!(E::admit(0), Admit::At(0));
    }

    #[test]
    fn a_command_above_the_ceiling_stops_rather_than_clamping() {
        // This is the important direction: clamping a runaway command would
        // silently run the motor at maximum.
        assert_eq!(E::admit(ENVELOPE_MAX + 1), Admit::Stop(Reason::HostAbort));
        assert_eq!(E::admit(1000), Admit::Stop(Reason::HostAbort));
        assert_eq!(E::admit(u16::MAX), Admit::Stop(Reason::HostAbort));
        // exactly at the ceiling is admitted
        assert_eq!(E::admit(ENVELOPE_MAX), Admit::At(ENVELOPE_MAX));
    }

    #[test]
    fn a_nonzero_command_below_the_floor_is_raised_to_the_floor() {
        for c in 1..ENVELOPE_MIN {
            assert_eq!(E::admit(c), Admit::At(ENVELOPE_MIN), "commanded {c}");
        }
        assert_eq!(E::admit(ENVELOPE_MIN), Admit::At(ENVELOPE_MIN));
    }

    #[test]
    fn the_ten_percent_command_passes_through_untouched() {
        assert_eq!(E::admit(100), Admit::At(100));
    }

    // --- ladder ------------------------------------------------------------

    #[test]
    fn the_ladder_starts_at_the_floor_and_saturates_at_the_ceiling() {
        assert_eq!(E::rung(0), ENVELOPE_MIN);
        assert_eq!(E::rung(1), ENVELOPE_MIN + ENVELOPE_STEP);
        // far past the end
        assert_eq!(E::rung(1000), ENVELOPE_MAX);
        assert_eq!(E::rung(u16::MAX), ENVELOPE_MAX);
    }

    #[test]
    fn every_ladder_rung_is_admissible() {
        for i in 0..E::rungs() {
            let d = E::rung(i);
            assert!(matches!(E::admit(d), Admit::At(v) if v == d), "rung {i} = {d}");
        }
    }

    #[test]
    fn the_ladder_is_monotone_and_terminates() {
        let n = E::rungs();
        assert!(n > 1 && n < 100, "implausible rung count {n}");
        let mut prev = 0;
        for i in 0..n {
            let d = E::rung(i);
            assert!(d >= prev, "rung {i} went backwards");
            prev = d;
        }
        assert_eq!(E::rung(n - 1), ENVELOPE_MAX, "last rung is the ceiling");
    }

    #[test]
    fn a_degenerate_zero_step_envelope_does_not_hang() {
        type Degenerate = Envelope<40, 500, 0>;
        assert_eq!(Degenerate::rungs(), 1);
        assert_eq!(Degenerate::rung(0), 40);
        assert_eq!(Degenerate::rung(5), 40);
    }

    #[test]
    fn a_degenerate_inverted_envelope_does_not_hang() {
        type Inverted = Envelope<500, 40, 50>;
        assert_eq!(Inverted::rungs(), 1);
    }

    // --- the composed path -------------------------------------------------

    fn path() -> DutyPath {
        DutyPath::new(RUN_PERIOD_TICKS, ENVELOPE_MAX, ENVELOPE_MIN)
    }

    #[test]
    fn the_ten_percent_command_produces_the_reference_compare_value() {
        let mut p = path();
        let ccr = p
            .command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(100)
            .expect("10% must be admissible");
        // 48 kHz: du100 at 1333 ticks -> CCR 133 (pinned literally); the ENV-29
        // carrier-24k A/B arm doubles the period, so its compare doubles too.
        #[cfg(not(feature = "carrier-24k"))]
        assert_eq!(ccr, 133, "du100 at 1333 ticks -> CCR 133");
        #[cfg(feature = "carrier-24k")]
        assert_eq!(ccr, 266, "du100 at 2666 ticks -> CCR 266");
        assert_eq!(p.applied(), 100);
    }

    #[test]
    fn an_out_of_envelope_command_yields_a_stop_and_applies_nothing() {
        let mut p = path();
        p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(100).unwrap();
        let before = p.applied();
        let r = p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(900);
        assert_eq!(r, Err(Reason::HostAbort));
        assert_eq!(p.applied(), before, "a refused command must not change the drive");
    }

    #[test]
    fn the_governor_caps_the_command_and_cannot_be_bypassed() {
        let mut p = path();
        let _ = p.governor_mut().warn(Reduction(50));
        let capped = ENVELOPE_MAX - 50;
        let ccr = p
            .command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(ENVELOPE_MAX)
            .unwrap();
        assert_eq!(p.applied(), capped, "governor must win over the command");
        assert_eq!(ccr, duty_to_ccr(RUN_PERIOD_TICKS, capped as u32));
    }

    #[test]
    fn a_foldback_after_a_command_lowers_the_compare_on_recompute() {
        let mut p = path();
        let full = p
            .command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(ENVELOPE_MAX)
            .unwrap();
        let _ = p.governor_mut().warn(Reduction(50));
        let folded = p.recompute();
        assert!(folded < full, "foldback must reduce the compare: {full} -> {folded}");
        assert_eq!(p.applied(), ENVELOPE_MAX - 50);
    }

    #[test]
    fn foldback_ratchets_and_never_recovers_on_a_later_command() {
        let mut p = path();
        let _ = p.governor_mut().warn(Reduction(50));
        let _ = p.governor_mut().warn(Reduction(50));
        let ratcheted = ENVELOPE_MAX - 100;
        assert_eq!(p.governor().ceiling(), ratcheted);
        // re-commanding maximum must not walk the ceiling back up
        p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(ENVELOPE_MAX)
            .unwrap();
        assert_eq!(p.governor().ceiling(), ratcheted);
        assert_eq!(p.applied(), ratcheted);
    }

    #[test]
    fn the_governor_floor_keeps_a_deep_foldback_above_the_envelope_minimum() {
        let mut p = path();
        for _ in 0..50 {
            let _ = p.governor_mut().warn(Reduction(50));
        }
        assert_eq!(p.governor().ceiling(), ENVELOPE_MIN);
        let ccr = p
            .command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(ENVELOPE_MAX)
            .unwrap();
        assert_eq!(p.applied(), ENVELOPE_MIN);
        assert_eq!(ccr, duty_to_ccr(RUN_PERIOD_TICKS, ENVELOPE_MIN as u32));
    }

    #[test]
    fn the_production_envelope_refuses_what_the_duty50_personality_admits() {
        // The two personalities differ only in the ceiling, and that
        // difference is visible at the type level rather than by rebuilding.
        assert_eq!(E::admit(400), Admit::Stop(Reason::HostAbort));
        assert_eq!(Duty50Envelope::admit(400), Admit::At(400));
        // and both admit the 10% target this crate exists to hold
        assert_eq!(E::admit(100), Admit::At(100));
        assert_eq!(Duty50Envelope::admit(100), Admit::At(100));
    }

    #[test]
    fn a_carrier_change_preserves_the_duty_and_rescales_the_compare() {
        let mut p = DutyPath::new(STARTUP_TICKS, ENVELOPE_MAX, ENVELOPE_MIN);
        let startup_ccr = p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(100).unwrap();
        assert_eq!(startup_ccr, duty_to_ccr(STARTUP_TICKS, 100));
        p.set_ticks(RUN_PERIOD_TICKS);
        let run_ccr = p.recompute();
        assert_eq!(p.applied(), 100, "duty is a percentage and must survive");
        #[cfg(not(feature = "carrier-24k"))]
        assert_eq!(run_ccr, 133);
        #[cfg(feature = "carrier-24k")]
        assert_eq!(run_ccr, 266);
        assert!(run_ccr < startup_ccr, "same duty, shorter period, smaller compare");
    }

    #[test]
    fn a_zero_command_turns_the_drive_off_without_a_stop() {
        let mut p = path();
        p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(100).unwrap();
        let ccr = p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(0).unwrap();
        assert_eq!(ccr, 0);
        assert_eq!(p.applied(), 0);
    }

    #[test]
    fn no_admissible_command_can_produce_a_compare_above_the_period() {
        // A compare above ARR is a permanently-on bridge leg.
        for ticks in [RUN_PERIOD_TICKS, STARTUP_TICKS] {
            let mut p = DutyPath::new(ticks, ENVELOPE_MAX, ENVELOPE_MIN);
            for c in 0..=ENVELOPE_MAX {
                if let Ok(ccr) = p.command::<ENVELOPE_MIN, ENVELOPE_MAX, ENVELOPE_STEP>(c) {
                    assert!(ccr <= ticks, "ticks {ticks} cmd {c} -> ccr {ccr}");
                    // and never more of the period than the envelope ceiling
                    assert!(
                        ccr * 1000 <= ticks * ENVELOPE_MAX as u32 + 1000,
                        "ticks {ticks} cmd {c} -> ccr {ccr}"
                    );
                }
            }
        }
    }
}
