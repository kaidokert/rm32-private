//! The ZC candidate/confirm/accept state machine (roadmap E1) plus
//! its pure discrimination arithmetic.
//!
//! Three entry points mirror the three interrupt contexts, and the
//! locking model is the firmware's PRIORITY STRUCTURE, not critical
//! sections — none are opened here:
//! - [`on_held_edge`] — COMP ISR (prio 1), after the hardware
//!   persistence loop held. LPTIM2 (commutation) shares prio 1 so
//!   edge-accept and commutate never nest; TIM1_UP (prio 3) is
//!   preempted. No CS by design.
//! - [`confirm_step`] + [`accept_gen_current`] — TIM1_UP (prio 3),
//!   wrap-confirm. The caller keeps its `free()` envelope and gates
//!   the accept on `accept_gen_current` INSIDE it.
//! - [`accept_publish`] — shared accept: publish the window's qZC,
//!   run the estimator, decide engage/schedule. The scheduling tail
//!   (advance/delay arithmetic + LPTIM2 write) stays with the caller
//!   so the elapsed-time read keeps its original position.
//!
//! **TOCTOU fix (roadmap B1):** the original gen-guard could be
//! defeated — a window close PLUS a fresh COMP re-arm between
//! TIM1_UP's candidate load and its CS refreshed `CAND_GEN` to the
//! NEW generation, so the CAND_GEN-vs-WINDOW_GEN re-check passed and
//! a previous window's timestamp was accepted (junk estimator delta,
//! inflated scheduler elapsed). The fix: `confirm_step` snapshots
//! the generation AT CANDIDATE-LOAD TIME and `accept_gen_current`
//! validates that LOCAL snapshot inside the CS — a re-arm can
//! refresh `CAND_GEN` all it wants. Candidate consume/discard are
//! compare-exchanges so a replaced candidate always survives.

use portable_atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

/// The FALCON v3 ADC-sign confirm rule: is the floating phase below
/// the driven-pair neutral? Evaluated from the mid-ON ADC samples of
/// phases A (PA4/ch9) and B (PA5/ch10); `true` matches the COMP
/// VALUE=1 convention so the candidate's expected level applies
/// unchanged.
///
/// Per-sector arithmetic (2× the float value vs 2× the neutral
/// avoids halving):
/// - sector 1: B floats, A high / C low → neutral = A/2 → `2B < A`
/// - sector 4: B floats, C high / A low → neutral = (vbus+A)/2
/// - sector 2: A floats, B high / C low → `2A < B`
/// - sector 5: A floats, C high / B low → neutral = (vbus+B)/2
/// - sectors 0/3: phase C floats (no ADC route on PB7) → the
///   wrap-sampled comp bit passes through (observation only; C
///   windows are dead-reckoned under closed loop).
///
/// The discriminator-probe verdict that made this the confirm rule:
/// 100 % accept / 0 % premature / 46±29 µs latency in closed-loop
/// conditions, vs the wrap-sampled COMP bit at 25-73 % premature
/// (the self-referential-lock mechanism). Sector 2's arithmetic is
/// the "48 kHz sector-2 mystery": at the ZC, 2×A ≈ vbus by
/// construction, so the sample timing constraints (47.5-cycle
/// sampling, ≥1.25 µs trigger) are hard requirements — enforced in
/// hardware setup, not here.
#[inline]
pub fn adc_sign_observed(
    sector: u8,
    pa_a: u16,
    pa_b: u16,
    vbus_est: u16,
    comp_value: bool,
) -> bool {
    let (a, b, v) = (pa_a as u32, pa_b as u32, vbus_est as u32);
    match sector {
        1 => b * 2 < a,
        4 => b * 2 < v + a,
        2 => a * 2 < b,
        5 => a * 2 < v + b,
        _ => comp_value,
    }
}

/// Decaying-max vbus estimate (ADC counts through the phase
/// divider): the driven-high phase reads ≈ vbus in 4 of 6 sectors,
/// so the max refreshes constantly while spinning; the `est >> 9`
/// decay (min 1 count/PWM cycle, ~tens of ms to fall substantially)
/// is fast enough to track supply sag, slow enough to ride through
/// the two sectors with no full-rail read. Feeds
/// [`adc_sign_observed`]'s sector-4/5 neutrals.
#[inline]
pub fn vbus_decay_step(est: u16, pa_a: u16, pa_b: u16) -> u16 {
    let m = pa_a.max(pa_b);
    if m > est {
        m
    } else if est > 0 {
        est.saturating_sub((est >> 9).max(1))
    } else {
        0
    }
}

/// Expected post-ZC comparator level under textbook polarity
/// (POLARITY=0, INP=neutral, INM=floating phase): even sectors ride
/// a falling-BEMF window → post-ZC the phase is BELOW neutral →
/// VALUE=1; odd sectors the inverse.
#[inline]
pub const fn expected_post_zc(sector: u8) -> bool {
    (sector & 1) == 0
}

