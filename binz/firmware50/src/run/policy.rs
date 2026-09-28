//! The production policy choices, as types (the rebuild task's
//! "Configuration design"): one marker type per decision the run makes, each
//! implementing the trait the [`super::Controller`] is generic over.
//!
//! Every number here was in the binary before E119 and is carried verbatim
//! with its derivation; the notebook entry named beside each is its source.

use crate::bemf::{FilterPolicy, FromMicros, MappedFilter, ReferenceFilterUs, WithShallowFloor, ZeroCross};
use crate::commutation::Reverse;
use crate::driven;
use crate::protection::{AverageCurrent, BusReference, FastBusSag, FoldbackGovernor, RAW_LIMIT};
use crate::report::{RunReport, Sink};

// ---------------------------------------------------------------------------
// The run's numbers (moved from the binary, E119)
// ---------------------------------------------------------------------------

/// Duty held during the static align, and the floor of the V/f line.
pub const VF_FLOOR_TENTHS: u16 = 10;

/// Electrical frequency at which the open-loop ramp hands over: 200, the
/// qualified image's *"50->200eHz startup staircase"* (`binz/AGENTS.md:293`).
pub const HANDOFF_EHZ: u32 = 200;

/// The matched speed window (campaign 8 step 3): the powered speed is
/// measured over a window that **ends at the last accepted crossing before
/// the stop**, so it can be compared with the coast that immediately follows
/// it. The anchor rolls forward whenever it is older than this, so a window is
/// between one and two of these long and its exact span is reported rather
/// than assumed. Whole-hold averages stay in the report as performance
/// statistics and gate nothing.
pub const TAIL_WINDOW_US: u32 = 2_000_000;

/// A qualifying run's total window, ms: ramp plus hold plus the coast.
///
/// **80 s, and the doc comment used to say 54.** It is the constant that
/// decides how long the bench is actually driven, and the safety argument for
/// the 50% cohort was written against a stale reading of it (E186 S1): with
/// `ramp.rs`'s 20.0 s ramp to 500 tenths, an `L` run at 50% holds 31-55 s and
/// spends about **57 s at or above 45%**. Five attempts is ~285 s of the
/// highest current this bench has run, with no thermal protection anywhere in
/// this firmware. `l` (explore) uses `BEMF_EXPLORE_MS` instead -- 45 s total,
/// ~20 s of hold, which `cohort.py`'s 30 s minimum hold then fails, so an
/// exploratory key cannot produce a qualifying run by construction.
/// **60 s since E330, down from 90.** The campaign's bar is a **>= 30 s**
/// actual-target hold, but a fixed 90 s window against `ramp.rs`'s
/// duty-scheduled ramp delivered 65-70 s of hold at every rung from 500 to 600
/// — so each attempt was more than **2x stricter than the requirement at more
/// than 2x the exposure**, and the LateArm hazard is per-second. At rung 600's
/// measured 0.0193/s that is P(complete) 0.32 against 0.51, i.e. the harness
/// was halving its own pass rate for nothing the bar asks for. Both E327
/// reviews flagged it independently as a defect nobody had decided.
///
/// **65 s, and the arithmetic is `hold = total - 5224 - ramp_us(rung)`.** My
/// first attempt set 60 s by computing 60 − 25 = 35 s of hold at rung 600 and
/// forgetting the ~5.2 s pre-closure phase that every run spends reaching the
/// 200 eHz handoff. The real hold was **29 776 ms — 224 ms under the 30 000
/// gate** (E331), which the fixture failed on that alone after an otherwise
/// clean `reason=2` run. The offset is exactly reproducible: every capture
/// shows `total_ms - closed_ms = 5224`.
///
/// **78 s, and the size is set by the RESTART, not by the holds** (E336). 65 s
/// gave 34.8 s of hold at rung 600 and cleared the bar comfortably — and could
/// not fit a rung-600 restart at all. `restart_campaign` reports
/// `need_ms = OFF + STARTUP + ramp_us(target) + MIN_HOLD` = **33 000** for
/// segment 2, while segment 1 at that rung consumes ~34 s of the window
/// (pre-closure, a 25 s ramp, the 2 s hold to the injection, and its report).
/// 31 s remaining against 33 s needed: infeasible, where 90 s had fitted.
///
/// So the window is sized by the hardest criterion the campaign asks for. At
/// 78 s the 600 restart has ~11 s of slack, and the holds become 47.8 s at rung
/// 600 and 52.8 s at 500 — *more* exposure than 65 s gave, which is also the
/// honest answer to the charge that 65 s graded on an easier curve: a
/// per-second hazard is tested harder here, not softer. Runs recorded at 90 s
/// remain valid evidence at their own, harder, exposure.
///
/// `hold = total - 5224 - ramp_us(rung)`; the 5224 ms pre-closure offset is
/// exactly reproducible in every capture.
///
/// It also cuts the thermal exposure this constant's previous comment warned
/// about by a third — five attempts is ~300 s at 60 s rather than ~450 s —
/// which matters because there is still **no thermal channel in `CHANNELS`**
/// (`src/hw/adc.rs:33-41`) and no thermal protection anywhere in this
/// firmware.
pub const BEMF_TOTAL_MS: u32 = 78_000;

