//! Commutation timing arithmetic: scheduling delay, auto-advance
//! ramp, ZC gate position, adaptive comparator blanking, and
//! persistence depth. All values are µs unless noted.

/// Auto-advance ramp (AM32's `auto_advance` in spirit): with the
/// manual setting at 0 the advance follows measured speed — 0° below
/// ~280 Hz electrical (measured no-op regime at bench loads), ramping
/// up with speed. A nonzero manual setting overrides the ramp
/// entirely.
///
/// Two components:
/// - **Base ramp** (`/45`, cap 12): unchanged low/mid speed — 0° at
///   the ~600 µs transit, ~8° by 850 Hz (the +108 Hz regime), etc.
/// - **High-speed margin boost** (2026-07-13): below ~200 µs
///   (>~925 Hz) the ZC crowds the 30 % gate — measured landing at
///   0.33 of the window vs the 0.30 gate, ~0 early margin. A single
///   schedule drift then pushes it before the gate and ignites the
///   monster ZC-miss cascade (`monster1..3` autopsy). The boost adds
///   up to +6° by 130 µs so the ZC lands ~16° later in the window,
///   away from the gate. Census @amp 42: monsters 39/20 s → **0**,
///   events de-clustered (Fano 2.5 → 0.9). The transit (>600 µs)
///   stays 0°, so engage is unaffected — the `static 16° kills engage`
///   lesson was about a FLAT advance, not a speed-gated one.
///
/// History: at 850–950 Hz / 350 mA a static 8° was worth +108 Hz (the
/// amp-34 droop was late-commutation braking, not V/f saturation); a
/// static 16° killed the engage transit. Speed-following gives the
/// transit 0° and the top its timing automatically.
#[inline]
pub fn auto_advance_deg(interval_us: u32, manual_deg: i32) -> i32 {
    if manual_deg != 0 {
        return manual_deg;
    }
    let iv = interval_us as i32;
    let base = ((600 - iv).max(0) / 45).min(12);
    let boost = ((200 - iv).max(0) * 6 / 70).min(6);
    (base + boost).min(20)
}

/// Minimum schedulable commutation delay, µs — the commutation-timer
/// floor. History: 24 µs on LPTIM2 /64, then 8 µs (LPTIM2 disable/enable
/// bounce + ARROK sync + warm-up). Now **2 µs** on the TIM15 one-pulse
/// timer: a plain APB2 GP timer at 1 µs/tick with no clock-domain ARR
/// sync and no warm-up, so `schedule_us` = stop→ARR→CNT=0→start fires
/// reliably at ~2 µs. This is the "8→2 µs" win — at ~1800 Hz / 90 µs
/// windows the ideal delay is ~5 µs, so the 8 µs floor was still
/// clamping (late by ~3 µs, the top-end ZC-miss seed); 2 µs passes it
/// unclamped. (TIM15's own `schedule_us` clamps the hardware write to
/// ≥1 µs; this is the control-logic floor.)
pub const LPTIM2_MIN_DELAY_US: i32 = 2;

/// Delay from an accepted ZC to the commutation instant:
/// `interval·(30−adv)/60 − elapsed`, clamped to [`LPTIM2_MIN_DELAY_US`].
#[inline]
pub fn commutation_delay_us(interval_us: u32, adv_deg: i32, elapsed_us: i32) -> u32 {
    (interval_us as i32 * (30 - adv_deg) / 60 - elapsed_us).max(LPTIM2_MIN_DELAY_US) as u32
}

/// Earliest-acceptable-ZC gate, µs from window start. 30 % of the
/// measured interval normally (the value every successful ladder ran
/// at — a brief 20 % excursion let early noise edges reach the
/// 1-confirm fast path and walked the estimator); ~8 % in
/// re-acquisition (just past commutation flyback) so ZCs that
/// drifted early — the lockout-spiral signature — become acceptable
/// again.
#[inline]
pub fn gate_us(interval_us: u32, reacq: bool) -> u32 {
    if reacq {
        (interval_us * 2 / 25).max(10)
    } else {
        (interval_us * 3 / 10).max(20)
    }
}

/// Speed-adaptive comparator blank (cribbed from AM32's actual L431
/// strategy, which has NO time-since-PWM-edge blank at all — its
/// noise defense is persistence depth scaled with speed). Our fixed
/// 8 µs blank left the comparator blind ~75 % of every 48 kHz window
/// and caused the SWIFT amp-28 break. The effective blank shrinks
/// with the measured interval; at low speed it equals the proven
/// user setting exactly.
#[inline]
pub fn blank_us(user_blank_us: u32, interval_us: u32) -> u32 {
    if interval_us != 0 {
        user_blank_us.min(interval_us / 75)
    } else {
        user_blank_us
    }
}

