//! The guard suite — every protective decision the firmware makes,
//! as pure logic. Each guard's constants trace to a measured
//! incident; the tests ARE the incident regression suite.
//!
//! Kill-authority note (2026-07-10 burn post-mortem): everything
//! here is software and dies with a wedged MCU. The IWDG and the
//! bench PSU current limit are the layers below; these guards only
//! matter while the core runs.

/// Wrapping "time since" with the cross-ISR read-order rule: the
/// caller must load the REFERENCE before `now` (a commutation ISR
/// preempting between the reads makes `last` newer than `now`, the
/// subtraction underflows to ~4×10⁹, and a healthy lock gets
/// killed — v3.x's phantom "acceleration envelope limit"). The
/// top-bit clamp is belt and suspenders.
pub fn since_us(now_ticks_10us: u32, last_ticks_10us: u32) -> u32 {
    let d = now_ticks_10us.wrapping_sub(last_ticks_10us);
    if d < u32::MAX / 2 {
        d.saturating_mul(10)
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kill {
    /// No accepted qZC for 12 intervals under CL: the loop is flying
    /// blind (stalled rotor / zombie field). The stalled-rotor case
    /// draws little current and never stops commutating — nothing
    /// else can see it.
    ZcStarved,
    /// Interval below the physical floor: an estimator walked down
    /// there is commutating its own noise; junk accepts keep the
    /// starvation guard content. Floor re-scoped 160 → 60 µs when a
    /// healthy 1,050 Hz lock was executed by the stale value
    /// (regime-expired guard).
    Runaway,
    /// Commutation chain silent > 3 intervals.
    Desync,
}

/// CL-mode watchdog decision for one drive tick.
/// R3 — the watchdog verdict: recoverable sync-losses become
/// duty-clamped RESEEDS (AM32 semantics: it re-seeds where we
/// killed — zero kill paths vs our four); only the unrecoverable
/// classes still kill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogAction {
    None,
    /// Cut duty to the reseed floor, enter re-acquisition, re-ramp
    /// after fresh accepts. The caller counts strikes.
    Reseed(Kill),
    /// Hard stop (r/q re-arms): the 22.5 ms backstop, the runaway
    /// floor, or strike escalation.
    Kill(Kill),
}

/// R3 backstop: no accepted ZC for this long is not a transient —
/// AM32's own last-resort timeout (45000 x 0.5 us ticks).
pub const BACKSTOP_NO_ZC_US: u32 = 22_500;
/// Reseed exits after this many fresh accepts (one electrical rev).
pub const RESEED_EXIT_ACCEPTS: u8 = 6;
/// Strike escalation: this many reseeds without amnesty -> kill.
pub const RESEED_MAX_STRIKES: u8 = 4;
/// Amnesty: this many accepts since the last reseed clears strikes
/// (AM32's zero_crosses > 1000 analog — a healthy lock never
/// accrues strikes).
pub const RESEED_AMNESTY_ACCEPTS: u32 = 1_000;
/// Commanded-amp floor while reseeding (~AM32's min_startup_duty/2).
pub const RESEED_AMP_PCT: u16 = 6;

/// R3 verdict. Same inputs as [`cl_watchdog`] + the strike count.
#[inline]
/// WAIT CLAMP (rotor-clocked world): a ZC-miss under the inversion
/// means the loop WAITS with duty parked on one sector while the
/// rotor coasts - BEMF-aided current ramps on L/R (levers2: lk=4
/// sag kill through a single-reseed transit). The blind-amp clamp
/// cannot see it: it keys on window closes and no closes happen
/// during a wait. This clamp keys on time-since-last-accept
/// directly: full amp to 2.5 intervals (normal jitter + late-ZC
/// tolerance), half to 5, quarter beyond. Restores instantly on
/// the next accept (the caller's amp pipeline re-evaluates every
/// tick).
#[inline]
pub fn wait_amp_clamp(amp: u16, since_qzc_us: u32, interval_us: u32) -> u16 {
    if interval_us == 0 {
        return amp;
    }
    let iv = interval_us.max(100);
    if since_qzc_us > iv * 5 {
        amp / 4
    } else if since_qzc_us * 2 > iv * 5 {
        amp / 2
    } else {
        amp
    }
}

/// IN-HOLD DUTY CUT predicate (wait clamp v2, autopsy-derived): a
/// healthy accept lands ~0.5x interval into its window and the next
/// commutation by ~1.0x - past 1.0x with no qZC the hold is already
/// abnormal, and at the measured ~2.5 A/PWM-cycle pump rate the
/// duty must cut BEFORE the 1.25x bounded-wait step (rimax=379 =
/// 9.7 A inside a time-bounded hold; fstart1). The parked v1 clamp
/// keyed at 2.5x - too LATE, not too eager - and cut 50 % - too
/// weak. False-fire surface: a legit late accept in (1.0x, 1.25x)
/// costs one floor-duty window, self-restoring - early, deep, and
/// cheap to be wrong about.
/// whcut2 autopsy: at 1.0x the threshold had ZERO margin against
/// the estimator's climb lag (~3% - estimate 146 vs actual 150 us),
/// so the cut fired EVERY window at ~1130 Hz (wcut=98, rsq=0,
/// slew=1657) and the loop strangled itself to a sag at floor duty.
/// 1.125x sits above the lag band (~1.05x) and below the 1.25x
/// step; the cap moves ~3A -> ~4-5A, still far under the 9.7A the
/// cut exists to prevent.
#[inline]
pub fn wait_cut_due(since_comm_us: u32, interval_us: u32) -> bool {
    interval_us != 0 && since_comm_us > interval_us + interval_us / 8
}

pub fn cl_watchdog_r3(
    interval_us: u32,
    since_last_qzc_us: u32,
    since_last_comm_us: u32,
    have_qzc_ref: bool,
    strikes: u8,
) -> WatchdogAction {
    if interval_us == 0 {
        return WatchdogAction::None;
    }
    if have_qzc_ref && since_last_qzc_us > BACKSTOP_NO_ZC_US {
        return WatchdogAction::Kill(Kill::ZcStarved);
    }
    if interval_us < 45 {
        return WatchdogAction::Kill(Kill::Runaway);
    }
    let starved = have_qzc_ref && since_last_qzc_us > interval_us.max(500) * 12;
    let desynced = since_last_comm_us > interval_us.max(1_000) * 3;
    if starved || desynced {
        let why = if starved {
            Kill::ZcStarved
        } else {
            Kill::Desync
        };
        if strikes >= RESEED_MAX_STRIKES {
            return WatchdogAction::Kill(why);
        }
        return WatchdogAction::Reseed(why);
    }
    WatchdogAction::None
}

pub fn cl_watchdog(
    interval_us: u32,
    since_last_qzc_us: u32,
    since_last_comm_us: u32,
    have_qzc_ref: bool,
) -> Option<Kill> {
    if interval_us == 0 {
        return None;
    }
    if have_qzc_ref && since_last_qzc_us > interval_us.max(500) * 12 {
        return Some(Kill::ZcStarved);
    }
    // E5 (CONSTANTS_AUDIT 2026-07-15): floor 60 -> 45. At 60 the
    // guard collided with the loop's own LEGAL dynamics below 100 us
    // intervals: the accept rate bound permits 0.6x (84 -> 50 us) and
    // the reacq re-seed 0.5x (84 -> 42 us), so a single permitted
    // fast step could land under the floor and kill a healthy lock.
    // 45 clears one legal 0.6x accept from anywhere >= 75 us while
    // still killing scheduler-junk runaways (the 24-50 us band a
    // detached-field runaway self-feeds into). Re-seed at 0.5x from
    // < 90 us can still cross it - acceptable: a genuine re-seed that
    // halves from 90 us is indistinguishable from a runaway anyway.
    if interval_us < 45 {
        return Some(Kill::Runaway);
    }
    if since_last_comm_us > interval_us.max(1_000) * 3 {
        return Some(Kill::Desync);
    }
    None
}

/// Supply-sag kill: −10 % from the arm-time (unloaded) baseline, or
/// the absolute brownout backstop, whichever is higher — debounced
/// over consecutive sub-threshold samples. A real supply sag lasts
/// milliseconds; a single corrupted injected sample does not (the
/// undebounced version false-tripped with its own report reading
/// "8130 mV < 90 % of 8107 mV" — the trip sample was a one-off
/// glitch and the message printed the recovered value).
#[derive(Debug, Clone, Copy)]
pub struct SagGuard {
    pub baseline_raw: u16,
    run: u16,
    /// The raw value that actually tripped, latched for reporting.
    pub trip_raw: u16,
}

/// ≈5.95 V: below this the 3.3 V rail is one transient from the
/// brownout that WEDGES the MCU with the bridge frozen (the burn).
pub const VBAT_ABS_FLOOR_RAW: u16 = 793;
/// Consecutive sub-threshold samples required. Sample rate = the PWM
/// carrier (vbat rides the per-cycle injected burst), so 64 samples =
/// **2.67 ms at the flashed 24 kHz** (1.3 ms at 48 kHz). E6 audit
/// note: this is CARRIER-RELATIVE by construction; 2.67 ms is the
/// bench-proven value every 24 kHz envelope result was earned on
/// (rides through 1-2 ms spike bursts, kills sustained collapse).
/// Re-derive per carrier if the sag latency matters at 48 kHz.
pub const SAG_DEBOUNCE: u16 = 64;

impl SagGuard {
    pub const fn new() -> Self {
        Self {
            baseline_raw: 0,
            run: 0,
            trip_raw: 0,
        }
    }

    /// Capture the unloaded baseline at arm; re-arming after a
    /// bench-dial change re-baselines automatically.
    pub fn arm(&mut self, unloaded_raw: u16) {
        self.baseline_raw = unloaded_raw;
        self.run = 0;
    }

    pub fn threshold_raw(&self) -> u16 {
        (self.baseline_raw - self.baseline_raw / 10).max(VBAT_ABS_FLOOR_RAW)
    }

    /// Feed one pump sample (only while the drive is armed). Returns
    /// true when the kill should fire.
    pub fn sample(&mut self, raw: u16) -> bool {
        if raw < self.threshold_raw() {
            self.run += 1;
            if self.run >= SAG_DEBOUNCE {
                self.trip_raw = raw;
                self.run = 0;
                return true;
            }
        } else {
            self.run = 0;
        }
        false
    }
}

impl Default for SagGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// The ISR-kill flag matrix (roadmap E4). Three ISR kill sites used
/// to hand-maintain slightly different flag sets — the OC
/// zombie-status incident was exactly one missing `CL_ACTIVE` line,
/// and the TIM7 watchdog kill was missing `CL_ARMED`. One function,
/// one matrix, host-pinned.
pub struct KillFlags<'a> {
    pub motor_enabled: &'a portable_atomic::AtomicBool,
    pub cl_active: &'a portable_atomic::AtomicBool,
    pub cl_armed: &'a portable_atomic::AtomicBool,
    pub cl_desync: &'a portable_atomic::AtomicBool,
    pub cl_starved: &'a portable_atomic::AtomicBool,
    pub oc_tripped: &'a portable_atomic::AtomicBool,
    pub vbat_sagged: &'a portable_atomic::AtomicBool,
    pub bb_frozen: &'a portable_atomic::AtomicBool,
    /// Last kill cause (1=desync 2=starved 3=oc 4=sag; 0=never).
    /// ALWAYS-visible in the i-echo: a silent CL death (levers1
    /// incident: loop dead, zero kill prints) must be attributable
    /// without depending on the print path having survived.
    pub last_kill: &'a portable_atomic::AtomicU8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsrKillKind {
    Desync,
    Starved,
    Overcurrent,
    Sag,
}