/// The exploratory window: the same startup and ramp, about 10 s at target
/// (E137). One of these precedes every rung's 3/3 cohort, and it is not
/// judged on the 30 s dwell.
pub const BEMF_EXPLORE_MS: u32 = 45_000;

/// Duty held through catch and ramp, tenths of a percent: the reference's
/// `du62`/`catchdu62`.
pub const CATCH_DUTY_TENTHS: u16 = 62;

/// The goal's first rung, tenths of a percent (E075; 704 eHz, 58 mA).
pub const BEMF_DUTY_TENTHS: u16 = 150;

/// Open-loop speed at which the handover catch acquires: `run200` (E057).
pub const CATCH_EHZ: u32 = 200;

/// Driven-stage speed, whole eHz: the sine's hold speed, `run200`.
pub const DRIVEN_EHZ: u32 = 200;
/// The driven stage's phase rate, Q0.32 revolutions per µs.
pub const DRIVEN_RATE: u32 = driven::phase_rate(DRIVEN_EHZ * 100);
/// Driven vector lead over the sine angle: `drivephase60`.
pub const DRIVEN_PHASE_DEG: i32 = 60;
/// Driven duty, tenths of a percent: `drivedu61`.
pub const DRIVEN_DUTY_TENTHS: u16 = 61;

/// Samples used to learn the floating-phase offset before the during-run
/// rotation witness counts alternations.
pub const WITNESS_MID_SAMPLES: u32 = 64;
/// Hysteresis band for the during-run witness, raw ADC codes (E007).
pub const WITNESS_HYST_CODES: i32 = 12;

/// Duty held at the instant the loop closes, tenths of a percent.
pub const HANDOFF_DUTY_TENTHS: u16 = if VF_FLOOR_TENTHS > 70 { VF_FLOOR_TENTHS } else { 70 };

/// Shortest sector interval the estimator may track, µs: the physical
/// maximum (4166 eHz), not a design choice (E002, E020).
pub const SECTOR_FLOOR_US: u32 = 40;