/// The state machine's view of the firmware statics.
pub struct ZcState<'a> {
    /// SCHEDULING INVERSION (rotor-clocked commutation): when set,
    /// phase-C windows (sectors 0/3) MUST arm the shot at every
    /// speed under SWIFT - without a free-run there is no
    /// dead-reckon fallback, and a non-arming window stalls the
    /// whole chain (invert2 incident: 3 ms silence at every C
    /// window -> reseed storm -> strike kill). The TOPEND gate on C
    /// re-timing was a FREE-RUN-era robustness patch and only
    /// applies when a free-run exists to dead-reckon C.
    pub zc_clocked: &'a AtomicBool,
    pub cand_zc_us: &'a AtomicU32,
    pub cand_expected: &'a AtomicBool,
    pub cand_confirms: &'a AtomicU8,
    pub cand_gen: &'a AtomicU8,
    pub window_gen: &'a AtomicU8,
    pub window_qzc_us: &'a AtomicU32,
    // Estimator statics (load-run-store through estimator::Estimator).
    pub interval_us: &'a AtomicU32,
    pub last_qzc_us: &'a AtomicU32,
    pub windows_since_qzc: &'a AtomicU8,
    pub last_qzc_10us: &'a AtomicU32,
    // CL flags.
    pub cl_active: &'a AtomicBool,
    pub cl_armed: &'a AtomicBool,
    pub cl_reacq: &'a AtomicBool,
    pub cl_noz_run: &'a AtomicU8,
    pub cl_fast_path: &'a AtomicBool,
    // REJECTION CENSUS counters (accepted samples + each silent-
    // rejection kind; see estimator::Reject).
    pub est_acc: &'a AtomicU32,
    pub est_rej_floor: &'a AtomicU32,
    pub est_rej_ceiling: &'a AtomicU32,
    pub est_rej_rate: &'a AtomicU32,
    pub est_rej_reseed: &'a AtomicU32,
    pub est_rej_harmonic: &'a AtomicU32,
    // AM32-GEOMETRY estimator mode (see estimator::am32_geom).
    pub cl_am32_geom: &'a AtomicBool,
    pub est_prev_period: &'a AtomicU32,
    // R2 — the STIFF average (AM32's average_interval, the
    // two-timescale keystone): a 6-deep average of ACCEPTED
    // intervals, kept as a fixed-point accumulator (acc/6 = avg) so
    // integer floor can't stall it. One accept moves it by delta/6 —
    // a premature edge cannot drag the gate down the way it drags
    // the fast estimate. Updated only here (accepts are serialized
    // by the window qZC mask).
    pub avg_interval_acc: &'a AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldEdge {
    /// SWIFT: accept immediately in the COMP ISR (edge-timestamped,
    /// zero wrap latency) — the caller invokes the accept path now.
    AcceptNow,
    /// Candidate armed; TIM1_UP confirms or discards it.
    Armed,
    /// A candidate is already pending — one candidate at a time.
    Ignored,
}

/// COMP context, after the persistence loop held. Publish order for
/// the candidate is load-bearing: EXPECTED/CONFIRMS/GEN before the
/// ZC word — TIM1_UP keys on `cand_zc_us != MAX`, so the metadata
/// must be consistent before the candidate becomes visible.
#[inline]
pub fn on_held_edge(zs: &ZcState<'_>, expected: bool, now_us: u32) -> HeldEdge {
    // SWIFT accepts immediately under lock — EXCEPT in re-acquisition
    // at the TOP END. Reacq widens the gate to 8 % after a ZC miss;
    // near the ceiling (interval < TOPEND_US) that widened gate admits
    // the premature/noise edge that seeds the divergence, so fall back
    // to the candidate + wrap-confirm path. Gated to the top end: at
    // mid speed the confirm latency only hurts recovery and the amp
    // 40-44 climb (bench), so keep the proven immediate accept there.
    let reacq_topend = zs.cl_reacq.load(Ordering::Relaxed) && {
        let iv = zs.interval_us.load(Ordering::Relaxed);
        iv > 0 && iv < crate::window::TOPEND_US
    };
    if zs.cl_fast_path.load(Ordering::Relaxed)
        && zs.cl_active.load(Ordering::Relaxed)
        && !reacq_topend
    {
        HeldEdge::AcceptNow
    } else if zs.cand_zc_us.load(Ordering::Relaxed) == u32::MAX {
        zs.cand_expected.store(expected, Ordering::Relaxed);
        zs.cand_confirms.store(0, Ordering::Relaxed);
        zs.cand_gen
            .store(zs.window_gen.load(Ordering::Relaxed), Ordering::Relaxed);
        zs.cand_zc_us.store(now_us, Ordering::Relaxed);
        HeldEdge::Armed
    } else {
        HeldEdge::Ignored
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// No candidate pending (or it was replaced under us).
    Idle,
    /// Confirmation deepened; not enough yet.
    Progress,
    /// Depth reached; the candidate has been consumed (cleared) and
    /// its generation snapshot captured. The caller enters its
    /// critical section and gates the accept on
    /// [`accept_gen_current`]`(zs, gen)`.
    AcceptPending { zc_us: u32, gen_snap: u8 },
    /// Observation contradicted the expected level; candidate
    /// discarded (`cl_active` gates the DIS black-box line).
    Discarded { confirms: u8, cl_active: bool },
}

/// TIM1_UP context: one wrap-sample confirmation step. Depth comes
/// from [`crate::timing::confirm_need`] (2 until CL_ACTIVE, 1 after,
/// 2 again in re-acquisition — the unconditional-1-confirm engage
/// runaway is the regression behind it).
///
/// Candidate lifetime matches the proven firmware: consumed
/// (cleared) HERE at depth, BEFORE the caller's critical section, so
/// a preempting COMP can arm the next window's candidate without
/// waiting on the accept. (A first cut held the candidate live until
/// inside the CS; on the bench that starved the accelerating-lock
/// regime — mid-acceleration losses walked the estimator into
/// 4-24 ms chaos. Candidate lifetime is load-bearing.)
///
/// Both the consume and the discard are compare-exchanges, so a
/// candidate re-armed between our load and the clear survives. The
/// TOCTOU (B1) is closed by the generation SNAPSHOT taken at
/// candidate-load time: a window close + fresh re-arm refreshes
/// `cand_gen` to the NEW generation (which defeated the original
/// CAND_GEN-vs-WINDOW_GEN re-check), but the stale LOCAL snapshot
/// still mismatches `window_gen` inside the CS.
#[inline]
pub fn confirm_step(zs: &ZcState<'_>, observed: bool) -> ConfirmAction {
    let cand = zs.cand_zc_us.load(Ordering::Relaxed);
    // Paired generation snapshot. (If a close lands between the two
    // loads, the candidate word is already MAX or replaced and the
    // compare-exchanges below refuse the stale value.)
    let gen_snap = zs.cand_gen.load(Ordering::Relaxed);
    if cand == u32::MAX {
        return ConfirmAction::Idle;
    }
    if observed == zs.cand_expected.load(Ordering::Relaxed) {
        let need = crate::timing::confirm_need(
            zs.cl_active.load(Ordering::Relaxed),
            zs.cl_reacq.load(Ordering::Relaxed),
            zs.interval_us.load(Ordering::Relaxed),
        ) as u8;
        let n = zs.cand_confirms.load(Ordering::Relaxed) + 1;
        if n >= need {
            if zs
                .cand_zc_us
                .compare_exchange(cand, u32::MAX, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                ConfirmAction::AcceptPending {
                    zc_us: cand,
                    gen_snap,
                }
            } else {
                // Replaced under us (close + re-arm): the fresh
                // candidate belongs to the new window — leave it.
                ConfirmAction::Idle
            }
        } else {
            zs.cand_confirms.store(n, Ordering::Relaxed);
            ConfirmAction::Progress
        }
    } else {
        let confirms = zs.cand_confirms.load(Ordering::Relaxed);
        let cl_active = zs.cl_active.load(Ordering::Relaxed);
        let _ =
            zs.cand_zc_us
                .compare_exchange(cand, u32::MAX, Ordering::Relaxed, Ordering::Relaxed);
        ConfirmAction::Discarded {
            confirms,
            cl_active,
        }
    }
}

/// MUST run inside the caller's critical section: is the candidate's
/// generation snapshot still the live window? Refuses stale accepts
/// whose window closed between the confirm and the CS — including
/// the close + fresh-re-arm interleaving that defeats a
/// `cand_gen`-based re-check (B1).
#[inline]
pub fn accept_gen_current(zs: &ZcState<'_>, gen_snap: u8) -> bool {
    gen_snap == zs.window_gen.load(Ordering::Relaxed)
}

/// What the caller does after a successful publish: record the ENG
/// event if `engaged`, and when `schedule` is set run the scheduling
/// tail (auto-advance + delay arithmetic — already host-tested in
/// `timing` — then the LPTIM2 write, SHOT_REFINED, the ACC event,
/// and mask-after-accept).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptPlan {
    pub engaged: bool,
    pub schedule: bool,
    pub interval_us: u32,
}

