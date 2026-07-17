//! The OWL interval estimator — the single most incident-rich piece
//! of logic in the project. Every branch below was shaped by a
//! measured failure; the tests replay them.

/// Bounds and constants (µs).
///
/// INT_MIN is an ABSURDITY bound, not a speed limit — but at 100 µs it
/// silently WAS one: every measured interval ≤ 100 µs (f_e ≥ 1667 Hz)
/// was rejected as insane, so above that speed the estimator PINNED
/// just over 100 while the true window ran 93–94 µs (direct evidence:
/// bb `REF d=102–106` vs wire window lengths 93–94 at amp 66). The
/// free-run then scheduled 1.0×(inflated interval) → systematically
/// late commutations fighting the ZC-refine every window → the ±10 µs
/// commutation-timing oscillation → BEMF-misalignment current spikes
/// with a soft onset exactly where the rotor crossed 1667 Hz — the
/// entire amp-66 spike population and the 68 kills (2026-07-14 deep
/// dive). 40 µs = f_e ≤ 4.2 kHz, comfortably past the 3 kHz roadmap;
/// half-period junk is rejected by the symmetric ±25 % rate bound, not
/// by this constant.
pub const INT_MIN: u32 = 40;
/// SEEDING bound: when the estimator is UNSEEDED (old == 0) the ±25 %
/// rate bound has no reference, so any delta in (INT_MIN, INT_MAX)
/// would seed it — at INT_MIN=40 that let engage-time comparator noise
/// (41–99 µs spacings) seed absurdly low and killed the engage (bench,
/// 2026-07-14: 2× engage failures immediately after the 40 change).
/// Seeding keeps the old 100 µs bar (engage happens at ≥ ~417 µs);
/// TRACKING below 100 µs is what INT_MIN=40 enables, and there the
/// ±25 % bound provides the junk rejection.
pub const SEED_MIN: u32 = 100;
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
    /// AM32-GEOMETRY mode (2026-07-16 port): under an established
    /// lock, use AM32's update rule verbatim — measurement = the
    /// average of the last TWO periods (sector alternation cancels
    /// before filtering), blend α = 0.5, NO acceptance bounds
    /// (nothing is ever refused, so the estimate can never starve;
    /// the white-box census measured 0.00 spike events/s to 2293 Hz
    /// with this rule on identical hardware). Engage/open-loop keep
    /// the bounded path — the census showed those rejections are
    /// protective there (~15k correct refusals per spin-up).
    pub am32_geom: bool,
    /// Previous ZC-to-ZC period (µs) for the 2-period pre-average.
    pub prev_period_us: u32,
}

impl Estimator {
    pub const fn new() -> Self {
        Self {
            interval_us: 0,
            last_qzc_us: None,
            windows_since_qzc: 0,
            reacq: false,
            am32_geom: false,
            prev_period_us: 0,
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
        self.on_accept_traced(zc_us, cl_active).0
    }

    /// REJECTION CENSUS (2026-07-16): every silent estimator no-op
    /// becomes a reported outcome. The INT_MIN pinning survived for
    /// days because a rejected sample still publishes the window's
    /// qZC — qzc reads 100 % while the estimate refuses to move; the
    /// coverage metric is structurally blind to this failure class.
    /// The firmware counts these per kind and prints them in the
    /// i-line, giving the AM32-geometry A/B its second discriminator
    /// (AM32's rule set rejects nothing, by construction).
    pub fn on_accept_traced(&mut self, zc_us: u32, cl_active: bool) -> (u32, Option<Reject>) {
        let mut reject = None;
        let spans = self.windows_since_qzc;
        self.windows_since_qzc = 0;
        if let Some(last) = self.last_qzc_us {
            if (1..=SPAN_MAX).contains(&spans) {
                let new_int = zc_us.wrapping_sub(last) / spans;
                let old = self.interval_us;
                if self.am32_geom && cl_active && old != 0 {
                    // THE HARMONIC GUARD (2026-07-16, post-cook): the
                    // fully-boundless first cut let a stalled motor's
                    // PWM-subharmonic noise (edges ~2x the carrier
                    // period) ratchet the estimate 305 -> 83 us in
                    // legal-looking steps - the iv/2 gate shrank WITH
                    // the corrupted estimate and cl:ACTIVE lied over a
                    // 0.6 A heater. One-sided, wide: a HALVING of the
                    // interval in one accept is physically impossible
                    // for this rotor; reject and COUNT it. The grow
                    // side stays unbounded (deceleration starvation -
                    // the reseed-refusal death class - lived there).
                    if new_int < old / 2 {
                        reject = Some(Reject::Harmonic);
                    } else if new_int < old * 2 / 3 || new_int > old * 3 / 2 {
                        // WIDE SYMMETRIC SANITY BOUND (2026-07-17,
                        // the recovered port): the pre-averaged
                        // measurement is clean enough that OWL's
                        // tight ±25 % is unnecessary — but the bench
                        // proved fully-boundless blending loses every
                        // 66->70 transit to junk-edge blends (0/5,
                        // sag kills; the +50 us phase-walk accept
                        // was a 190-vs-345 sample this bound
                        // rejects). [2/3, 3/2] tracks any physical
                        // transit (5 %/window needed) with 10x
                        // margin, and rejects the premature class.
                        reject = Some(Reject::RateBound);
                    } else {
                        // AM32 rule: interval = (interval + (prev+this)/2)/2.
                        // Any accept also ends re-acquisition (with
                        // nothing refused on this side, one accept
                        // re-seeds).
                        let prev = if self.prev_period_us != 0 {
                            self.prev_period_us
                        } else {
                            new_int
                        };
                        self.interval_us = (old + (prev + new_int) / 2) / 2;
                        self.prev_period_us = new_int;
                        self.reacq = false;
                    }
                } else if self.reacq && cl_active {
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
                    } else {
                        reject = Some(Reject::Reseed);
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
                    let sane = new_int > if old == 0 { SEED_MIN } else { INT_MIN }
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
                    } else {
                        // Which bound refused it?
                        reject = Some(if new_int <= (if old == 0 { SEED_MIN } else { INT_MIN }) {
                            Reject::Floor
                        } else if new_int >= INT_MAX {
                            Reject::Ceiling
                        } else {
                            Reject::RateBound
                        });
                    }
                }
            }
        }
        self.last_qzc_us = Some(zc_us);
        (self.interval_us, reject)
    }
}