/// Duty ceiling for every six-step plan, tenths of a percent.
///
/// 250 through campaign 5; campaign 6 (E137) raised it to 375 and campaign 9
/// to **500**, the commanded rung.
///
/// **It therefore no longer constrains anything, and the old justification is
/// withdrawn** (E181 SS6.4). While the cap sat below the rung it kept the
/// firmware from commanding past a 1 A supply by construction; at 500 on a
/// 2 A supply it equals what the run asks for, so the only thing standing
/// between the bench and an over-current is the PSU's own limit and the
/// firmware's current protection -- whose allowance is 4 A, i.e. twice the
/// supply (see `protection::RAW_LIMIT`). That is a real gap, named here rather
/// than papered over by a stale comment: nothing in the firmware stops a run
/// between 2 A and 4 A.
///
/// The `I ~ duty^2` law the old comment cited is also superseded: the measured
/// exponent across the campaign-7 rungs is ~2.4-2.9, and the 50% current is
/// measured (1836-1877 mA on the signed proxy), not extrapolated.
/// Raised 500 -> 525 in E235 and 525 -> 600 in E241. 525 measured clean 3/3
/// (hold 2032 mA, 68% of a 3 A clamp, rail in CV at 983 per mille, no
/// foldback), the current law was validated one rung beyond its fit, and the
/// remaining rungs are what determine the thin-margin rate's shape.
///
/// **The new ceiling is where the FIRMWARE's own allowance becomes reachable,
/// not an arbitrary target**: at 600 tenths the worst 10.1 ms block projects to
/// 3503-4222 mA against `RAW_LIMIT`'s 4000, so `AverageCurrent` may fold back
/// there -- and a foldback disqualifies the rung by the goal's own wording.
/// That is the prediction 550 and 575 test.
///
/// The original E235 reasoning, still the reason a cap raise is admissible at
/// all: every open question in campaign 10 is a question about behaviour above
/// 500 tenths, and nothing had ever been measured there. The current law, the
/// interval law, the droop law and the late-arm margin were all being
/// extrapolated, and three of the four extrapolations turned out wrong.
///
/// Not a protection threshold: the comment above records that this cap's
/// original justification is withdrawn. [`RAW_LIMIT`], [`FastBusSag`], the bus
/// floor, the hysteresis and the storm cap are untouched.
///
/// **Measured at 525** (E239, three runs): hold **2032 mA**, 68% of a 3 A
/// clamp with the rail in constant voltage at 983 per mille, no foldback, and
/// +5 to +6 us of arm margin against the measured modal spend of 6 us (E234,
/// 20 478 arms). The worst 10.1 ms block read **2609-3174 mA** -- and that is
/// a WHOLE-RUN figure including the ramp, not a hold figure
/// (`run/mod.rs`: a `CurrentMark`-windowed worst is owed since E194), so no
/// projection is built on it here. E241 did build one and E242 withdrew it.
// ENV-2: raised by exactly one 2.5 % increment. This is the campaign ceiling,
// enforced in the compare-value arithmetic so no key can exceed it -- not a
// protection. Current, sag, tracking, timing, bus floor and watchdog are all
// untouched. It clamps only ABOVE its value, so behaviour at every duty <= 600
// is unchanged by this edit.
// ENV-9: raised one more increment, 625 -> 650, the same way and for the same
// reason. Nothing else moves: every gate that judges the run is unchanged.
pub const SIXSTEP_DUTY_CAP: u16 = 650;

/// Rescue attempts the level revisit may make in one sector after its first
/// attempt was refused (E140), each armed by another half-interval of overdue.
///
/// The revisit is otherwise once per sector: `revisit_step` is set on the
/// attempt and cleared only by an accepted crossing. E138 showed that dead
/// end kills the drive -- a sector whose crossing the persistence filter
/// swallows never gets a second look, the commutation falls behind the rotor,
/// and the loop desyncs with no recovery. A rescue changes no gate: it only
/// pends the decision again, and the blanking gate and the filter still judge
/// the edge.
pub const REVISIT_RESCUE_MAX: u8 = 4;

/// Closed-loop time before a gate-4 injection fires (E080).
pub const INJECT_AFTER_US: u32 = 3_000_000;
/// Interrupt-masked stall for the tick-gap provocation (the guard allows 200).
pub const INJECT_STALL_US: u16 = 400;
/// Duty for the sag provocation: the reference's 50% rung.
///
/// **This is a load *step*, so it only provokes anything from a rung below
/// 50%** -- at the 50% rung it equals the commanded duty and gate 4 becomes a
/// no-op (E181 SS6.5). The positive control that the sag guard can still latch
/// on a given image is therefore run at 25%, where the step is a real one, and
/// a 50% cohort inherits that evidence rather than producing it. The
/// alternative -- stepping *above* 50% on a 2 A supply -- would provoke the
/// guard by driving the bench past its limit, which is the wrong direction to
/// buy a positive control in.
pub const INJECT_SAG_DUTY_TENTHS: u16 = 500;

/// Duty at or above which the sag provocation steps **relative to the rung**
/// instead of to the fixed [`INJECT_SAG_DUTY_TENTHS`].
///
/// The fixed 500 target is what every existing positive control used, and at
/// 15 / 25 / 37.5 / 47.5% it is a genuine upward step; it is preserved exactly
/// below this threshold. At and above it the fixed target is a step *down* --
/// an unload, which cannot provoke a sag guard in the direction the guard
/// trips -- so the step becomes relative and upward.
pub const INJECT_SAG_RELATIVE_FROM: u16 = 450;

/// How far above the rung the relative sag provocation steps, tenths.
pub const INJECT_SAG_STEP_TENTHS: u16 = 75;