/// Shared accept: publish the window's qualified ZC (mask-after-
/// accept: `None` when the window already has one), update the OWL
/// estimator (load-run-store through [`crate::estimator`], same
/// non-transactional profile as before), and decide engage/schedule.
/// C float windows (sectors 0/3, dead-reckoned) must neither
/// schedule nor engage.
#[inline]
pub fn accept_publish(
    zs: &ZcState<'_>,
    sector: u8,
    zc_us: u32,
    now_10us: u32,
) -> Option<AcceptPlan> {
    if zs.window_qzc_us.load(Ordering::Relaxed) != u32::MAX {
        return None;
    }
    zs.window_qzc_us.store(zc_us, Ordering::Relaxed);

    let mut est = crate::estimator::Estimator {
        interval_us: zs.interval_us.load(Ordering::Relaxed),
        last_qzc_us: match zs.last_qzc_us.load(Ordering::Relaxed) {
            u32::MAX => None,
            v => Some(v),
        },
        windows_since_qzc: zs.windows_since_qzc.load(Ordering::Relaxed) as u32,
        reacq: zs.cl_reacq.load(Ordering::Relaxed),
        am32_geom: zs.cl_am32_geom.load(Ordering::Relaxed),
        prev_period_us: zs.est_prev_period.load(Ordering::Relaxed),
    };
    let (_, reject) = est.on_accept_traced(zc_us, zs.cl_active.load(Ordering::Relaxed));
    match reject {
        None => zs.est_acc.fetch_add(1, Ordering::Relaxed),
        Some(crate::estimator::Reject::Floor) => zs.est_rej_floor.fetch_add(1, Ordering::Relaxed),
        Some(crate::estimator::Reject::Ceiling) => {
            zs.est_rej_ceiling.fetch_add(1, Ordering::Relaxed)
        }
        Some(crate::estimator::Reject::RateBound) => {
            zs.est_rej_rate.fetch_add(1, Ordering::Relaxed)
        }
        Some(crate::estimator::Reject::Reseed) => zs.est_rej_reseed.fetch_add(1, Ordering::Relaxed),
        Some(crate::estimator::Reject::Harmonic) => {
            zs.est_rej_harmonic.fetch_add(1, Ordering::Relaxed)
        }
    };
    zs.interval_us.store(est.interval_us, Ordering::Relaxed);
    zs.last_qzc_us
        .store(est.last_qzc_us.unwrap_or(u32::MAX), Ordering::Relaxed);
    zs.windows_since_qzc
        .store(est.windows_since_qzc.min(255) as u8, Ordering::Relaxed);
    zs.cl_reacq.store(est.reacq, Ordering::Relaxed);
    zs.est_prev_period
        .store(est.prev_period_us, Ordering::Relaxed);
    {
        // R2 stiff average update: acc += interval - acc/6 (6-deep
        // average as a fixed-point accumulator; seeded on first use).
        // GEOMETRY-MODE ONLY: this sits on the accept path inside
        // the COMP ISR - the stock path must stay byte-lean (the
        // R2/R3 first cut added ~1 us there and regressed paired
        // ladders vs the r1b tag).
        let acc = zs.avg_interval_acc.load(Ordering::Relaxed);
        let iv = zs.interval_us.load(Ordering::Relaxed);
        if iv != 0 && zs.cl_am32_geom.load(Ordering::Relaxed) {
            let new_acc = if acc == 0 {
                iv.saturating_mul(6)
            } else {
                acc.wrapping_add(iv).wrapping_sub(acc / 6)
            };
            zs.avg_interval_acc.store(new_acc, Ordering::Relaxed);
        }
    }
    zs.cl_noz_run.store(0, Ordering::Relaxed);
    zs.last_qzc_10us.store(now_10us, Ordering::Relaxed);

    let interval_us = zs.interval_us.load(Ordering::Relaxed);
    let mut engaged = false;
    let mut schedule = false;
    let is_ab = sector != 0 && sector != 3;
    if interval_us != 0 {
        // Engage only on an A/B sector — C is never the engage window.
        if is_ab && zs.cl_armed.load(Ordering::Relaxed) {
            zs.cl_armed.store(false, Ordering::Relaxed);
            zs.cl_active.store(true, Ordering::Relaxed);
            engaged = true;
        }
        if zs.cl_active.load(Ordering::Relaxed) {
            // A/B always re-time the commutation from the accepted ZC.
            // C (sectors 0/3) re-times too — but ONLY under SWIFT AND at
            // the TOP END (interval < TOPEND_US). Under SWIFT the
            // comparator gives a real C ZC (via the CHAMELEON mux; the
            // ADC-confirm path has no C route). At the ceiling, re-timing
            // C removes the free-run error that opens the following A/B
            // window late (the ZC-miss initiator) — worth amp 50→55. But
            // at mid speed the dead-reckon is more robust than a
            // possibly-noisy comparator C ZC while accelerating (bench:
            // re-timing C broke the amp 40-44 climb), so keep dead-reckon
            // there.
            let c_retime = zs.cl_fast_path.load(Ordering::Relaxed)
                && (zs.zc_clocked.load(Ordering::Relaxed)
                    || interval_us < crate::window::TOPEND_US);
            schedule = is_ab || c_retime;
        }
    }
    Some(AcceptPlan {
        engaged,
        schedule,
        interval_us,
    })
}