/// Apply the complete flag set for an ISR kill. The caller performs
/// the hardware actions (`all_off()`, COMP EXTI mask) immediately
/// after — a ~µs of flags-before-FETs is harmless (main's microloop
/// is 1 ms), while flags-after-FETs risks a preempting context
/// seeing a dead motor with CL alive. The main-notification flag
/// (`cl_desync`/`oc_tripped`/`vbat_sagged`) is stored LAST so main
/// never reports before the payload flags are consistent.
pub fn apply_isr_kill(kf: &KillFlags<'_>, kind: IsrKillKind) {
    use portable_atomic::Ordering::Relaxed;
    kf.motor_enabled.store(false, Relaxed);
    kf.cl_active.store(false, Relaxed);
    kf.cl_armed.store(false, Relaxed);
    kf.last_kill.store(
        match kind {
            IsrKillKind::Desync => 1,
            IsrKillKind::Starved => 2,
            IsrKillKind::Overcurrent => 3,
            IsrKillKind::Sag => 4,
        },
        Relaxed,
    );
    match kind {
        IsrKillKind::Desync | IsrKillKind::Starved => {
            // Freeze the black box so the dump shows the lead-up.
            kf.bb_frozen.store(true, Relaxed);
            kf.cl_starved.store(kind == IsrKillKind::Starved, Relaxed);
            kf.cl_desync.store(true, Relaxed);
        }
        IsrKillKind::Overcurrent => kf.oc_tripped.store(true, Relaxed),
        IsrKillKind::Sag => kf.vbat_sagged.store(true, Relaxed),
    }
}