/// Absolute bus floor, mV, and the BOOSTXL bus divider ratio x100.
pub const BUS_FLOOR_MV: u32 = 8_400;
pub const BUS_DIVIDER_X100: u32 = 1_194;
/// Admissible band for an undriven current-sense-amplifier average, raw codes.
pub const CSA_BIAS_MIN: u32 = 500;
pub const CSA_BIAS_MAX: u32 = 3_600;

/// Coast window, ms, and its comparator settings (E040, E065).
pub const COAST_WINDOW_MS: u32 = 1_500;
pub const COAST_HYST_CODES: i32 = 25;
pub const COAST_COMP_HYST: u8 = 1;
pub const COAST_DEBOUNCE_US: u32 = 40;

/// Sector interval, µs, for an electrical frequency (six sectors per cycle).
#[must_use]
pub const fn sector_interval_us(ehz: u32) -> u32 {
    if ehz == 0 {
        return u32::MAX / 4;
    }
    1_000_000 / (6 * ehz)
}

/// Compare value a six-step plan programs for this duty, in TIM1 ticks.
#[must_use]
pub const fn sixstep_ccr_of(duty_tenths: u16, period: u32) -> u32 {
    let d = if duty_tenths > SIXSTEP_DUTY_CAP {
        SIXSTEP_DUTY_CAP
    } else {
        duty_tenths
    };
    period * d as u32 / 1000
}

/// The OFF-window gate for the during-run witness: `cnt` inside the
/// carrier's OFF region, clear of both switching edges by a settle margin
/// scaled to the carrier (`64` ticks at `startup_ticks`).
#[must_use]
pub const fn in_off_window(cnt: u32, duty_ticks: u32, period: u32, startup_ticks: u32) -> bool {
    let s = 64 * period / startup_ticks;
    let settle = if s == 0 { 1 } else { s };
    let open = duty_ticks.saturating_add(settle);
    let close = period.saturating_sub(settle);
    open < close && cnt >= open && cnt <= close
}

// ---------------------------------------------------------------------------
// The policy traits
// ---------------------------------------------------------------------------

/// Closed-loop detector: the estimator the handover builds from the seed, and
/// the persistence filter COMP applies.
pub trait Bemf {
    type Filter: FilterPolicy;
    const FILTER: Self::Filter;
    fn estimator(seed_us: u32) -> ZeroCross;
}

/// Persistence depth for the in-ISR filter: the reference's 12 reads,
/// speed-scheduled since E083 (`map(average_interval, 100, 500, 3, 12)` with
/// the very-fast floor, in the reference's half-µs, adapted by `FromMicros`):
/// 12 reads through 15%, 8 at 20%, 7 at 25%. Clamped at 12, so the audited
/// loop bound is unchanged. The COMP root uses this constant.
pub const DET_FILTER: ReferenceFilterUs = FromMicros(WithShallowFloor(MappedFilter));

/// Advance, sixty-fourths of a half cycle, for an applied duty.
pub trait Advance {
    fn level(duty_tenths: u16) -> u32;
}

/// Average-current protection and the foldback it drives.
pub trait CurrentLimit {
    fn meter(zero_block: u32) -> AverageCurrent;
    fn governor(target_tenths: u16) -> FoldbackGovernor;
}

/// The fast bus-sag hard stop.
pub trait SagLimit {
    fn watch(bus_ref: BusReference) -> FastBusSag;
}

/// Ordinary-start recovery: when a restart is admitted and how it runs.
pub trait RestartRule {
    /// Bridge-off time before the restart.
    const OFF_US: u32;
    /// Ordinary startup to transfer.
    const STARTUP_US: u32;
    /// Hold at target the recovery must still have room for.
    const MIN_HOLD_US: u32;
    /// When, after the ramp reaches target, the campaign's loss is injected.
    const INJECT_AT_TARGET_US: u32;
    fn policy() -> crate::restart::Restart;
}

/// How a run's evidence is written, once, after `safe_off`.
pub trait Reporting {
    fn emit(report: &RunReport, out: &mut impl Sink);
}

// ---------------------------------------------------------------------------
// The production choices
// ---------------------------------------------------------------------------

/// How this bench is wired: the reference's `bench-reverse-phases`
/// permutation (`binz/AGENTS.md:18`, E010). The ISR roots use this same type.
pub type Wiring = Reverse;

