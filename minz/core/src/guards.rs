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
/// Consecutive sub-threshold samples required (1.3 ms at the 48 kHz
/// pump).
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

    // ---- sag_step (atomic-backed adoption path) ----

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