/// Load-run-store form of [`SagGuard::sample`] for the firmware's
/// atomic-backed state (baseline + run live in atomics; `trip` tells
/// the caller to latch the raw and kill). Semantics single-sourced
/// through SagGuard so its regression suite covers this path.
/// R4a — sag debounce parameterized by the LIVE carrier's sample
/// count (timing::sag_debounce_samples): the sample rate is the PWM
/// wrap rate, so a fixed 64-count debounce would shrink from the
/// proven 2.67 ms to 1.33 ms at 48 kHz and start killing benign
/// spike-burst dips the envelope work always rode through.
pub fn sag_step_scaled(baseline_raw: u16, run: u16, raw: u16, debounce: u16) -> (u16, bool) {
    let threshold = (baseline_raw - baseline_raw / 10).max(VBAT_ABS_FLOOR_RAW);
    if raw < threshold {
        let run = run + 1;
        if run >= debounce {
            (0, true)
        } else {
            (run, false)
        }
    } else {
        (0, false)
    }
}

pub fn sag_step(baseline_raw: u16, run: u16, raw: u16) -> (u16, bool) {
    let mut g = SagGuard {
        baseline_raw,
        run,
        trip_raw: 0,
    };
    let trip = g.sample(raw);
    (g.run, trip)
}

/// 2^shift PWM cycles per overcurrent-average window (85 ms at
/// 24 kHz, 43 ms at 48 kHz).
pub const TRIP_WINDOW_SHIFT: u32 = 11;

/// One accumulator step for the windowed current average: returns
/// `(new_acc, new_cnt, Some(avg_raw))` when a window just completed
/// (acc/cnt reset to zero in the returned pair).
pub fn trip_accum_step(acc: u32, cnt: u32, sample_raw: u16) -> (u32, u32, Option<u32>) {
    let acc = acc + sample_raw as u32;
    let cnt = cnt + 1;
    if cnt >= (1 << TRIP_WINDOW_SHIFT) {
        (0, 0, Some(acc >> TRIP_WINDOW_SHIFT))
    } else {
        (acc, cnt, None)
    }
}

/// Overcurrent decision on the windowed average of ON-window shunt
/// samples. SEMANTICS MATTER: those samples are PHASE current — the
/// battery sees phase × duty; a phase-referred trip over-reads
/// supply draw by 1/duty (killed a healthy amp-54 run whose true
/// battery draw was ~1.6 A).
///
/// Open loop: 2.0 A phase-referred — the stall-heater guard (a
/// stalled open-loop drive has no other watchdog). Under CL: a
/// gross-fault backstop only (~4 A phase) — tighter throttle-scaled
/// envelopes (2× then 3.8× the healthy curve) kept declaring walls
/// in passable terrain that AM32 rides through; stall detection
/// under CL belongs to ZC-starvation, supply collapse to the sag
/// kill, a wedged core to the IWDG.
pub fn overcurrent(avg_phase_raw: u32, cl_active: bool) -> bool {
    if cl_active {
        avg_phase_raw > 150 // ≈ 4 A gross-fault backstop
    } else {
        avg_phase_raw > 75 // ≈ 2.0 A stall-heater guard
    }
}