/// The reference detector: seeded at the driven interval, bounded
/// `[SECTOR_FLOOR_US, 1.5 x seed]` (E002, E020).
pub struct BemfPolicy;
impl Bemf for BemfPolicy {
    type Filter = ReferenceFilterUs;
    const FILTER: ReferenceFilterUs = DET_FILTER;
    fn estimator(seed_us: u32) -> ZeroCross {
        ZeroCross::new_bounded(seed_us, SECTOR_FLOOR_US, seed_us + seed_us / 2)
    }
}

/// The duty at or above which the high advance level applies: 35%.
pub const ADVANCE_STEP_TENTHS: u16 = 350;

/// The advance level below [`ADVANCE_STEP_TENTHS`].
#[cfg(not(any(feature = "advance-ref", feature = "advance-18")))]
pub const ADVANCE_LOW: u32 = 20;
/// ENV-7: a flat 18, the cheapest lever between the reference 16 (arm margin)
/// and the 20 carrier (speed). A constant, not a code change.
#[cfg(all(feature = "advance-18", not(feature = "advance-ref")))]
pub const ADVANCE_LOW: u32 = 18;
/// **`advance-ref` flattens the schedule to the REFERENCE value, 16** (E330).
///
/// E326 tried advance 16 at the high end only, `ADVANCE_HIGH >= ADVANCE_LOW`
/// refused it, and I reverted — framing 20 as the baseline and 16 as an exotic
/// reduction. That was backwards. `src/commutation.rs:238` is
/// `pub type DefaultAdvance = FixedAdvance<16>` and AM32's `Src/main.c:656` is
/// `temp_advance = 16`: **16 is the reference, and this campaign's 20/22 is a
/// divergence upward.** The rising schedule itself is firmware50's, not the
/// reference's, which uses one fixed advance.
///
/// Why it is the arithmetically complete fix. At a flat 16,
/// `wait = ci/2 - ci*16/64 = ci/4`, so `wait >= 10` for **every `ci` at or
/// above `SECTOR_FLOOR_US = 40`** — and every recorded latch had
/// `spent_at_late = 9` with the chain corpus capping per-arm `spent` at 10. So
/// **no descent the estimator's own clamp permits can reach `left == 0`.**
/// Advance 20 covers only `ci >= 50` and 18 only `ci >= 42`, and E322 measured
/// the descent chasing exactly that boundary downward (22 -> 20 moved the latch
/// from ci 49/50 to 46, and floor 5 then latched at 46-48).
///
/// And lower advance *cuts* current, which the metered bracket says is the
/// other binding constraint at rung 600.
///
/// **What it owns.** It changes the ramp's advance too, which is why E326
/// backed away: three tests pin `ADVANCE_LOW` and the A/B comment says
/// "changing both at once would make the result unattributable". That
/// discipline is right for isolating a *level*, but this is not that
/// experiment — it is a return to the reference schedule, and it must be
/// judged as one change with both ends stated rather than smuggled through
/// the high end.
#[cfg(feature = "advance-ref")]
pub const ADVANCE_LOW: u32 = 16;

/// The advance level at and above [`ADVANCE_STEP_TENTHS`]. **Production is 22.**
///
/// The `advance-low` feature drops it to 20, for the A/B E300 predeclared. The
/// reason that A/B exists: `wait_time(ci, level) = ci * (32 - level) / 64` must
/// exceed the measured 11 µs arm cost, and by the **real integer** function
/// (`src/commutation.rs`, mirrored in `scripts/chain.py`) level 22 clears it
/// only above `ci` = 72 (always 74) while level 20 clears it above 60
/// (always 62) -- about **12 µs of interval headroom, ~3 rungs**.
///
/// Measured need: `thin_count` (`wait − spent <= 2 µs`) is 0 through rung 350
/// and then 8 / 11 / 121 / **688** / **4118** per 10⁶ accepted commutations at
/// rungs 425 / 450 / 475 / 500 / 525 (E303 corrects E300's 1062 at rung 500:
/// no emitted denominator yields that figure, and the corrected series is
/// smoother -- 5.7x then 6.0x rather than a spurious 8.8x then 3.8x).
///
/// Two labels in E298/E300 were also wrong and are corrected here. `ci_us` is
/// **the estimate at the stop**, not a mean (`report.rs`), so "mean ci" in
/// those entries is the mean over runs of a *last value*; the firmware's own
/// hold mean is `mean_ci_us`, and at rung 525 it reads **74**, one µs
/// *above* the always-clear line rather than sitting on it. And
/// `spent_max_us = 11` is a **whole-run saturating maximum**, not a typical
/// arm cost -- the mode is ~6 -- so every margin derived from it is a
/// worst-case margin, and it is image-specific (10-24 across older images).
///
/// `late_arms` is still 0 at every clean rung.
///
/// **This is a control schedule, not a protection threshold**, and it is the
/// "bounded control improvement" the campaign goal asks for. Nothing about the
/// guards, their fractions, their streaks or their latches is touched.
#[cfg(not(any(feature = "advance-low", feature = "advance-ref", feature = "advance-18")))]
pub const ADVANCE_HIGH: u32 = 22;
#[cfg(all(feature = "advance-low", not(any(feature = "advance-ref", feature = "advance-18"))))]
pub const ADVANCE_HIGH: u32 = 20;
/// ENV-7 flat 18; see [`ADVANCE_LOW`].
#[cfg(all(feature = "advance-18", not(feature = "advance-ref")))]
pub const ADVANCE_HIGH: u32 = 18;
/// The reference value at both ends; see [`ADVANCE_LOW`]. Takes precedence
/// over `advance-low` so enabling both is not a silent conflict.
#[cfg(feature = "advance-ref")]
pub const ADVANCE_HIGH: u32 = 16;