/// R2: the stiff average in µs (acc/6); 0 when unseeded.
#[inline]
pub fn avg_interval_us(zs: &ZcState<'_>) -> u32 {
    zs.avg_interval_acc.load(Ordering::Relaxed) / 6
}

/// SCHEDULE-FIRST pre-check (the `elapsed` cut): decide — BEFORE the
/// estimator runs — whether this accept will re-time the commutation,
/// and hand back the PRE-update interval to compute the delay from.
///
/// Rationale (measured 2026-07-14): `elapsed` (ZC → schedule call) was
/// 16–18 µs, of which 6–8 µs was `accept_publish`'s estimator work done
/// before the delay was even computed — pure added lateness, and the
/// entire schedule budget at ~1800 Hz. The firmware now schedules from
/// this pre-check FIRST, then runs [`accept_publish`] for the estimator
/// and publishes. The pre-update interval is what the free-run already
/// scheduled from at the previous commutation; the ±25 %-bounded
/// ¾-smoothed update makes pre≈post at steady state (one window stale
/// during accel — bounded by the same rate bound).
///
/// MUST mirror `accept_publish`'s schedule decision. The one legitimate
/// divergence: near the TOPEND boundary the pre/post interval can
/// straddle 125 µs — a one-window scheduling-mode difference for a C
/// window, same class as the boundary toggling that already exists.
/// The engage accept (cl_armed, A/B) schedules in `accept_publish` via
/// the just-set cl_active — pre-check treats `cl_armed` as schedulable
/// so the engage's first shot is also scheduled early.
#[inline]
pub fn schedule_precheck(zs: &ZcState<'_>, sector: u8) -> Option<u32> {
    if zs.window_qzc_us.load(Ordering::Relaxed) != u32::MAX {
        return None; // mask-after-accept: window already has its ZC
    }
    let iv = zs.interval_us.load(Ordering::Relaxed);
    if iv == 0 {
        return None; // unseeded estimator
    }
    let is_ab = sector != 0 && sector != 3;
    let ok = if is_ab {
        zs.cl_active.load(Ordering::Relaxed) || zs.cl_armed.load(Ordering::Relaxed)
    } else {
        zs.cl_active.load(Ordering::Relaxed)
            && zs.cl_fast_path.load(Ordering::Relaxed)
            && (zs.zc_clocked.load(Ordering::Relaxed) || iv < crate::window::TOPEND_US)
    };
    if ok { Some(iv) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ab_sectors_compare_float_to_neutral() {
        // Sector 1: B floats, neutral = A/2. B just below → true.
        assert!(adc_sign_observed(1, 1000, 499, 0, false));
        assert!(!adc_sign_observed(1, 1000, 500, 0, false)); // 2B == A: not below
        // Sector 2: A floats vs B/2.
        assert!(adc_sign_observed(2, 499, 1000, 0, false));
        assert!(!adc_sign_observed(2, 501, 1000, 0, false));
        // Sector 4: B floats, neutral = (vbus + A)/2.
        assert!(adc_sign_observed(4, 100, 900, 1900, false)); // 1800 < 2000
        assert!(!adc_sign_observed(4, 100, 1100, 1900, false));
        // Sector 5: A floats, neutral = (vbus + B)/2.
        assert!(adc_sign_observed(5, 900, 100, 1900, false));
        assert!(!adc_sign_observed(5, 1100, 100, 1900, false));
    }

    #[test]
    fn c_sectors_pass_comp_bit_through() {
        for sec in [0u8, 3] {
            assert!(adc_sign_observed(sec, 0, 0, 0, true));
            assert!(!adc_sign_observed(sec, 4000, 4000, 4000, false));
        }
    }

    #[test]
    fn regression_sector2_zc_boundary_48khz() {
        // The sector-2 mystery: at the ZC, 2×float-A ≈ vbus by
        // construction. The rule must flip exactly there — a sample
        // taken during dead-time/ringing (A reads low) must not read
        // as "confirmed" when B is also corrupted low. Pin the
        // boundary arithmetic.
        let vbus = 1900u16; // ≈ 8 V through the divider, in counts
        let b_driven = vbus; // B is the driven-high phase in sec 2
        // A exactly at neutral (B/2): NOT below.
        assert!(!adc_sign_observed(2, b_driven / 2, b_driven, vbus, false));
        // 1 count under: below.
        assert!(adc_sign_observed(
            2,
            b_driven / 2 - 1,
            b_driven,
            vbus,
            false
        ));
    }

    #[test]
    fn vbus_tracks_max_and_decays() {
        // Rises immediately to the driven-high read.
        assert_eq!(vbus_decay_step(0, 1900, 100), 1900);
        assert_eq!(vbus_decay_step(1800, 100, 1900), 1900);
        // Decays by max(est>>9, 1) when no larger sample.
        assert_eq!(vbus_decay_step(1900, 0, 0), 1900 - 3);
        assert_eq!(vbus_decay_step(511, 0, 0), 510); // >>9 == 0 → min 1
        // Floor: zero stays zero (no underflow churn).
        assert_eq!(vbus_decay_step(0, 0, 0), 0);
        assert_eq!(vbus_decay_step(1, 0, 0), 0);
    }

    // ---- state machine ----

    struct Rig {
        zc_clocked: AtomicBool,
        cand_zc_us: AtomicU32,
        cand_expected: AtomicBool,
        cand_confirms: AtomicU8,
        cand_gen: AtomicU8,
        window_gen: AtomicU8,
        window_qzc_us: AtomicU32,
        interval_us: AtomicU32,
        last_qzc_us: AtomicU32,
        windows_since_qzc: AtomicU8,
        last_qzc_10us: AtomicU32,
        cl_active: AtomicBool,
        cl_armed: AtomicBool,
        cl_reacq: AtomicBool,
        cl_noz_run: AtomicU8,
        cl_fast_path: AtomicBool,
        est_acc: AtomicU32,
        est_rej_floor: AtomicU32,
        est_rej_ceiling: AtomicU32,
        est_rej_rate: AtomicU32,
        est_rej_reseed: AtomicU32,
        est_rej_harmonic: AtomicU32,
        cl_am32_geom: AtomicBool,
        est_prev_period: AtomicU32,
        avg_interval_acc: AtomicU32,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                zc_clocked: AtomicBool::new(false),
                cand_zc_us: AtomicU32::new(u32::MAX),
                cand_expected: AtomicBool::new(false),
                cand_confirms: AtomicU8::new(0),
                cand_gen: AtomicU8::new(0),
                window_gen: AtomicU8::new(0),
                window_qzc_us: AtomicU32::new(u32::MAX),
                interval_us: AtomicU32::new(600),
                last_qzc_us: AtomicU32::new(9_400),
                windows_since_qzc: AtomicU8::new(0),
                last_qzc_10us: AtomicU32::new(0),
                cl_active: AtomicBool::new(false),
                cl_armed: AtomicBool::new(false),
                cl_reacq: AtomicBool::new(false),
                cl_noz_run: AtomicU8::new(3),
                cl_fast_path: AtomicBool::new(false),
                est_acc: AtomicU32::new(0),
                est_rej_floor: AtomicU32::new(0),
                est_rej_ceiling: AtomicU32::new(0),
                est_rej_rate: AtomicU32::new(0),
                est_rej_reseed: AtomicU32::new(0),
                est_rej_harmonic: AtomicU32::new(0),
                cl_am32_geom: AtomicBool::new(false),
                est_prev_period: AtomicU32::new(0),
                avg_interval_acc: AtomicU32::new(0),
            }
        }

        fn zs(&self) -> ZcState<'_> {
            ZcState {
                zc_clocked: &self.zc_clocked,
                cand_zc_us: &self.cand_zc_us,
                cand_expected: &self.cand_expected,
                cand_confirms: &self.cand_confirms,
                cand_gen: &self.cand_gen,
                window_gen: &self.window_gen,
                window_qzc_us: &self.window_qzc_us,
                interval_us: &self.interval_us,
                last_qzc_us: &self.last_qzc_us,
                windows_since_qzc: &self.windows_since_qzc,
                last_qzc_10us: &self.last_qzc_10us,
                cl_active: &self.cl_active,
                cl_armed: &self.cl_armed,
                cl_reacq: &self.cl_reacq,
                cl_noz_run: &self.cl_noz_run,
                cl_fast_path: &self.cl_fast_path,
                est_acc: &self.est_acc,
                est_rej_floor: &self.est_rej_floor,
                est_rej_ceiling: &self.est_rej_ceiling,
                est_rej_rate: &self.est_rej_rate,
                est_rej_reseed: &self.est_rej_reseed,
                est_rej_harmonic: &self.est_rej_harmonic,
                cl_am32_geom: &self.cl_am32_geom,
                est_prev_period: &self.est_prev_period,
                avg_interval_acc: &self.avg_interval_acc,
            }
        }

        /// What close_float_window does to the candidate/generation
        /// (window.rs owns the full close; this is the slice relevant
        /// to the interleavings here).
        fn close_window(&self) {
            self.cand_zc_us.store(u32::MAX, Ordering::Relaxed);
            self.window_gen.store(
                self.window_gen.load(Ordering::Relaxed).wrapping_add(1),
                Ordering::Relaxed,
            );
            self.window_qzc_us.store(u32::MAX, Ordering::Relaxed);
        }
    }

    #[test]
    fn swift_accepts_in_comp_only_under_active_fast_path() {
        let r = Rig::new();
        r.cl_fast_path.store(true, Ordering::Relaxed);
        r.cl_active.store(true, Ordering::Relaxed);
        assert_eq!(on_held_edge(&r.zs(), true, 10_000), HeldEdge::AcceptNow);
        // In re-acquisition AT THE TOP END, SWIFT must NOT AcceptNow —
        // it arms a candidate so the widened 8 % gate can't feed an
        // unconfirmed (premature) edge (the amp-52 divergence seed).
        r.cl_reacq.store(true, Ordering::Relaxed);
        r.interval_us.store(110, Ordering::Relaxed); // < TOPEND_US
        assert_eq!(on_held_edge(&r.zs(), true, 10_000), HeldEdge::Armed);
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), 10_000);
        r.close_window();
        // But at mid speed, reacq still accepts immediately (the
        // confirm latency there hurts the climb).
        r.interval_us.store(300, Ordering::Relaxed); // > TOPEND_US
        assert_eq!(on_held_edge(&r.zs(), true, 10_000), HeldEdge::AcceptNow);
        r.cl_reacq.store(false, Ordering::Relaxed);
        r.interval_us.store(0, Ordering::Relaxed);
        r.close_window();
        // Fast path off, or engage/open-loop: candidate route.
        r.cl_active.store(false, Ordering::Relaxed);
        assert_eq!(on_held_edge(&r.zs(), true, 10_000), HeldEdge::Armed);
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), 10_000);
        assert!(r.cand_expected.load(Ordering::Relaxed));
        assert_eq!(r.cand_confirms.load(Ordering::Relaxed), 0);
        // One candidate at a time.
        assert_eq!(on_held_edge(&r.zs(), false, 10_050), HeldEdge::Ignored);
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), 10_000);
    }

    #[test]
    fn confirm_depth_two_open_loop_one_under_lock() {
        // Open loop / engage: 2 confirms (the unconditional-1-confirm
        // engage-runaway regression).
        let r = Rig::new();
        on_held_edge(&r.zs(), true, 10_000);
        assert_eq!(confirm_step(&r.zs(), true), ConfirmAction::Progress);
        assert_eq!(
            confirm_step(&r.zs(), true),
            ConfirmAction::AcceptPending {
                zc_us: 10_000,
                gen_snap: 0
            }
        );
        // Candidate consumed at depth (proven lifetime: COMP can arm
        // the next window's candidate without waiting on the accept).
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), u32::MAX);
        // Under an established lock: 1 confirm.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        on_held_edge(&r.zs(), true, 10_000);
        assert_eq!(
            confirm_step(&r.zs(), true),
            ConfirmAction::AcceptPending {
                zc_us: 10_000,
                gen_snap: 0
            }
        );
        // Re-acquisition: strict 2 again.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        r.cl_reacq.store(true, Ordering::Relaxed);
        on_held_edge(&r.zs(), true, 10_000);
        assert_eq!(confirm_step(&r.zs(), true), ConfirmAction::Progress);
    }

    #[test]
    fn discard_logs_and_clears_but_spares_a_fresh_candidate() {
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        on_held_edge(&r.zs(), true, 10_000);
        let a = confirm_step(&r.zs(), false);
        assert_eq!(
            a,
            ConfirmAction::Discarded {
                confirms: 0,
                cl_active: true
            }
        );
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), u32::MAX);
        // Same-family race as B1: candidate replaced between the
        // step's load and its clear — the compare-exchange must spare
        // the fresh one. (Simulated by hand-rolling the stale clear.)
        on_held_edge(&r.zs(), true, 11_000);
        let stale = 10_000u32;
        let _ =
            r.cand_zc_us
                .compare_exchange(stale, u32::MAX, Ordering::Relaxed, Ordering::Relaxed);
        assert_eq!(
            r.cand_zc_us.load(Ordering::Relaxed),
            11_000,
            "fresh candidate destroyed by a stale discard"
        );
    }

    #[test]
    fn gen_snapshot_current_on_happy_path() {
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        on_held_edge(&r.zs(), true, 10_000);
        match confirm_step(&r.zs(), true) {
            ConfirmAction::AcceptPending { zc_us, gen_snap } => {
                assert_eq!(zc_us, 10_000);
                assert!(accept_gen_current(&r.zs(), gen_snap));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn regression_b1_toctou_close_plus_rearm_defeats_gen_guard() {
        // The exact interleaving from the review: TIM1_UP confirms a
        // candidate; a window close AND a fresh COMP arm land before
        // its critical section. The fresh arm refreshes cand_gen to
        // the NEW generation — the original CAND_GEN-vs-WINDOW_GEN
        // re-check PASSED and accepted the stale timestamp. The
        // load-time generation snapshot must refuse it, and the fresh
        // candidate must survive.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed); // depth 1
        on_held_edge(&r.zs(), true, 10_000);
        let (stale_zc, stale_gen) = match confirm_step(&r.zs(), true) {
            ConfirmAction::AcceptPending { zc_us, gen_snap } => (zc_us, gen_snap),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(stale_zc, 10_000);
        // ... LPTIM2 preempts: closes the window ...
        r.close_window();
        // ... COMP preempts: arms a FRESH candidate in the new window
        // (this is what refreshes cand_gen to the new generation) ...
        assert_eq!(on_held_edge(&r.zs(), false, 10_640), HeldEdge::Armed);
        assert_eq!(
            r.cand_gen.load(Ordering::Relaxed),
            r.window_gen.load(Ordering::Relaxed),
            "precondition: a CAND_GEN re-check would pass here"
        );
        // ... TIM1_UP resumes inside its CS with the stale snapshot:
        assert!(
            !accept_gen_current(&r.zs(), stale_gen),
            "stale candidate accepted across a window close"
        );
        assert_eq!(
            r.cand_zc_us.load(Ordering::Relaxed),
            10_640,
            "fresh candidate must survive"
        );
    }

    #[test]
    fn depth_consume_spares_a_candidate_replaced_under_us() {
        // Close + re-arm between confirm_step's load and its CAS
        // clear: the step must return Idle and leave the fresh
        // candidate alone. (Simulated by replacing the candidate,
        // then hand-running the stale CAS the step would attempt.)
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        on_held_edge(&r.zs(), true, 10_000);
        r.close_window();
        on_held_edge(&r.zs(), true, 10_640);
        let stale = 10_000u32;
        assert!(
            r.cand_zc_us
                .compare_exchange(stale, u32::MAX, Ordering::Relaxed, Ordering::Relaxed)
                .is_err(),
            "stale CAS must fail against the fresh candidate"
        );
        assert_eq!(r.cand_zc_us.load(Ordering::Relaxed), 10_640);
    }

    #[test]
    fn r2_stiff_average_is_unwalkable() {
        // The two-timescale keystone: a premature accept drags the
        // FAST estimate but barely moves the stiff average the gate
        // references. Feed steady 345 us then one 190 us junk accept
        // (geometry mode, no bounds).
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        r.cl_am32_geom.store(true, Ordering::Relaxed);
        r.interval_us.store(345, Ordering::Relaxed);
        r.est_prev_period.store(345, Ordering::Relaxed);
        let mut zc = 10_000u32;
        for _ in 0..30 {
            zc += 345;
            r.window_qzc_us.store(u32::MAX, Ordering::Relaxed);
            r.windows_since_qzc.store(1, Ordering::Relaxed);
            accept_publish(&r.zs(), 2, zc, 1_000).unwrap();
        }
        let avg_before = avg_interval_us(&r.zs());
        assert!((330..=350).contains(&avg_before), "avg {avg_before}");
        // one premature accept at 240 us (-30 %: inside the wide
        // sanity band, so it IS accepted - the interesting case; a
        // 190 us sample is already refused by the [2/3,3/2] bound,
        // itself part of the defense in depth)
        zc += 240;
        r.window_qzc_us.store(u32::MAX, Ordering::Relaxed);
        r.windows_since_qzc.store(1, Ordering::Relaxed);
        accept_publish(&r.zs(), 2, zc, 1_000).unwrap();
        let avg_after = avg_interval_us(&r.zs());
        // stiff: moves by <= delta/6 (~26 us), NOT to the junk value
        assert!(avg_after >= avg_before - 30, "avg collapsed: {avg_after}");
        // while the fast estimate moved further (the schedule tracks)
        let fast = r.interval_us.load(Ordering::Relaxed);
        assert!(fast < avg_after, "fast {fast} avg {avg_after}");
    }

    #[test]
    fn accept_publish_masks_estimates_and_engages() {
        let r = Rig::new();
        r.cl_armed.store(true, Ordering::Relaxed);
        r.windows_since_qzc.store(1, Ordering::Relaxed);
        let plan = accept_publish(&r.zs(), 2, 10_000, 1_000).unwrap();
        assert!(plan.engaged, "armed + A/B sector engages");
        assert!(plan.schedule);
        assert!(r.cl_active.load(Ordering::Relaxed));
        assert!(!r.cl_armed.load(Ordering::Relaxed));
        assert_eq!(r.window_qzc_us.load(Ordering::Relaxed), 10_000);
        assert_eq!(r.cl_noz_run.load(Ordering::Relaxed), 0, "NOZ run resets");
        assert_eq!(r.last_qzc_10us.load(Ordering::Relaxed), 1_000);
        // Estimator ran: 10 000 − 9 400 = 600 over 1 span → smoothed
        // stays 600.
        assert_eq!(plan.interval_us, 600);
        // Mask-after-accept: a second ZC in the same window is dead.
        assert_eq!(accept_publish(&r.zs(), 2, 10_050, 1_001), None);
    }

    #[test]
    fn accept_publish_c_windows_never_engage_and_reckon_without_swift() {
        // C never engages. Without SWIFT (ADC-confirm), C never
        // schedules either — no ADC route on phase C, so it stays
        // dead-reckoned.
        for sec in [0u8, 3] {
            let r = Rig::new();
            r.cl_armed.store(true, Ordering::Relaxed);
            r.cl_active.store(true, Ordering::Relaxed);
            let plan = accept_publish(&r.zs(), sec, 10_000, 1_000).unwrap();
            assert!(!plan.engaged, "sector {sec} engaged");
            assert!(!plan.schedule, "sector {sec} scheduled without SWIFT");
            assert!(r.cl_armed.load(Ordering::Relaxed), "arm must survive");
        }
        // Unseeded estimator: no schedule either.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        r.interval_us.store(0, Ordering::Relaxed);
        r.last_qzc_us.store(u32::MAX, Ordering::Relaxed);
        let plan = accept_publish(&r.zs(), 2, 10_000, 1_000).unwrap();
        assert!(!plan.schedule);
    }

    #[test]
    #[test]
    fn regression_invert2_c_windows_must_arm_when_rotor_clocked() {
        // invert2 incident (2026-07-17): under the scheduling
        // inversion, phase-C windows (sectors 0/3) hit the TOPEND
        // gate at mid speed and armed nothing - with no free-run to
        // dead-reckon them the chain stalled ~3 ms at every C window
        // (reseed storm -> strike kill at amp 60). zc_clocked lifts
        // the gate: C arms at ANY speed under SWIFT.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        r.cl_fast_path.store(true, Ordering::Relaxed);
        r.interval_us.store(450, Ordering::Relaxed); // mid speed > TOPEND
        r.window_qzc_us.store(u32::MAX, Ordering::Relaxed);
        // Free-run world: C must NOT arm (dead-reckon covers it).
        r.zc_clocked.store(false, Ordering::Relaxed);
        assert_eq!(schedule_precheck(&r.zs(), 0), None);
        assert_eq!(schedule_precheck(&r.zs(), 3), None);
        // Rotor-clocked world: C MUST arm.
        r.zc_clocked.store(true, Ordering::Relaxed);
        assert_eq!(schedule_precheck(&r.zs(), 0), Some(450));
        assert_eq!(schedule_precheck(&r.zs(), 3), Some(450));
        // A/B unaffected either way.
        assert_eq!(schedule_precheck(&r.zs(), 1), Some(450));
    }

    fn schedule_precheck_agrees_with_accept_publish() {
        // Firmware order: precheck (schedule-first) THEN accept_publish.
        // The precheck's decision must equal plan.schedule for every
        // flag/sector/interval combination (steady state: spans=0 so the
        // estimator doesn't move the interval between the two calls).
        for sector in [0u8, 1, 2, 3, 4, 5] {
            for &(active, armed, fast) in &[
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (true, false, true),
                (false, true, true),
                (true, true, true),
            ] {
                for &iv in &[0u32, 110, 600] {
                    let r = Rig::new();
                    r.cl_active.store(active, Ordering::Relaxed);
                    r.cl_armed.store(armed, Ordering::Relaxed);
                    r.cl_fast_path.store(fast, Ordering::Relaxed);
                    r.interval_us.store(iv, Ordering::Relaxed);
                    let pre = schedule_precheck(&r.zs(), sector);
                    let plan = accept_publish(&r.zs(), sector, 10_000, 1_000).unwrap();
                    assert_eq!(
                        pre.is_some(),
                        plan.schedule,
                        "mismatch: sec={sector} active={active} armed={armed} fast={fast} iv={iv}"
                    );
                    if let Some(p) = pre {
                        assert_eq!(p, iv, "precheck must return the PRE-update interval");
                    }
                }
            }
        }
        // Masked window (already has a qZC): precheck None, publish None.
        let r = Rig::new();
        r.cl_active.store(true, Ordering::Relaxed);
        r.window_qzc_us.store(9_000, Ordering::Relaxed);
        assert!(schedule_precheck(&r.zs(), 2).is_none());
        assert!(accept_publish(&r.zs(), 2, 10_000, 1_000).is_none());
    }

    #[test]
    fn regression_engage_lottery_seeding_accept_2026_07_15() {
        // THE ENGAGE LOTTERY: after arm (estimator reset), the accept
        // that SEEDS the interval can also be the ENGAGE accept (A/B
        // + armed). schedule_precheck sees the PRE-update interval=0
        // and returns None — but accept_publish seeds, engages, and
        // returns schedule=true. The firmware MUST arm the first shot
        // from the plan in this case (post-publish fallback), or
        // nothing ever commutates: TIM7 freezes on cl_active and the
        // desync watchdog kills ~4 ms later (bb: ENG -> silence ->
        // DSY). Engage previously survived only when a C-window qZC
        // seeded first (2 of 6 windows) — the per-build-layout
        // "engage lottery" (bench: one build 0/12, another 8/8).
        let r = Rig::new();
        r.cl_armed.store(true, Ordering::Relaxed);
        r.interval_us.store(0, Ordering::Relaxed); // arm-reset
        r.windows_since_qzc.store(1, Ordering::Relaxed); // spans=1
        // last_qzc set by Rig (a prior no-seed accept at 9 400).
        assert!(
            schedule_precheck(&r.zs(), 2).is_none(),
            "precheck must refuse the unseeded interval"
        );
        let plan = accept_publish(&r.zs(), 2, 10_000, 1_000).unwrap();
        assert!(plan.engaged, "the seeding A/B accept engages");
        assert!(plan.schedule, "and demands a schedule");
        assert_eq!(plan.interval_us, 600, "seeded by this very accept");
        // The firmware pairing: precheck None + plan.schedule true =>
        // the post-publish fallback arms the first shot.
    }

    #[test]
    fn accept_publish_c_windows_reschedule_only_swift_topend() {
        // SWIFT + TOP END: the comparator gives a real C ZC, so C
        // re-times its commutation (but still never engages).
        for sec in [0u8, 3] {
            let r = Rig::new();
            r.cl_active.store(true, Ordering::Relaxed);
            r.cl_fast_path.store(true, Ordering::Relaxed);
            r.cl_armed.store(true, Ordering::Relaxed);
            r.interval_us.store(110, Ordering::Relaxed); // < TOPEND_US
            let plan = accept_publish(&r.zs(), sec, 10_000, 1_000).unwrap();
            assert!(!plan.engaged, "sector {sec} engaged under SWIFT");
            assert!(plan.schedule, "sector {sec} must re-time (SWIFT, top end)");
            assert!(r.cl_armed.load(Ordering::Relaxed), "C must not consume arm");
        }
        // SWIFT but MID speed (default interval 600 > TOPEND_US): C
        // stays dead-reckoned — re-timing there broke the amp 40-44
        // climb on the bench.
        for sec in [0u8, 3] {
            let r = Rig::new();
            r.cl_active.store(true, Ordering::Relaxed);
            r.cl_fast_path.store(true, Ordering::Relaxed);
            let plan = accept_publish(&r.zs(), sec, 10_000, 1_000).unwrap();
            assert!(!plan.schedule, "sector {sec} must dead-reckon at mid speed");
        }
    }

    #[test]
    fn vbus_decay_timescale_rides_through_no_rail_sectors() {
        // Piecewise-linear decay (not exponential): from a full-rail
        // 1900 counts, dropping to ~1900/e takes ~700 PWM cycles
        // (≈29 ms at 24 kHz, ≈15 ms at 48 kHz). What matters on the
        // bench: over the ~2 sectors with no full-rail read (≤ a few
        // hundred µs at speed) the estimate loses only a handful of
        // counts, so the sector-4/5 neutrals stay honest.
        let mut est = 1900u16;
        let mut cycles = 0u32;
        while est > (1900.0 / core::f32::consts::E) as u16 {
            est = vbus_decay_step(est, 0, 0);
            cycles += 1;
        }
        assert!((600..=800).contains(&cycles), "decay cycles = {cycles}");
        // 10 cycles of blackout from full rail costs ≤ 30 counts.
        let mut est = 1900u16;
        for _ in 0..10 {
            est = vbus_decay_step(est, 0, 0);
        }
        assert!(est >= 1870);
    }
}
