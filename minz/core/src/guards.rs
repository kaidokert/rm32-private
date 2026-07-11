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
    if interval_us < 60 {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ---- since_us: the watchdog underflow race ----

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
        // floor is 60 µs now; 158 must survive, true junk (24-50 µs
        // scheduler-floor runaway) must die.
        assert_eq!(cl_watchdog(158, 0, 0, true), None);
        assert_eq!(cl_watchdog(59, 0, 0, true), Some(Kill::Runaway));
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