/// The qualified image's schedule: 20 below 35% duty, 22 at/above
/// (`binz/AGENTS.md:133`).
pub struct AdvancePolicy;
impl Advance for AdvancePolicy {
    fn level(duty_tenths: u16) -> u32 {
        if duty_tenths >= ADVANCE_STEP_TENTHS {
            ADVANCE_HIGH
        } else {
            ADVANCE_LOW
        }
    }
}

// Bounds on what this campaign is willing to build, and **not** a mirror of any
// core clamp -- `advance_of` clamps only at `level > 64` (`commutation.rs:254`),
// so nothing downstream would stop a wild value; it would just quietly
// re-scale the advance. (I first wrote this comment claiming the core clamps
// 18..=22. That is a *binz* fact, not a firmware50 one, and citing it here
// without re-reading the source is exactly
// [[feedback-dont-inherit-scars-unverified]].)
//
// 22 is the production high level and 16 is `DefaultAdvance = FixedAdvance<16>`,
// the reference value, so the reference-to-production span is the legitimate
// range for an A/B. Anything outside it is a new question, not a variant.
const _: () = assert!(ADVANCE_HIGH >= 16 && ADVANCE_HIGH <= 22);
const _: () = assert!(ADVANCE_LOW >= 16 && ADVANCE_LOW <= 22);
const _: () = assert!(ADVANCE_HIGH >= ADVANCE_LOW);
// The step must stay where production put it: this A/B varies the *level*, not
// the duty at which the schedule changes. Changing both at once would make the
// result unattributable.
const _: () = assert!(ADVANCE_STEP_TENTHS == 350);

/// `AverageCurrent` at the nominal allowance, foldback to the V/f floor.
pub struct CurrentProtection;
impl CurrentLimit for CurrentProtection {
    fn meter(zero_block: u32) -> AverageCurrent {
        AverageCurrent::new(zero_block, RAW_LIMIT)
    }
    fn governor(target_tenths: u16) -> FoldbackGovernor {
        FoldbackGovernor::new(target_tenths.max(VF_FLOOR_TENTHS), VF_FLOOR_TENTHS)
    }
}

/// The fast bus-sag stop against this run's bridge-off reference.
pub struct BusSagProtection;
impl SagLimit for BusSagProtection {
    fn watch(bus_ref: BusReference) -> FastBusSag {
        FastBusSag::new(bus_ref)
    }
}

/// The reference's one-shot ordinary restart (`binz/NORMAL_RESTART_E770.md`).
pub struct Restart;
impl RestartRule for Restart {
    const OFF_US: u32 = 1_000_000;
    const STARTUP_US: u32 = 5_000_000;
    const MIN_HOLD_US: u32 = 2_000_000;
    const INJECT_AT_TARGET_US: u32 = 2_000_000;
    fn policy() -> crate::restart::Restart {
        crate::restart::Restart::new()
    }
}