/// ZOMBIE-FIELD DETECTOR (2026-07-16, post-cook). The physical
/// invariant on this bench: a REAL rotor at electrical speed above
/// ~1.3 kHz (interval < 125 us) draws well over 1 A; a free-running
/// field over a STALLED rotor at low duty draws a few hundred mA.
/// The cook: a PWM-subharmonic harmonic lock reported cl:ACTIVE at
/// "2 kHz" while the motor stood still cooking at 0.6 A — every
/// other guard had a hole at exactly that point (starvation: junk
/// accepts kept flowing; runaway floor: 83 us > 45; OC trip: 0.6 A
/// < 1.5 A). This detector closes the class: fast field + implausibly
/// low current sustained for 500 ms cannot be a spinning rotor.
pub const ZOMBIE_IV_US: u32 = 125; // "fast field": > ~1333 Hz electrical
pub const ZOMBIE_I_RAW: u16 = 15; // ~0.4 A: impossible up there (real: 60+ raw)
pub const ZOMBIE_RUN: u16 = 12_000; // 500 ms of consecutive samples at 24 kHz

/// One PWM-cycle step. `run` accumulates only while BOTH conditions
/// hold; any plausible sample resets it. Trip => kill.
#[inline]
pub fn zombie_step(run: u16, cl_active: bool, interval_us: u32, i_raw: u16) -> (u16, bool) {
    if cl_active && interval_us > 0 && interval_us < ZOMBIE_IV_US && i_raw < ZOMBIE_I_RAW {
        let run = run.saturating_add(1);
        (run, run >= ZOMBIE_RUN)
    } else {
        (0, false)
    }
}

/// FAST BURST RESPONDER (2026-07-15) — clamp, don't kill; but NEVER
/// leniently. Every envelope death from amp ~72 up is the same event:
/// a 12 A+ current burst compounds over 10-18 ms, the bus collapses,
/// and the sag guard kills. The protection stack has a burst-shaped
/// hole: the OC trip averages 85 ms (blind to bursts), the sag guard
/// reacts only AFTER the bus has collapsed. This responder watches
/// the per-PWM-cycle mid-ON current and cuts the commanded amplitude
/// to 2/3 while a burst is live, restoring on decay — converting
/// fatal compounding into the ride-through the loop already performs
/// for most bursts.
///
/// SAFETY INVARIANTS (the operator's fry-the-bench veto):
/// - The responder can only ever REDUCE duty. It touches no kill:
///   sag, OC, runaway, starvation all fire exactly as before.
/// - A clamp that fails to bring the current down is NOT protection:
///   if the clamp stays engaged for [`BURST_MAX_HOLD`] consecutive
///   cycles (40 ms — longer than any observed burst) the step
///   ESCALATES TO A KILL. A clamp can never keep a shorted/stalled
///   state cooking indefinitely.
/// - Trip needs [`BURST_ON_RUN`] consecutive over-threshold cycles
///   (333 us) so the benign 2-4 A single-window spikes (sub-ms,
///   ridden through for weeks) never engage it.
pub const BURST_TRIP_RAW: u16 = 185; // ~5 A: above benign spikes (2-4.5 A), far below burst peaks (12 A+)
pub const BURST_RELEASE_RAW: u16 = 110; // ~3 A release threshold (hysteresis)
pub const BURST_ON_RUN: u16 = 8; // 333 us of sustained overcurrent to engage
pub const BURST_OFF_RUN: u16 = 48; // 2 ms below release to disengage
pub const BURST_MAX_HOLD: u16 = 960; // 40 ms clamped without recovery -> KILL

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurstStep {
    pub run: u16,
    pub cool: u16,
    pub hold: u16,
    pub active: bool,
    /// Escalation: the clamp held BURST_MAX_HOLD without the current
    /// recovering. The caller must kill (hardware off + flags).
    pub kill: bool,
}

/// One 24 kHz step of the burst responder. Pure; firmware keeps
/// run/cool/hold/active in atomics (sole writer TIM1_UP).
#[inline]
pub fn burst_step(run: u16, cool: u16, hold: u16, active: bool, i_raw: u16) -> BurstStep {
    if !active {
        let run = if i_raw >= BURST_TRIP_RAW {
            run.saturating_add(1)
        } else {
            0
        };
        if run >= BURST_ON_RUN {
            BurstStep {
                run: 0,
                cool: 0,
                hold: 0,
                active: true,
                kill: false,
            }
        } else {
            BurstStep {
                run,
                cool: 0,
                hold: 0,
                active: false,
                kill: false,
            }
        }
    } else {
        let hold = hold.saturating_add(1);
        if hold >= BURST_MAX_HOLD {
            // Anti-burnout escalation: clamping did not bring the
            // current down within 40 ms. This is not a burst; kill.
            return BurstStep {
                run: 0,
                cool: 0,
                hold,
                active: true,
                kill: true,
            };
        }
        if i_raw < BURST_RELEASE_RAW {
            let cool = cool.saturating_add(1);
            if cool >= BURST_OFF_RUN {
                BurstStep {
                    run: 0,
                    cool: 0,
                    hold: 0,
                    active: false,
                    kill: false,
                }
            } else {
                BurstStep {
                    run: 0,
                    cool,
                    hold,
                    active: true,
                    kill: false,
                }
            }
        } else {
            BurstStep {
                run: 0,
                cool: 0,
                hold,
                active: true,
                kill: false,
            }
        }
    }
}

