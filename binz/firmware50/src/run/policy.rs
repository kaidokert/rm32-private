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

/// The run window: 44 s from E083 to campaign 5 (4.72 s startup + 7.5 s ramp
/// to 25% + 31.8 s hold at target). **54 s from E137**, because the ramp is
/// 1%/0.5 s from 10% and reaching 37.5% costs 14.0 s rather than 7.5 s, so a
/// 30 s dwell no longer fits in 44 s. It stays inside `GUARD_CAMPAIGN_US`,
/// which moves with it and keeps its margin; the backstop only catches a
/// foreground that never ends a run and is not a fault threshold.
pub const BEMF_TOTAL_MS: u32 = 80_000;

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
/// 250 through campaign 5. Campaign 6 (E137) raises it to **375**, the goal's
/// ceiling: the bench's 1 A supply reaches its limit near there (E136's
/// measured `I ∝ duty²` puts 37.5% at about 0.90 A), so the clamp keeps the
/// firmware from commanding past the supply by construction.
pub const SIXSTEP_DUTY_CAP: u16 = 500;

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
pub const INJECT_SAG_DUTY_TENTHS: u16 = 500;

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

/// The qualified image's schedule: 20 below 35% duty, 22 at/above
/// (`binz/AGENTS.md:133`).
pub struct AdvancePolicy;
impl Advance for AdvancePolicy {
    fn level(duty_tenths: u16) -> u32 {
        if duty_tenths >= 350 {
            22
        } else {
            20
        }
    }
}

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
        // Clamped at `SIXSTEP_DUTY_CAP`: 37.5% from E137, 50% from E147, which
        // is campaign 7's ceiling and the supply's.
        assert_eq!(sixstep_ccr_of(375, 1333), 499);
        assert_eq!(sixstep_ccr_of(500, 1333), 666);
        assert_eq!(sixstep_ccr_of(750, 1333), 666, "capped at 50%");
        assert_eq!(AdvancePolicy::level(349), 20);
        assert_eq!(AdvancePolicy::level(350), 22);
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