/// The compact post-run report the fixture parses.
pub struct Telemetry;
impl Reporting for Telemetry {
    fn emit(report: &RunReport, out: &mut impl Sink) {
        report.emit(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_match_the_values_the_binary_used() {
        assert_eq!(sector_interval_us(200), 833);
        assert_eq!(sector_interval_us(0), u32::MAX / 4);
        assert_eq!(sixstep_ccr_of(250, 1333), 333);
        // Clamped at `SIXSTEP_DUTY_CAP`: 37.5% from E137, 50% from E147, and
        // 52.5% from E235 -- one rung above the qualified 50%, so that the
        // current, interval, droop and late-arm laws can be measured there
        // instead of extrapolated.
        //
        // This assertion is why the cap raise could not be silent: it failed
        // the moment the constant moved, which is the gate doing its job.
        assert_eq!(sixstep_ccr_of(375, 1333), 499);
        assert_eq!(sixstep_ccr_of(500, 1333), 666);
        assert_eq!(sixstep_ccr_of(525, 1333), 699);
        assert_eq!(sixstep_ccr_of(600, 1333), 799);
        // ENV-2 raised the cap 600 -> 625 and this assertion failed, exactly as
        // its comment above promises -- but I committed without running the
        // firmware50 suite (the pre-commit hook tests only the rm32 crate), so
        // the tree went red and the review caught it, not me. Rewritten so the
        // property survives: the cap VALUE is pinned, so the next move trips
        // this again, and anything above the cap clamps to the cap.
        assert_eq!(
            SIXSTEP_DUTY_CAP, 650,
            "a cap move must be deliberate: update this with it"
        );
        assert_eq!(sixstep_ccr_of(625, 1333), 833);
        assert_eq!(sixstep_ccr_of(650, 1333), 866);
        assert_eq!(
            sixstep_ccr_of(750, 1333),
            sixstep_ccr_of(SIXSTEP_DUTY_CAP, 1333),
            "clamped at the cap"
        );
        // Below the step is 20 in **both** builds; at and above it the
        // `advance-low` feature is the only thing that moves. Asserted against
        // the constants rather than literals so the default build still pins
        // 20/22 while the A/B build is not a test failure.
        assert_eq!(AdvancePolicy::level(349), ADVANCE_LOW);
        assert_eq!(AdvancePolicy::level(350), ADVANCE_HIGH);
        // "Below the step never varies" held while the only variant moved the
        // high end. `advance-ref` returns BOTH ends to the reference 16 (E330),
        // so the claim is now configuration-scoped rather than absolute.
        #[cfg(not(any(feature = "advance-ref", feature = "advance-18")))]
        assert_eq!(AdvancePolicy::level(349), 20, "below the step, default");
        #[cfg(all(feature = "advance-18", not(feature = "advance-ref")))]
        assert_eq!(AdvancePolicy::level(349), 18, "below the step, flat 18");
        #[cfg(feature = "advance-ref")]
        assert_eq!(AdvancePolicy::level(349), 16, "below the step, reference");
        #[cfg(not(any(feature = "advance-low", feature = "advance-ref", feature = "advance-18")))]
        assert_eq!(AdvancePolicy::level(350), 22, "production high level");
        #[cfg(all(feature = "advance-low", not(any(feature = "advance-ref", feature = "advance-18"))))]
        assert_eq!(AdvancePolicy::level(350), 20, "advance-low variant");
        #[cfg(all(feature = "advance-18", not(feature = "advance-ref")))]
        assert_eq!(AdvancePolicy::level(350), 18, "advance-18 variant");
        #[cfg(feature = "advance-ref")]
        assert_eq!(AdvancePolicy::level(350), 16, "advance-ref: the reference value");
        // The step itself is fixed, so an A/B varies one thing.
        assert_eq!(AdvancePolicy::level(ADVANCE_STEP_TENTHS - 1), ADVANCE_LOW);
        assert_eq!(AdvancePolicy::level(ADVANCE_STEP_TENTHS), ADVANCE_HIGH);
        assert_eq!(HANDOFF_DUTY_TENTHS, 70);
    }

    #[test]
    fn off_window_needs_room_for_both_settle_margins() {
        // 48 kHz run carrier, 25% duty: 333 ticks on, settle 13.
        assert!(!in_off_window(300, 333, 1333, 6400));
        assert!(in_off_window(400, 333, 1333, 6400));
        assert!(!in_off_window(1330, 333, 1333, 6400));
        // A duty with no OFF window left.
        assert!(!in_off_window(1300, 1320, 1333, 6400));
    }
}