/// Amplitude through the burst clamp: 2/3 while a burst is live.
/// Composes with (after) `blind_amp_clamp`; only ever reduces.
#[inline]
pub fn burst_amp_clamp(base_amp: u16, burst_active: bool) -> u16 {
    if burst_active {
        (base_amp * 2 / 3).max(1)
    } else {
        base_amp
    }
}

/// FIX #2 — blind-free-run current clamp. During a ZC-miss cascade
/// (the monster mechanism: `monster1..3` autopsy) the loop commutates
/// BLIND at the frozen interval while the field drifts off the still-
/// turning rotor, ramping line-line current 1 A → 4 A+ over ~15
/// windows until it sags the bus and trips the kill (= the chop).
///
/// `noz_run` = consecutive A/B-window ZC misses since the last accept
/// (`cl_noz_run`, reset on every accept). Isolated misses (run 1–2)
/// are RIDDEN THROUGH at full drive — that's normal, 0.2–2.2 % of
/// windows. A sustained cascade (run ≥3) means we're flying blind, so
/// progressively cut the commanded amplitude: don't pump full current
/// into a field whose rotor position we've lost. Caps the monster
/// before it can sag-kill; re-acquisition (fix #1) then re-locks at
/// the reduced level, and normal drive resumes on the next accept.
#[inline]
pub fn blind_amp_clamp(base_amp: u16, noz_run: u8) -> u16 {
    if noz_run < 3 {
        base_amp
    } else if noz_run < 6 {
        (base_amp * 2 / 3).max(1)
    } else {
        (base_amp / 3).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- since_us: the watchdog underflow race ----

    #[test]
    fn regression_zombie_field_2026_07_16() {
        // The cook scenario: "2 kHz" harmonic lock (83 us) over a
        // stalled rotor at 0.6 A (~22 raw mid-ON... the STALL read
        // ~5-10 raw at the sample point). Must trip at exactly 500 ms.
        let mut run = 0u16;
        let mut tripped_at = None;
        for n in 0..13_000u32 {
            let (r, trip) = zombie_step(run, true, 83, 8);
            run = r;
            if trip {
                tripped_at = Some(n);
                break;
            }
        }
        assert_eq!(tripped_at, Some(ZOMBIE_RUN as u32 - 1));
        // A REAL 90 us lock draws 60-90 raw: never trips.
        let mut run = 0u16;
        for _ in 0..100_000 {
            let (r, trip) = zombie_step(run, true, 90, 70);
            run = r;
            assert!(!trip);
        }
        // Low speed at low current is normal (idle-ish): never trips.
        let mut run = 0u16;
        for _ in 0..100_000 {
            let (r, trip) = zombie_step(run, true, 600, 5);
            run = r;
            assert!(!trip);
        }
        // One plausible sample resets the accumulation.
        let (r, _) = zombie_step(ZOMBIE_RUN - 1, true, 83, 70);
        assert_eq!(r, 0);
    }

    #[test]
    fn burst_never_trips_on_benign_single_window_spikes() {
        // Benign 2-4.5 A spikes last a single window (~2 PWM cycles
        // at speed) — far under the 8-cycle engage run.
        let mut s = BurstStep {
            run: 0,
            cool: 0,
            hold: 0,
            active: false,
            kill: false,
        };
        for _ in 0..100 {
            for _ in 0..3 {
                s = burst_step(s.run, s.cool, s.hold, s.active, 300); // 8 A spike, 3 cycles
                assert!(!s.active && !s.kill);
            }
            for _ in 0..20 {
                s = burst_step(s.run, s.cool, s.hold, s.active, 60); // normal
                assert!(!s.active && !s.kill);
            }
        }
    }

    #[test]
    fn burst_trips_clamps_and_releases_on_decay() {
        let mut s = BurstStep {
            run: 0,
            cool: 0,
            hold: 0,
            active: false,
            kill: false,
        };
        // Sustained 6 A: engages at exactly the 8th cycle.
        for i in 0..8 {
            assert!(!s.active, "active early at cycle {i}");
            s = burst_step(s.run, s.cool, s.hold, s.active, 220);
        }
        assert!(s.active && !s.kill);
        assert_eq!(burst_amp_clamp(78, true), 52); // 2/3 cut
        assert_eq!(burst_amp_clamp(78, false), 78);
        // Burst rides down over 5 ms (120 cycles) then current drops.
        for _ in 0..120 {
            s = burst_step(s.run, s.cool, s.hold, s.active, 200);
            assert!(s.active && !s.kill);
        }
        // 2 ms below release -> disengage, no kill, full drive back.
        for _ in 0..(BURST_OFF_RUN - 1) {
            s = burst_step(s.run, s.cool, s.hold, s.active, 50);
            assert!(s.active);
        }
        s = burst_step(s.run, s.cool, s.hold, s.active, 50);
        assert!(!s.active && !s.kill);
    }

    #[test]
    fn burst_release_hysteresis_needs_consecutive_cool() {
        let mut s = BurstStep {
            run: 0,
            cool: 0,
            hold: 0,
            active: false,
            kill: false,
        };
        for _ in 0..8 {
            s = burst_step(s.run, s.cool, s.hold, s.active, 220);
        }
        assert!(s.active);
        // Alternating low/high never accumulates the cool run.
        for _ in 0..200 {
            s = burst_step(s.run, s.cool, s.hold, s.active, 50);
            s = burst_step(s.run, s.cool, s.hold, s.active, 150); // above release
            assert!(s.active, "released on non-consecutive cool");
        }
    }

    #[test]
    fn burst_anti_burnout_kills_at_max_hold() {
        // THE FRY-THE-BENCH VETO: a clamp that cannot bring the
        // current down escalates to a kill at 40 ms — it can never
        // keep a shorted/stalled state cooking.
        let mut s = BurstStep {
            run: 0,
            cool: 0,
            hold: 0,
            active: false,
            kill: false,
        };
        for _ in 0..8 {
            s = burst_step(s.run, s.cool, s.hold, s.active, 220);
        }
        assert!(s.active);
        let mut killed_at = None;
        for n in 0..BURST_MAX_HOLD + 10 {
            s = burst_step(s.run, s.cool, s.hold, s.active, 220); // stays hot
            if s.kill {
                killed_at = Some(n);
                break;
            }
        }
        let n = killed_at.expect("must escalate to kill");
        assert!(
            n as u32 <= BURST_MAX_HOLD as u32,
            "kill within 40 ms, got {n}"
        );
    }

    #[test]
    fn burst_clamp_only_ever_reduces() {
        for amp in 0..=96u16 {
            assert!(burst_amp_clamp(amp, true) <= amp.max(1));
            assert_eq!(burst_amp_clamp(amp, false), amp);
        }
    }

    #[test]
    fn blind_amp_clamp_rides_isolated_misses_caps_cascade() {
        // Isolated misses (normal, ridden through): full drive.
        assert_eq!(blind_amp_clamp(30, 0), 30);
        assert_eq!(blind_amp_clamp(30, 2), 30);
        // Sustained cascade: progressive cut.
        assert_eq!(blind_amp_clamp(30, 3), 20); // 2/3
        assert_eq!(blind_amp_clamp(30, 5), 20);
        assert_eq!(blind_amp_clamp(30, 6), 10); // 1/3
        assert_eq!(blind_amp_clamp(30, 20), 10);
        // Never zero (keep the field alive for re-acq).
        assert_eq!(blind_amp_clamp(1, 20), 1);
    }

    #[test]
    fn regression_watchdog_underflow_race_v3x() {
        // A commutation ISR stored a NEWER reference between the
        // caller's two reads: last > now. The old code produced
        // ~4×10⁹ µs of "silence" and killed a healthy 309 Hz lock
        // 20 µs after a refined commutation.
        let now = 1_000_000u32;
        let last_newer = 1_000_002u32;
        assert_eq!(since_us(now, last_newer), 0);
    }

    #[test]
    fn since_normal_and_wrap() {
        assert_eq!(since_us(1000, 900), 1000);
        // Across u32 wrap of the tick counter.
        assert_eq!(since_us(5, u32::MAX - 4), 100);
    }

    // ---- CL watchdog ----

    #[test]
    fn starvation_fires_at_12_intervals() {
        let iv = 650;
        assert_eq!(cl_watchdog(iv, iv * 12, 0, true), None); // == is not >
        assert_eq!(cl_watchdog(iv, iv * 12 + 1, 0, true), Some(Kill::ZcStarved));
    }

    #[test]
    fn starvation_needs_a_reference() {
        // Before the first accept there is nothing to starve from.
        assert_eq!(cl_watchdog(650, u32::MAX / 4, 0, false), None);
    }

    #[test]
    fn r3_watchdog_reseeds_where_it_killed() {
        use WatchdogAction as W;
        // Starvation at speed (6 ms silence at 90 us): RESEED now.
        assert_eq!(
            cl_watchdog_r3(90, 6_100, 0, true, 0),
            W::Reseed(Kill::ZcStarved)
        );
        // Desync (commutation silence): RESEED.
        assert_eq!(
            cl_watchdog_r3(600, 0, 3_100 * 3, true, 1),
            W::Reseed(Kill::Desync)
        );
        // The 22.5 ms backstop is a real KILL regardless of strikes.
        assert_eq!(
            cl_watchdog_r3(90, 23_000, 0, true, 0),
            W::Kill(Kill::ZcStarved)
        );
        // Runaway floor still kills (canary class).
        assert_eq!(cl_watchdog_r3(40, 0, 0, true, 0), W::Kill(Kill::Runaway));
        // Strike escalation: 4th reseed becomes a kill.
        assert_eq!(
            cl_watchdog_r3(90, 6_100, 0, true, RESEED_MAX_STRIKES),
            W::Kill(Kill::ZcStarved)
        );
        // Healthy: nothing.
        assert_eq!(cl_watchdog_r3(90, 100, 100, true, 0), W::None);
    }

    #[test]
    fn regression_runaway_floor_regime_2026_07_10() {
        // 160 µs floor executed a HEALTHY 1,050 Hz lock (interval
        // 158): bb showed a clean ACC/REF chain, then DSY d=3. The
        // floor is 45 µs now (E5: 60 collided with the loop's own
        // legal 0.6× accept step from any interval < 100 µs — one
        // permitted fast accept at 84 µs lands at 50 and was killed).
        // 158 must survive, a legal fast step from the operating band
        // must survive, true junk (sub-45 scheduler-floor runaway)
        // must die.
        assert_eq!(cl_watchdog(158, 0, 0, true), None);
        assert_eq!(cl_watchdog(50, 0, 0, true), None); // 0.6×84 legal step
        assert_eq!(cl_watchdog(44, 0, 0, true), Some(Kill::Runaway));
        assert_eq!(cl_watchdog(30, 0, 0, true), Some(Kill::Runaway));
    }

    #[test]
    fn desync_fires_on_chain_silence() {
        assert_eq!(cl_watchdog(650, 0, 3001 * 3, true), Some(Kill::Desync));
        assert_eq!(cl_watchdog(650, 0, 2_500, true), None); // < 3×max(1000,650)
    }

    #[test]
    fn no_interval_no_opinion() {
        assert_eq!(cl_watchdog(0, u32::MAX / 4, u32::MAX / 4, true), None);
    }

    // ---- Sag guard ----

    #[test]
    fn sag_baseline_relative_threshold() {
        let mut g = SagGuard::new();
        g.arm(1080); // ~8.1 V
        // −10 %: 972 raw ≈ 7.3 V.
        assert_eq!(g.threshold_raw(), 972);
    }

    #[test]
    fn regression_single_glitch_no_trip_2026_07_11() {
        // The false trip at amp 33: ONE corrupted sample below
        // threshold must not kill.
        let mut g = SagGuard::new();
        g.arm(1080);
        assert!(!g.sample(700)); // glitch
        assert!(!g.sample(1075)); // recovered
        for _ in 0..1000 {
            assert!(!g.sample(1075));
        }
    }

    #[test]
    fn sustained_sag_trips_and_latches_trip_value() {
        let mut g = SagGuard::new();
        g.arm(1080);
        let mut fired = false;
        for i in 0..SAG_DEBOUNCE {
            fired = g.sample(750 - i); // deepening sag
            if fired {
                break;
            }
        }
        assert!(fired);
        // Report shows the SAMPLE THAT TRIPPED, not a later recovered
        // value (the "8130 < 90% of 8107" nonsense-message bug).
        assert!(g.trip_raw < g.threshold_raw());
    }

    #[test]
    fn abs_floor_governs_low_baselines() {
        let mut g = SagGuard::new();
        g.arm(820); // 6.15 V baseline: −10 % would be 738 < floor
        assert_eq!(g.threshold_raw(), VBAT_ABS_FLOOR_RAW);
    }

    #[test]
    fn debounce_resets_on_recovery() {
        let mut g = SagGuard::new();
        g.arm(1080);
        for _ in 0..SAG_DEBOUNCE - 1 {
            assert!(!g.sample(900));
        }
        assert!(!g.sample(1050)); // recovery resets the run
        for _ in 0..SAG_DEBOUNCE - 1 {
            assert!(!g.sample(900));
        }
    }

    // ---- ISR kill flag matrix ----

    struct FlagRig {
        motor_enabled: portable_atomic::AtomicBool,
        cl_active: portable_atomic::AtomicBool,
        cl_armed: portable_atomic::AtomicBool,
        cl_desync: portable_atomic::AtomicBool,
        cl_starved: portable_atomic::AtomicBool,
        oc_tripped: portable_atomic::AtomicBool,
        vbat_sagged: portable_atomic::AtomicBool,
        bb_frozen: portable_atomic::AtomicBool,
        last_kill: portable_atomic::AtomicU8,
    }

    impl FlagRig {
        fn driving_cl() -> Self {
            use portable_atomic::AtomicBool as B;
            Self {
                motor_enabled: B::new(true),
                cl_active: B::new(true),
                cl_armed: B::new(true), // worst case: stale arm too
                cl_desync: B::new(false),
                cl_starved: B::new(false),
                oc_tripped: B::new(false),
                vbat_sagged: B::new(false),
                bb_frozen: B::new(false),
                last_kill: portable_atomic::AtomicU8::new(0),
            }
        }

        fn flags(&self) -> KillFlags<'_> {
            KillFlags {
                motor_enabled: &self.motor_enabled,
                cl_active: &self.cl_active,
                cl_armed: &self.cl_armed,
                cl_desync: &self.cl_desync,
                cl_starved: &self.cl_starved,
                oc_tripped: &self.oc_tripped,
                vbat_sagged: &self.vbat_sagged,
                bb_frozen: &self.bb_frozen,
                last_kill: &self.last_kill,
            }
        }
    }

    #[test]
    fn regression_no_zombie_flags_after_any_kill_kind() {
        use portable_atomic::Ordering::Relaxed;
        // The OC zombie-status incident (CL_ACTIVE left set → 10 fake
        // ladder rungs) and the watchdog kill's missing CL_ARMED:
        // EVERY kill kind must leave motor + both CL flags false.
        for kind in [
            IsrKillKind::Desync,
            IsrKillKind::Starved,
            IsrKillKind::Overcurrent,
            IsrKillKind::Sag,
        ] {
            let r = FlagRig::driving_cl();
            apply_isr_kill(&r.flags(), kind);
            assert!(!r.motor_enabled.load(Relaxed), "{kind:?}");
            assert!(!r.cl_active.load(Relaxed), "{kind:?} zombie CL_ACTIVE");
            assert!(!r.cl_armed.load(Relaxed), "{kind:?} zombie CL_ARMED");
        }
    }

    #[test]
    fn kill_matrix_per_kind_reports() {
        use portable_atomic::Ordering::Relaxed;
        let r = FlagRig::driving_cl();
        apply_isr_kill(&r.flags(), IsrKillKind::Starved);
        assert!(r.cl_desync.load(Relaxed) && r.cl_starved.load(Relaxed));
        assert!(r.bb_frozen.load(Relaxed), "bb must freeze the lead-up");
        assert!(!r.oc_tripped.load(Relaxed) && !r.vbat_sagged.load(Relaxed));

        let r = FlagRig::driving_cl();
        apply_isr_kill(&r.flags(), IsrKillKind::Desync);
        assert!(r.cl_desync.load(Relaxed) && !r.cl_starved.load(Relaxed));
        assert!(r.bb_frozen.load(Relaxed));

        let r = FlagRig::driving_cl();
        apply_isr_kill(&r.flags(), IsrKillKind::Overcurrent);
        assert!(r.oc_tripped.load(Relaxed));
        assert!(!r.cl_desync.load(Relaxed) && !r.bb_frozen.load(Relaxed));

        let r = FlagRig::driving_cl();
        apply_isr_kill(&r.flags(), IsrKillKind::Sag);
        assert!(r.vbat_sagged.load(Relaxed));
        assert!(!r.cl_desync.load(Relaxed) && !r.bb_frozen.load(Relaxed));
    }

    #[test]
    fn r4a_sag_step_scaled_matches_legacy_at_64_and_stretches_at_128() {
        let base = 1000u16;
        let sagged = 850u16; // below 900 threshold
        // debounce 64: trips on the 64th consecutive sample, like legacy.
        let mut run = 0u16;
        let mut tripped_at = 0u32;
        for i in 1..=200u32 {
            let (r, t) = sag_step_scaled(base, run, sagged, 64);
            run = r;
            if t {
                tripped_at = i;
                break;
            }
        }
        assert_eq!(tripped_at, 64);
        // debounce 128: rides through 100 samples (a 2 ms dip at
        // 48 kHz), trips only at 128.
        let mut run = 0u16;
        for _ in 0..100 {
            let (r, t) = sag_step_scaled(base, run, sagged, 128);
            assert!(!t);
            run = r;
        }
        let mut tripped_at = 100u32;
        loop {
            let (r, t) = sag_step_scaled(base, run, sagged, 128);
            run = r;
            tripped_at += 1;
            if t {
                break;
            }
        }
        assert_eq!(tripped_at, 128);
        // recovery sample resets the run.
        let (r, _) = sag_step_scaled(base, 120, base, 128);
        assert_eq!(r, 0);
    }

    // ---- sag_step (atomic-backed adoption path) ----

    #[test]
    fn wait_cut_fires_past_one_interval_only() {
        // 1.125x threshold: the estimator's climb-lag band
        // (~1.02-1.05x) must NOT fire (whcut2 strangle incident).
        assert!(!wait_cut_due(105, 100));
        assert!(!wait_cut_due(112, 100));
        assert!(wait_cut_due(113, 100));
        // unseeded estimator: never (engage regime).
        assert!(!wait_cut_due(10_000, 0));
    }

    #[test]
    fn wait_clamp_halves_then_quarters_with_wait_length() {
        // 100 us interval: full to 250 us, half to 500, quarter after.
        assert_eq!(wait_amp_clamp(60, 200, 100), 60);
        assert_eq!(wait_amp_clamp(60, 300, 100), 30);
        assert_eq!(wait_amp_clamp(60, 501, 100), 15);
        // Unseeded estimator: no clamp (engage regime).
        assert_eq!(wait_amp_clamp(60, 10_000, 0), 60);
    }

    #[test]
    fn sag_step_matches_sagguard_semantics() {
        // Debounce accumulates, recovery resets, trip fires at the
        // threshold count — same suite as SagGuard, through the
        // load-run-store form the firmware actually calls.
        let base = 1080;
        let mut run = 0;
        for _ in 0..SAG_DEBOUNCE - 1 {
            let (r, trip) = sag_step(base, run, 900);
            assert!(!trip);
            run = r;
        }
        let (_, trip) = sag_step(base, run, 900);
        assert!(trip, "64th consecutive sub-threshold sample kills");
        // Recovery resets.
        let (r, _) = sag_step(base, 50, 1075);
        assert_eq!(r, 0);
    }

    // ---- trip accumulator ----

    #[test]
    fn trip_accum_windows_and_resets() {
        let mut acc = 0u32;
        let mut cnt = 0u32;
        let mut avg = None;
        for _ in 0..(1 << TRIP_WINDOW_SHIFT) {
            let (a, c, v) = trip_accum_step(acc, cnt, 100);
            acc = a;
            cnt = c;
            avg = v;
        }
        assert_eq!(avg, Some(100), "flat 100-count input averages to 100");
        assert_eq!((acc, cnt), (0, 0), "window resets");
        // Mid-window: no verdict.
        assert_eq!(trip_accum_step(0, 0, 4095).2, None);
    }

    // ---- Overcurrent ----

    #[test]
    fn oc_open_loop_stall_guard() {
        assert!(!overcurrent(75, false));
        assert!(overcurrent(76, false));
    }

    #[test]
    fn regression_cl_backstop_rides_the_knee() {
        // The ~2.7 A transitional draw at the amp-51 knee (AM32
        // rides it) must NOT trip; a 4 A+ gross fault must.
        let raw_2p7a = 101;
        let raw_4p2a = 157;
        assert!(!overcurrent(raw_2p7a, true));
        assert!(overcurrent(raw_4p2a, true));
    }
}