/// Persistence depth for the ZC qualification read-loop: 5 reads
/// while the time-blank is doing the filtering; deepen to AM32's 12
/// once the blank has faded below 5 µs.
#[inline]
pub fn persistence_reads(blank_us: u32) -> u32 {
    if blank_us >= 5 { 5 } else { 12 }
}

/// ZC-confirm depth in PWM wraps: 1 under an established lock, 2
/// otherwise (engage / open loop / re-acquisition). The open-loop
/// 1-confirm is 52-69 % premature (probe replay) and shipped
/// unconditionally it caused engage runaways to 144 µs; under lock,
/// 1-confirm at 48 kHz is empirically clean to 1,348 Hz / 100 %
/// coverage. (A carrier-scaled variant that doubled the counts at
/// 48 kHz was tried during the second 48 kHz push and reverted with
/// the rest of that branch — the unscaled depths are the proven
/// configuration at BOTH carriers.)
#[inline]
pub fn confirm_need(cl_active: bool, reacq: bool) -> u32 {
    if cl_active && !reacq { 1 } else { 2 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_zero_below_280hz() {
        // Measured no-op regime: advance must not fire at low speed.
        assert_eq!(auto_advance_deg(1667, 0), 0); // 100 Hz
        assert_eq!(auto_advance_deg(600, 0), 0); // ~278 Hz
    }

    #[test]
    fn advance_ramps_and_caps() {
        // ~850 Hz (196 µs): ~8° — the regime where 8° won +108 Hz.
        // The high-speed boost hasn't kicked in yet (>200 µs edge).
        let a = auto_advance_deg(196, 0);
        assert!((8..=9).contains(&a), "got {a}");
        // 300 µs / ~550 Hz: base ramp only, unchanged.
        assert_eq!(auto_advance_deg(300, 0), 6);
    }

    #[test]
    fn advance_high_speed_boost_for_margin() {
        // The 2026-07-13 monster fix: below 200 µs the boost lands the
        // ZC later in the window (away from the 30 % gate).
        // 130 µs (~1.3 kHz, amp 42-44): base 10 + boost 6 = 16° — the
        // value that zeroed the amp-42 monsters on the bench.
        assert_eq!(auto_advance_deg(130, 0), 16);
        // Boost edge: at 200 µs no boost yet; just under, it ramps in.
        assert_eq!(auto_advance_deg(200, 0), 8);
        // Very fast: base 12 + boost 6, clamped to 20.
        assert_eq!(auto_advance_deg(60, 0), 18);
        assert_eq!(auto_advance_deg(0, 0), 18);
    }

    #[test]
    fn advance_manual_overrides() {
        assert_eq!(auto_advance_deg(196, 8), 8);
        assert_eq!(auto_advance_deg(1667, 20), 20);
    }

    #[test]
    fn delay_basic_and_clamp() {
        // interval 600 µs, no advance, accepted 60 µs after ZC:
        // 600·30/60 − 60 = 240.
        assert_eq!(commutation_delay_us(600, 0, 60), 240);
        // Late accept can't go below the LPTIM2 floor (reworked 24→8).
        assert_eq!(commutation_delay_us(200, 12, 400), LPTIM2_MIN_DELAY_US as u32);
        // High-speed case that USED to clamp at 24 now passes through:
        // 124·14/60 − 8 = 20 µs (above the 8 µs floor).
        assert_eq!(commutation_delay_us(124, 16, 8), 20);
    }

    #[test]
    fn gate_normal_is_30pct_and_reacq_8pct() {
        assert_eq!(gate_us(1000, false), 300);
        assert_eq!(gate_us(1000, true), 80);
        // Floors.
        assert_eq!(gate_us(10, false), 20);
        assert_eq!(gate_us(10, true), 10);
    }

    #[test]
    fn blank_shrinks_with_speed_regression_amp28() {
        // Proven low-speed behavior unchanged: 8 µs user at 1667 µs.
        assert_eq!(blank_us(8, 1667), 8);
        // 740 Hz (225 µs): 3 µs — the fix that ended the amp-28 wall.
        assert_eq!(blank_us(8, 225), 3);
        // 1.1 kHz: 2 µs.
        assert_eq!(blank_us(8, 150), 2);
        // Unknown interval: user setting verbatim.
        assert_eq!(blank_us(20, 0), 20);
    }

    #[test]
    fn persistence_deepens_as_blank_fades() {
        assert_eq!(persistence_reads(8), 5);
        assert_eq!(persistence_reads(5), 5);
        assert_eq!(persistence_reads(4), 12);
        assert_eq!(persistence_reads(0), 12);
    }

    #[test]
    fn confirm_depth_regime_scoped() {
        // CL fast path 1; engage/open-loop/re-acq demand 2 (the
        // unconditional-1-confirm engage-runaway regression).
        assert_eq!(confirm_need(true, false), 1);
        assert_eq!(confirm_need(false, false), 2);
        assert_eq!(confirm_need(true, true), 2);
        assert_eq!(confirm_need(false, true), 2);
    }
}
