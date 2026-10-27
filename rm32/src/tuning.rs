//! Tuning policy: the rig-qualified divergences from AM32, as a type.
//!
//! The L431 and binz rigs were qualified with three behaviours AM32 does
//! not have: a wrong-phase-orbit current trip, a reclimb duty clamp, and a
//! fast-rotor desync response that stays in interrupt mode with a half
//! duty kick. A port selects them with `Platform::Tuning`. Every value is an
//! associated const, so `if let Some(c) = T::ORBIT_TRIP` and
//! `match T::DESYNC` resolve at compile time: under [`Am32Tuning`] the rig
//! code compiles out, under [`QualifiedRigTuning`] it folds to the same
//! immediates the qualified images were built with.

/// Wrong-phase-orbit trip (divergence from AM32): the acceptance chain can
/// self-clock on switching artifacts in a wrong phase -- plausible
/// zero-crossings, current far above the duty-proportional norm, no
/// desync-detector jump. The trip is relative (fires when current exceeds
/// `duty * slope + offset_ma`, sustained `trip_ms` ticks while locked)
/// because a static ampere threshold cannot separate a wrong-phase orbit at
/// 70 % from legitimate 100 % draw. Response = full desync path: the demote
/// IS the phase reset.
pub struct OrbitTripConfig {
    pub slope: i32,
    pub offset_ma: i32,
    /// Consecutive 1 kHz ticks over the line before tripping (ms).
    pub trip_ms: u16,
    /// Rail voltage (cV) the line was calibrated on; the line scales by
    /// measured vbat relative to it (battery sessions run ~12 V where the
    /// same duty legitimately draws ~1.5x the current).
    pub cal_vbat_cv: i32,
    /// Commanded-transient suppression: while the applied duty moved more
    /// than this within the snapshot window, high current is expected (slam
    /// accel, ~10 A on battery) and the trip holds off.
    pub transient_duty: u16,
}

/// Reclimb clamp (divergence from AM32): until `confirm_zc` zero crossings
/// confirm the lock, the duty ceiling is capped at `duty_cap`. Covers fresh
/// engage and post-fall recovery identically; the cap clears the startup
/// duty band while bounding the recovery acceleration surge.
pub struct ReclimbConfig {
    pub confirm_zc: u32,
    pub duty_cap: u16,
}

/// What a desync from interrupt mode does.
pub enum DesyncPolicy {
    /// AM32: demote to polling and drop the duty to min_startup/2.
    Am32,
    /// Fast-rotor desyncs (commutation interval below `max_ci_us`, current
    /// sane) stay in interrupt mode with a half duty kick. AM32 re-locks
    /// within ~1 ms at main-loop rate; rm32 polls on the 20 kHz tick grid,
    /// which cannot re-lock above ~800 Hz e, so a demoted fast rotor coasts
    /// for seconds. The detector then holds off for `rearm_holdoff_zc` zero
    /// crossings: the kick's own deceleration would refire it against a
    /// stale average (a self-loop); a real desync still trips on the first
    /// check past the holdoff.
    HalfKickStayInterrupt {
        max_ci_us: u32,
        rearm_holdoff_zc: u32,
    },
}

pub trait TuningPolicy {
    const ORBIT_TRIP: Option<OrbitTripConfig>;
    const RECLIMB: Option<ReclimbConfig>;
    const DESYNC: DesyncPolicy;
}

/// AM32's behaviour: no rig divergence.
pub struct Am32Tuning;

impl TuningPolicy for Am32Tuning {
    const ORBIT_TRIP: Option<OrbitTripConfig> = None;
    const RECLIMB: Option<ReclimbConfig> = None;
    const DESYNC: DesyncPolicy = DesyncPolicy::Am32;
}

/// The constants the L431 and binz rigs were qualified with.
pub struct QualifiedRigTuning;

impl TuningPolicy for QualifiedRigTuning {
    const ORBIT_TRIP: Option<OrbitTripConfig> = Some(OrbitTripConfig {
        slope: 3,
        offset_ma: 2500,
        trip_ms: 30,
        cal_vbat_cv: 816,
        transient_duty: 150,
    });
    const RECLIMB: Option<ReclimbConfig> = Some(ReclimbConfig {
        confirm_zc: 1500,
        duty_cap: 800,
    });
    const DESYNC: DesyncPolicy = DesyncPolicy::HalfKickStayInterrupt {
        max_ci_us: 300,
        rearm_holdoff_zc: 100,
    };
}

/// Wrong-phase-orbit discriminator: current far above the
/// duty-proportional norm. The line scales with measured vbat since
/// current at a given duty scales with source voltage. Always false
/// without an orbit trip.
#[inline]
pub(crate) fn orbit_current<T: TuningPolicy>(current_ma: i16, duty: u16, vbat_cv: u16) -> bool {
    let Some(c) = T::ORBIT_TRIP else {
        return false;
    };
    let line = (duty as i32) * c.slope + c.offset_ma;
    let scaled = if vbat_cv > 600 {
        line * (vbat_cv as i32) / c.cal_vbat_cv
    } else {
        line
    };
    (current_ma as i32) > scaled
}