/// Why the estimator refused to move on an accepted qZC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reject {
    /// Below INT_MIN (or SEED_MIN when unseeded) — the INT_MIN-
    /// pinning class (the silent 1667 Hz ceiling).
    Floor,
    /// Above INT_MAX.
    Ceiling,
    /// Outside the ±25 %/window rate bound.
    RateBound,
    /// Re-acq re-seed refused (spans != 1 or outside [0.5, 2]×old).
    Reseed,
    /// Geometry-mode harmonic guard: a one-accept interval halving is
    /// physically impossible — the PWM-subharmonic stall attractor.
    Harmonic,
}

impl Default for Estimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn am32_geometry_rule_verbatim() {
        // interval = (old + (prev+this)/2)/2, no bounds, under lock.
        let mut e = Estimator::new();
        e.am32_geom = true;
        e.interval_us = 100;
        e.prev_period_us = 90;
        e.last_qzc_us = Some(1000);
        e.windows_since_qzc = 1;
        // this period = 110: (100 + (90+110)/2)/2 = (100+100)/2 = 100
        let (iv, rej) = e.on_accept_traced(1110, true);
        assert_eq!(iv, 100);
        assert_eq!(rej, None);
        assert_eq!(e.prev_period_us, 110);
        // The wide sanity bound rejects wild samples (+300 %) —
        // the fully-boundless variant lost every 66->70 transit.
        e.last_qzc_us = Some(2000);
        e.windows_since_qzc = 1;
        let (iv2, rej2) = e.on_accept_traced(2400, true); // 400 us = +300 %
        assert_eq!(rej2, Some(Reject::RateBound));
        assert_eq!(iv2, 100);
        // A +40 % sample (transit-fast, physically possible) passes —
        // this is the one OWL's ±25 % refused.
        e.last_qzc_us = Some(3000);
        e.windows_since_qzc = 1;
        let (_, rej2b) = e.on_accept_traced(3140, true);
        assert_eq!(rej2b, None);
        // Fix the running state for the reacq check below.
        e.prev_period_us = 140;
        // Reacq ends on any accept in this mode (nothing can starve).
        e.reacq = true;
        e.last_qzc_us = Some(3000);
        e.windows_since_qzc = 1;
        let (_, rej3) = e.on_accept_traced(3150, true);
        assert_eq!(rej3, None);
        assert!(!e.reacq);
    }

    #[test]
    fn am32_geometry_cancels_sector_alternation() {
        // Alternating 80/100 us periods (rising/falling asymmetry):
        // the 2-period pre-average feeds a constant 90 to the blend,
        // so the estimate converges to 90 and STAYS - no ripple.
        let mut e = Estimator::new();
        e.am32_geom = true;
        e.interval_us = 90;
        e.prev_period_us = 100;
        e.last_qzc_us = Some(0);
        let mut zc = 0u32;
        for i in 0..100 {
            let period = if i % 2 == 0 { 80 } else { 100 };
            zc += period;
            e.windows_since_qzc = 1;
            e.on_accept_traced(zc, true);
            assert_eq!(e.interval_us, 90, "ripple at step {i}");
        }
    }

    #[test]
    fn regression_pwm_subharmonic_ratchet_2026_07_16() {
        // THE COOK: stalled rotor, comparator chewing PWM noise. The
        // boundless rule walked 305 us down to the 83 us subharmonic
        // attractor and free-ran a 2 kHz field into a stationary
        // rotor at 0.6 A. The harmonic guard must refuse any
        // one-accept halving; the estimate must never reach the
        // attractor from a stalled 305 us state.
        let mut e = Estimator::new();
        e.am32_geom = true;
        e.interval_us = 305;
        e.prev_period_us = 305;
        e.last_qzc_us = Some(0);
        let mut zc = 0u32;
        // Noise edges at the 83 us attractor spacing, relentlessly.
        for _ in 0..1000 {
            zc += 83;
            e.windows_since_qzc = 1;
            let (_, rej) = e.on_accept_traced(zc, true);
            assert_eq!(rej, Some(Reject::Harmonic));
            assert_eq!(e.interval_us, 305, "estimate walked toward the attractor");
        }
        // A legitimate fast accel step (-30 %) still passes.
        e.last_qzc_us = Some(zc);
        e.windows_since_qzc = 1;
        let (_, rej) = e.on_accept_traced(zc + 214, true);
        assert_eq!(rej, None);
    }

    #[test]
    fn am32_geometry_off_during_engage() {
        // Not cl_active or unseeded: the bounded stock path runs.
        let mut e = Estimator::new();
        e.am32_geom = true;
        e.interval_us = 100;
        e.last_qzc_us = Some(1000);
        e.windows_since_qzc = 1;
        // cl_active=false -> bounded path -> +40 % is REJECTED.
        assert_eq!(e.on_accept_traced(1140, false).1, Some(Reject::RateBound));
    }

    #[test]
    fn rejection_census_kinds_2026_07_16() {
        // Every silent no-op is now a classified outcome.
        let mut e = Estimator::new();
        e.interval_us = 100;
        e.last_qzc_us = Some(1000);
        e.windows_since_qzc = 1;
        // Rate bound: 100 -> 140 is +40 % > +25 %.
        assert_eq!(e.on_accept_traced(1140, true).1, Some(Reject::RateBound));
        // Floor: below INT_MIN.
        e.interval_us = 50;
        e.last_qzc_us = Some(2000);
        e.windows_since_qzc = 1;
        assert_eq!(e.on_accept_traced(2030, true).1, Some(Reject::Floor));
        // Accepted: within bounds.
        e.interval_us = 100;
        e.last_qzc_us = Some(3000);
        e.windows_since_qzc = 1;
        assert_eq!(e.on_accept_traced(3110, true).1, None);
        // Reseed refusal: reacq + spans=2.
        e.reacq = true;
        e.interval_us = 100;
        e.last_qzc_us = Some(4000);
        e.windows_since_qzc = 2;
        assert_eq!(e.on_accept_traced(4200, true).1, Some(Reject::Reseed));
    }

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
    fn regression_estimator_tracks_below_100us_2026_07_14() {
        // INT_MIN=100 silently rejected every interval ≤ 100 µs, so
        // above 1667 Hz the estimator pinned ~100+ while the rotor ran
        // 93-94 µs — late free-run scheduling, the ±10 µs timing
        // oscillation, and the whole amp-66 spike population. The
        // estimator must track a 94 µs train exactly.
        let mut e = Estimator::new();
        run_train(&mut e, 0, 600, 3, false); // seed at low speed
        // Walk down within the ±25 % bound: 600→460→350→270→210→160→125→96→94…
        let mut t = 3 * 600;
        let mut iv = e.interval_us;
        // Decelerate the training period by 0.97× per step. The map
        // ratio_next = 4q·r/(3+r) has fixed point r* = 4q−3, and staying
        // inside the ±25 % bound needs r* ≥ 0.8 ⇒ q ≥ 0.95: with the
        // ¾-smoothing the estimator can track at most ~5 % deceleration
        // per window (a real transit-behavior limit, not just a test
        // artifact). Then dwell at 94 µs to converge.
        let mut period = 600u32;
        for _ in 0..80 {
            period = (period * 97 / 100).max(94);
            e.on_window_close();
            t += period;
            iv = e.on_accept(t, true);
        }
        for _ in 0..20 {
            e.on_window_close();
            t += 94;
            iv = e.on_accept(t, true);
        }
        assert!(
            (90..=96).contains(&iv),
            "estimator failed to track 94 µs (got {iv}) — INT_MIN regression"
        );
    }

    #[test]
    fn regression_unseeded_cannot_seed_below_100us() {
        // With INT_MIN at 40, an UNSEEDED estimator (no ±25 % reference)
        // must still refuse sub-100 µs junk — engage-time comparator
        // noise seeded 41-99 µs and killed the engage otherwise.
        let mut e = Estimator::new();
        e.on_window_close();
        e.on_accept(10_000, false); // chain seed only
        e.on_window_close();
        let iv = e.on_accept(10_094, false); // 94 µs delta, unseeded
        assert_eq!(iv, 0, "unseeded estimator seeded from sub-100 junk");
        // But a plausible open-loop delta seeds fine.
        let mut e = Estimator::new();
        e.on_window_close();
        e.on_accept(10_000, false);
        e.on_window_close();
        assert_eq!(e.on_accept(10_000 + 1_667, false), 1_667);
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
