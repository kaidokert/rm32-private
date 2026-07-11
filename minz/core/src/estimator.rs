//! The OWL interval estimator — the single most incident-rich piece
//! of logic in the project. Every branch below was shaped by a
//! measured failure; the tests replay them.

/// Bounds and constants (µs).
pub const INT_MIN: u32 = 100;
pub const INT_MAX: u32 = 30_000;
/// Chain broken beyond this many window closes since the last qZC.
pub const SPAN_MAX: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimator {
    /// Smoothed sector interval, µs. 0 = unseeded.
    pub interval_us: u32,
    /// Timestamp (µs) of the last accepted qZC; `None` = chain broken.
    pub last_qzc_us: Option<u32>,
    /// Window closes since the last accepted qZC (span divider).
    pub windows_since_qzc: u32,
    /// Re-acquisition mode: tiny gate + strict confirms upstream,
    /// bounded direct re-seed here.
    pub reacq: bool,
}

impl Estimator {
    pub const fn new() -> Self {
        Self {
            interval_us: 0,
            last_qzc_us: None,
            windows_since_qzc: 0,
            reacq: false,
        }
    }

    /// Arm-time reset. A poisoned static interval (e.g. 144 µs left
    /// by a runaway) is otherwise UNRECOVERABLE — the runaway floor
    /// kills every engage while the rate bound rejects every honest
    /// open-loop sample (1667 ≫ 1.8×144): individually-correct
    /// guards deadlocked as a system. Engagement waits for a fresh
    /// estimate, so this re-seeds from open-loop qZCs in a few
    /// windows.
    pub fn reset_for_arm(&mut self) {
        self.interval_us = 0;
        self.last_qzc_us = None;
        self.windows_since_qzc = 0;
        self.reacq = false;
    }

    /// Every float-window close.
    pub fn on_window_close(&mut self) {
        self.windows_since_qzc = self.windows_since_qzc.saturating_add(1);
    }

    /// Enter re-acquisition (2 consecutive ZC-less A/B windows,
    /// decided upstream). Breaks the qZC chain: recovery must be
    /// measured from TWO fresh strict-confirmed ZCs — a stale
    /// pre-spiral `last` hands the re-seed an aliased delta (seen
    /// re-seeding 162 µs and tripping the runaway floor).
    pub fn enter_reacq(&mut self) {
        self.reacq = true;
        self.last_qzc_us = None;
    }

    /// An accepted qualified ZC at `zc_us`. Returns the (possibly
    /// updated) interval. `cl_active` gates the re-acq re-seed path.
    pub fn on_accept(&mut self, zc_us: u32, cl_active: bool) -> u32 {
        let spans = self.windows_since_qzc;
        self.windows_since_qzc = 0;
        if let Some(last) = self.last_qzc_us {
            if (1..=SPAN_MAX).contains(&spans) {
                let new_int = zc_us.wrapping_sub(last) / spans;
                let old = self.interval_us;
                if self.reacq && cl_active {
                    // Bounded direct re-seed from two FRESH strict
                    // ZCs: [0.5, 2.0]×old — wide enough to undo any
                    // walk the spiral caused, narrow enough to
                    // reject aliased junk.
                    if spans == 1
                        && new_int > INT_MIN
                        && new_int < INT_MAX
                        && new_int > old / 2
                        && new_int < old.saturating_mul(2)
                    {
                        self.interval_us = new_int;
                        self.reacq = false;
                    }
                } else {
                    // Normal path: ±25 %/window rate bound. Symmetric
                    // (the one-sided version let half-period junk
                    // walk the loop onto 2× rotor frequency —
                    // harmonic lock at "539 Hz" qzc 28 % vs true
                    // 257 Hz at 100 %); ±25 % rather than ±2 % (the
                    // tight "physical" clamp locked a 46 Hz crawl —
                    // under a synchronous loop the measurement
                    // echoes the loop's own field and tight clamps
                    // remove the convergence signal).
                    let sane = new_int > INT_MIN
                        && new_int < INT_MAX
                        && (old == 0
                            || (new_int < old.saturating_mul(5) / 4
                                && new_int > old.saturating_mul(4) / 5));
                    if sane {
                        self.interval_us = if old == 0 {
                            new_int
                        } else {
                            (3 * old + new_int) / 4
                        };
                    }
                }
            }
        }
        self.last_qzc_us = Some(zc_us);
        self.interval_us
    }
}

impl Default for Estimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed a steady qZC train (one per window) and return the
    /// converged interval.
    fn run_train(e: &mut Estimator, start_us: u32, period: u32, n: u32, cl: bool) -> u32 {
        let mut iv = e.interval_us;
        for k in 0..n {
            e.on_window_close();
            iv = e.on_accept(start_us + k * period, cl);
        }
        iv
    }

    #[test]
    fn seeds_from_first_chain_delta() {
        let mut e = Estimator::new();
        run_train(&mut e, 0, 650, 3, false);
        assert_eq!(e.interval_us, 650);
    }

    #[test]
    fn smooths_three_quarters() {
        let mut e = Estimator::new();
        run_train(&mut e, 0, 600, 5, false);
        // Last accept of the train sits at t = 4·600 = 2400; one
        // longer delta (700, inside the ±25 % band) converges
        // gradually via the ¾ smoothing.
        let iv1 = e.interval_us;
        e.on_window_close();
        let iv2 = e.on_accept(4 * 600 + 700, false);
        assert!(iv2 > iv1 && iv2 < 700, "iv1={iv1} iv2={iv2}");
    }

    #[test]
    fn regression_harmonic_lock_walkdown_blocked() {
        // Half-period junk (≈0.5×) must be rejected outright —
        // one-sided bounds let it walk the loop onto 2× rotor
        // frequency.
        let mut e = Estimator::new();
        run_train(&mut e, 0, 650, 4, true);
        e.on_window_close();
        let iv = e.on_accept(4 * 650 + 325, true); // 0.5× delta
        assert_eq!(iv, 650, "junk half-period accepted");
    }

    #[test]
    fn regression_aliased_growth_blocked() {
        // ~1.5× aliased accept (wrong-crossing edge, same parity
        // sector) walked the estimate 351 → 537 µs pre-fix; the
        // symmetric bound rejects anything ≥1.25×.
        let mut e = Estimator::new();
        run_train(&mut e, 0, 351, 4, true);
        e.on_window_close();
        let iv = e.on_accept(4 * 351 + 527, true);
        assert_eq!(iv, 351);
    }

    #[test]
    fn regression_poisoned_interval_unrecoverable_then_reset() {
        // 144 µs left by a runaway: honest 1,667 µs open-loop samples
        // are all rejected (1667 > 1.8×144) — the deadlock.
        let mut e = Estimator::new();
        run_train(&mut e, 0, 144, 3, false);
        assert_eq!(e.interval_us, 144);
        let iv = run_train(&mut e, 10_000, 1_667, 8, false);
        assert_eq!(iv, 144, "poison should be sticky without reset");
        // The fix: reset_for_arm clears it and re-seeding works.
        e.reset_for_arm();
        let iv = run_train(&mut e, 50_000, 1_667, 3, false);
        assert_eq!(iv, 1_667);
    }

    #[test]
    fn span_division_bridges_missed_windows() {
        let mut e = Estimator::new();
        run_train(&mut e, 0, 600, 3, false);
        // Two window closes with no accept (dead-reckoned C window +
        // one miss), then an accept spanning 2 periods.
        e.on_window_close();
        e.on_window_close();
        let iv = e.on_accept(3 * 600 + 1_200, false);
        assert_eq!(iv, 600);
    }

    #[test]
    fn chain_breaks_beyond_span_max() {
        let mut e = Estimator::new();
        run_train(&mut e, 0, 600, 3, false);
        for _ in 0..5 {
            e.on_window_close();
        }
        // Spans=5 > 3: no update, but the chain re-seeds `last`.
        let iv = e.on_accept(99_999, false);
        assert_eq!(iv, 600);
        assert_eq!(e.last_qzc_us, Some(99_999));
    }

    #[test]
    fn regression_reacq_reseed_bounded_2026_07_09() {
        // Unbounded re-seed accepted an aliased 162 µs and tripped
        // the runaway floor. Bounded: [0.5, 2]×old, spans==1, both
        // ZCs fresh (chain broken on entry).
        let mut e = Estimator::new();
        run_train(&mut e, 0, 527, 4, true); // walked-up estimate
        e.enter_reacq();
        assert_eq!(e.last_qzc_us, None, "chain must break on entry");
        // First fresh ZC only seeds the chain.
        e.on_window_close();
        e.on_accept(10_000, true);
        assert!(e.reacq);
        // Aliased second delta (162 µs < 527/2) must NOT re-seed.
        e.on_window_close();
        e.on_accept(10_162, true);
        assert!(e.reacq, "aliased re-seed accepted");
        assert_eq!(e.interval_us, 527);
        // Honest second delta re-seeds and exits re-acq.
        e.on_window_close();
        let iv = e.on_accept(10_162 + 351, true);
        assert_eq!(iv, 351);
        assert!(!e.reacq);
    }

    #[test]
    fn reacq_reseed_requires_cl() {
        // The re-seed path is CL-only; in open loop the normal
        // bounded path applies even in reacq state.
        let mut e = Estimator::new();
        run_train(&mut e, 0, 527, 4, false);
        e.enter_reacq();
        e.on_window_close();
        e.on_accept(20_000, false);
        e.on_window_close();
        e.on_accept(20_351, false);
        assert!(e.reacq, "open-loop accept must not clear reacq");
    }
}
