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

/// AM32's LITERAL advance law (main.c: `auto_advance_level =
/// map(duty_cycle, 100, 2000, 13, 23)`): keyed to COMMANDED DUTY —
/// pure feed-forward. The speed-keyed ramp above is POSITIVE
/// FEEDBACK under an estimate excursion (est falls → advance rises →
/// commutation earlier → excursion compounds = a gain term in the
/// subharmonic-walk loop, 2026-07-19 ratchet autopsy). AM32's
/// duty-keyed advance is walk-neutral by construction, and its
/// operating numbers verify the law (14° at ~10 % duty = their
/// measured wait/ci = 0.297). Duty in percent (5..100 ≈ their
/// 100..2000/2000): 13° at ≤5 % → 23° at 100 %.
#[inline]
pub fn auto_advance_deg_am32(duty_pct: u32, manual_deg: i32) -> i32 {
    if manual_deg != 0 {
        return manual_deg;
    }
    let d = duty_pct.clamp(5, 100) as i32;
    13 + (d - 5) * 10 / 95
}

/// Minimum schedulable commutation delay, µs — the commutation-timer
/// floor. History: 24 µs on LPTIM2 /64, then 8, then 2 (the floor
/// rework). NOTE (E6 doc-truth): the TIM15 one-pulse swap this
/// comment once narrated was REVERTED — LPTIM2 /64 + floor 2 beat it
/// in a one-variable control (pred jitter 0.9 % vs 3.4 %, APB2
/// domain jitter). The flashed commutation timer is LPTIM2; its
/// hardware clamp is 4 µs (lptim2_oneshot `clamp(4, ..)`), so THIS
/// logic floor of 2 is currently shadowed by the hardware clamp, and
/// the full-path ~3 µs overhead is compensated via
/// [`LPTIM2_FULL_SCHEDULE_OVERHEAD_US`] (E3), not by this constant.
pub const LPTIM2_MIN_DELAY_US: i32 = 2;

/// E3 (CONSTANTS_AUDIT 2026-07-15): fixed overhead of the LPTIM2
/// FULL schedule path (`schedule_us`: disable/enable bounce +
/// `asm::delay(200)` warm-up + ARROK sync ≈ 3 µs) that elapses AFTER
/// the delay value is computed and BEFORE the count starts. The
/// ZC-refine path must fold this into `elapsed` or every refined
/// commutation fires ~3 µs late — while free-run re-arms
/// (`reschedule_light`, no bounce/warm-up) fire on time: an
/// alternating late/on-time jitter that seeds top-end divergence
/// (at 84-95 µs windows, 3 µs ≈ 13° electrical of the refine delay).
/// The free-run path must NOT apply this.
pub const LPTIM2_FULL_SCHEDULE_OVERHEAD_US: i32 = 3;

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

/// AM32-verbatim gate for geometry mode: `average_interval / 2`
/// (main.c COMP gate `INTERVAL_TIMER->CNT > average_interval/2`),
/// keyed to the STIFF average like theirs. Found by the 2026-07-19
/// even/odd autopsy at 322 Hz: our 30 % gate (133 µs of a 442 µs
/// window) admitted a PWM-dwell noise edge at ~199 µs on the odd
/// (rising-BEMF) windows — 40 µs BEFORE the true ZC — while AM32's
/// half-interval gate (221 µs) blocks it on the same board. The
/// resulting ±75 µs even/odd commutation alternation was the static
/// two-band structure under every climb wobble. Re-acq keeps the
/// widened 8 % gate.
#[inline]
pub fn gate_us_am32(avg_interval_us: u32, interval_us: u32, reacq: bool) -> u32 {
    if reacq {
        return gate_us(interval_us, true);
    }
    let base = if avg_interval_us != 0 {
        avg_interval_us
    } else {
        interval_us
    };
    (base / 2).max(20)
}

/// PARITY TRIM (2026-07-19, the half-speed trap autopsy): qualified
/// ZCs are displaced ±δ by BEMF polarity (δ ≈ 37 µs at 322 Hz —
/// offset-through-slope, measured on BOTH firmwares: AM32's own
/// low-speed trace alternates steps 1/3/5 = 340 µs vs 2/4/6 = 302 µs
/// on this bench). The loop echoes the displacement into a stable
/// ±δ commutation alternation = ±26° alternating field error at low
/// speed = the half-speed trap under every climb wobble. This trims
/// the SCHEDULED delay per window polarity to cancel the OBSERVED
/// window-length alternation — closed-loop on the symptom, agnostic
/// to the electrical origin, and stronger than AM32 (which merely
/// tolerates its smaller δ at its faster operating points).
///
/// EMA halves per parity (α = 1/8 per window); trim for the window's
/// closing commutation = −(ema_this − ema_mid)/2, clamped ±80 µs,
/// active only at low speed (interval ≥ 200 µs) where the trap
/// lives. Pure state-in/state-out for host tests; the firmware keeps
/// the two EMAs in atomics.
pub const PARITY_TRIM_MIN_IV_US: u32 = 200;
pub const PARITY_TRIM_CLAMP_US: i32 = 80;

/// One window closed with length `wl_us` on an even (`parity=0`) or
/// odd sector: returns the updated EMA for that parity.
#[inline]
pub fn parity_ema_step(ema_us: u32, wl_us: u32) -> u32 {
    if ema_us == 0 {
        wl_us
    } else {
        ema_us - ema_us / 8 + wl_us / 8
    }
}

/// Delay trim (µs, signed) for a commutation closing a window of
/// this parity. Positive = commutate later.
#[inline]
pub fn parity_trim_us(ema_this: u32, ema_other: u32, interval_us: u32) -> i32 {
    if interval_us < PARITY_TRIM_MIN_IV_US || ema_this == 0 || ema_other == 0 {
        return 0;
    }
    let mid = (ema_this + ema_other) / 2;
    // Full cancellation (the /2 first cut left exactly half the
    // alternation on the bench: ±76 → ±38 µs); the α=1/8 EMA is the
    // loop damping.
    let raw = -(ema_this as i32 - mid as i32);
    raw.clamp(-PARITY_TRIM_CLAMP_US, PARITY_TRIM_CLAMP_US)
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
///
/// E4 (CONSTANTS_AUDIT 2026-07-15): under an ESTABLISHED lock
/// (cl_active, not reacq) the depth stays 5 even with the blank
/// faded — at 84-95 µs windows the 12 spaced reads cost ~1.4 µs on
/// the COMP critical path (a fifth of the ZC->shot budget) to filter
/// a BEMF that is large and clean at these speeds. Re-acquisition
/// and pre-lock keep the full 12 (noise defense where it earns it).
#[inline]
pub fn persistence_reads(blank_us: u32, cl_locked: bool) -> u32 {
    if blank_us >= 5 || cl_locked { 5 } else { 12 }
}

/// R2 — AM32's speed-mapped persistence, now VERBATIM
/// (main.c:~2463, re-read 2026-07-19):
/// `filter_level = map(average_interval, 100, 500, 3, 12)` — 3 reads
/// at 100 µs, 12 at 500+. The earlier port used a 50..250 map
/// (systematically DEEPER at every speed: 5 vs their 3 at 100 µs)
/// and forced 12 in re-acquisition — a minz invention (E2a audit),
/// not AM32. The t100flood full-resolution event autopsy convicted
/// that combination as THE dead-window root: every fatal window's
/// edges passed blank+gate and died at persistence (NOZ attribution
/// valid==raw), and after one miss the reacq deepening made the NEXT
/// window's hold 2.4× harder — a self-tightening recovery. AM32
/// never misses because its filter is shallow at speed and NEVER
/// tightens on a miss; there is no reacq-deepening to copy, so ours
/// is gone. Pre-lock at slow intervals keeps 12 (their
/// `zero_crosses<100 && commutation_interval>500` branch).
#[inline]
pub fn persistence_reads_r2(avg_interval_us: u32, cl_locked: bool, _reseeding: bool) -> u32 {
    if avg_interval_us == 0 || (!cl_locked && avg_interval_us > 500) {
        return 12;
    }
    if avg_interval_us >= 500 {
        12
    } else if avg_interval_us <= 100 {
        3
    } else {
        3 + (avg_interval_us - 100) * 9 / 400
    }
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
///
/// E2a (CONSTANTS_AUDIT 2026-07-15): in RE-ACQUISITION under an
/// active lock at the TOP END (interval < TOPEND_US), depth drops
/// back to 1 — two 41.67 µs wraps (83 µs) physically cannot complete
/// inside an 84-95 µs window before `close_float_window` clears the
/// candidate, so depth-2 reacq at these speeds was structurally
/// unable to accept ANY ZC (the NOZ-cascade generator). Depth 2 is
/// kept for reacq at lower speeds (where it fits and its premature
/// rejection is the proven configuration) and for engage/open-loop.
#[inline]
pub fn confirm_need(cl_active: bool, reacq: bool, interval_us: u32) -> u32 {
    if cl_active && !reacq {
        1
    } else if cl_active && reacq && interval_us > 0 && interval_us < crate::window::TOPEND_US {
        1
    } else {
        2
    }
}

/// R1 — FIRMWARE di/dt LIMITER (AM32 duty-governance port,
/// GAP_CLOSING_PLAN rung 1; AM32 main.c:1688-1708). AM32 shapes
/// EVERY duty write into micro-steps in its 20 kHz tick:
/// {2,6,16}/2000 counts per 50 µs by regime = 2/6/16 %/ms,
/// symmetric accel/decel. minz previously applied host throttle
/// steps as instantaneous CCR edges (a ~1 % jump in one PWM update,
/// then frozen 50 ms) — each edge bigger than anything AM32 ever
/// applies, and a prime seed for the window-position runaway.
/// Rates are expressed per-millisecond and scaled by the caller's
/// actual elapsed time, so the clamp is cadence-independent
/// (TIM7 6 kHz refresh, LPTIM2 per-commutation, open loop alike).
pub const SLEW_STARTUP_PERMILLE_PER_MS: u32 = 20; // 2 %/ms
pub const SLEW_LOW_PERMILLE_PER_MS: u32 = 60; // 6 %/ms (interval > 500 us)
pub const SLEW_HIGH_PERMILLE_PER_MS: u32 = 160; // 16 %/ms (at speed)
/// No duty application for this long = the motor was stopped/killed:
/// restart the ramp from zero (covers every kill path without
/// touching them).
pub const SLEW_STALE_US: u32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlewOut {
    pub duty: u16,
    pub clamped: bool,
}

/// One shaped duty application. `elapsed_us` = time since the last
/// application (any site). Regimes mirror AM32: startup (not locked,
/// or duty below ~7.5 % of max) 2 %/ms; low speed (interval >
/// 500 µs) 6 %/ms; high speed 16 %/ms.
#[inline]
pub fn duty_slew(
    last: u16,
    target: u16,
    elapsed_us: u32,
    max_duty: u16,
    cl_active: bool,
    interval_us: u32,
) -> SlewOut {
    let (last, elapsed_us) = if elapsed_us > SLEW_STALE_US {
        (0, 1_000) // stale: restart the ramp gently from zero
    } else {
        (last, elapsed_us)
    };
    let rate = if !cl_active || last < max_duty / 13 {
        SLEW_STARTUP_PERMILLE_PER_MS
    } else if interval_us == 0 || interval_us > 500 {
        SLEW_LOW_PERMILLE_PER_MS
    } else {
        SLEW_HIGH_PERMILLE_PER_MS
    };
    let allowed = ((max_duty as u64 * rate as u64 * elapsed_us as u64) / 1_000_000).max(1) as u16;
    if target > last {
        let step = target - last;
        if step > allowed {
            SlewOut {
                duty: last + allowed,
                clamped: true,
            }
        } else {
            SlewOut {
                duty: target,
                clamped: false,
            }
        }
    } else {
        let step = last - target;
        if step > allowed {
            SlewOut {
                duty: last - allowed,
                clamped: true,
            }
        } else {
            SlewOut {
                duty: target,
                clamped: false,
            }
        }
    }
}

/// R4 — AM32's variable_pwm carrier map (main.c:2131, converted to
/// µs): `arr = map(interval_us, 48, 100, base/2, base)`. Below 48 µs
/// the carrier is 48 kHz (arr = base/2); above 100 µs it is the base
/// 24 kHz. Between, it interpolates — ripple current shrinks exactly
/// as the windows tighten through the danger band.
#[inline]
pub fn carrier_arr(interval_us: u32, base_arr: u16) -> u16 {
    // AM32 main.c:2193: tim1_arr = map(commutation_interval, 96, 200,
    // ARR/2, ARR) — where commutation_interval is in INTERVAL_TIMER
    // TICKS and TIM2->PSC=39 (peripherals.c:442) = 0.5 µs/tick, so
    // the REAL band is 48..100 µs. UNITS SAGA (twice-flipped;
    // settled 2026-07-20 by the five-agent audit + direct source
    // read of both PSC and the map): the first cut's 48..100 µs was
    // RIGHT; the "correction" to 96..200 µs was the µs-misreading —
    // it ran the ~90 µs death band at 48 kHz where the reference
    // runs ~24-27 kHz (double EMI/ripple, half the ON-window
    // margins, in exactly the fatal regime).
    let half = base_arr / 2;
    if interval_us == 0 || interval_us >= 100 {
        base_arr
    } else if interval_us <= 48 {
        half
    } else {
        // linear: 48..100 -> half..base
        let span = (base_arr - half) as u32;
        (half as u32 + span * (interval_us - 48) / 52) as u16
    }
}

/// R4a — the sag debounce in SAMPLES for the live carrier: the
/// proven behavior is ~2.67 ms (64 samples at 24 kHz); the sample
/// rate is the PWM wrap rate, so at higher carriers the count must
/// scale or benign 1.5-2 ms dips (ridden through for weeks) start
/// killing. samples = 2670 µs / wrap_us.
#[inline]
pub fn sag_debounce_samples(arr: u16) -> u16 {
    let wrap_us_x100 = (arr as u32 + 1) * 100 / 80; // 80 MHz timer
    ((267_000 / wrap_us_x100.max(1)) as u16).max(16)
}

/// Climb lead (remaining-levers rung): during a commanded climb the
/// 3/4-smoothed interval LAGS the accelerating rotor, so delays
/// computed from it land commutations late - the late-schedule seed
/// of the transit miss cascades. While climbing, shave the
/// SCHEDULING copy of the interval by 1/16 (~6 %); the estimator,
/// gate, and telemetry keep the honest value. No-op at dwells.
#[inline]
pub fn climb_lead_iv(interval_us: u32, climbing: bool) -> u32 {
    if climbing {
        interval_us - interval_us / 16
    } else {
        interval_us
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_trim_cancels_the_measured_alternation() {
        // Feed the measured trap: even windows 592, odd 442.
        let mut ee = 0u32;
        let mut eo = 0u32;
        for _ in 0..64 {
            ee = parity_ema_step(ee, 592);
            eo = parity_ema_step(eo, 442);
        }
        assert!((580..=600).contains(&ee), "{ee}");
        assert!((430..=450).contains(&eo), "{eo}");
        // Even windows are long: their closing commutation trims
        // EARLIER (negative); odd trims later. Half the offset each.
        let te = parity_trim_us(ee, eo, 517);
        let to = parity_trim_us(eo, ee, 517);
        assert!((-80..=-60).contains(&te), "{te}");
        assert!((60..=80).contains(&to), "{to}");
        // Uniform windows: no trim.
        assert_eq!(parity_trim_us(500, 500, 517), 0);
        // High speed: off.
        assert_eq!(parity_trim_us(120, 90, 105), 0);
        // Unseeded: off.
        assert_eq!(parity_trim_us(0, 442, 517), 0);
    }

    #[test]
    fn am32_advance_is_duty_keyed_feedforward() {
        // Their map(duty, 100, 2000, 13, 23) anchor points.
        assert_eq!(auto_advance_deg_am32(5, 0), 13);
        assert_eq!(auto_advance_deg_am32(10, 0), 13); // 13.5 floor
        assert_eq!(auto_advance_deg_am32(50, 0), 17);
        assert_eq!(auto_advance_deg_am32(100, 0), 23);
        // Speed never enters — an estimate excursion can't move it.
        // Manual override still wins.
        assert_eq!(auto_advance_deg_am32(50, 8), 8);
    }

    #[test]
    fn gate_am32_is_half_the_stiff_average() {
        // The 322 Hz even/odd case: 442 µs window, stiff avg 517 —
        // gate 258 blocks the 199 µs pre-ZC dwell edge the 30 % gate
        // (133 µs) admitted.
        assert_eq!(gate_us_am32(517, 442, false), 258);
        // Stiff average unseeded: fall back to the fast interval.
        assert_eq!(gate_us_am32(0, 442, false), 221);
        // Re-acq keeps the widened 8 % gate.
        assert_eq!(gate_us_am32(517, 442, true), gate_us(442, true));
    }

    #[test]
    fn climb_lead_shaves_only_while_climbing() {
        assert_eq!(climb_lead_iv(160, false), 160);
        assert_eq!(climb_lead_iv(160, true), 150);
        assert_eq!(climb_lead_iv(96, true), 90);
        assert_eq!(climb_lead_iv(0, true), 0);
    }

    #[test]
    fn r4_carrier_map_am32_parity() {
        // AM32 main.c:2193: map(commutation_interval, 96, 200, ARR/2,
        // ARR) — in INTERVAL_TIMER TICKS (TIM2 PSC=39 = 0.5 µs/tick,
        // peripherals.c:442), so the REAL band is 48..100 µs. UNITS
        // SAGA settled 2026-07-20 (five-agent audit + source read):
        // this test previously pinned the µs-misreading (96..200 µs)
        // which ran the ~90 µs death band at 48 kHz where the
        // reference runs ~24-27 kHz. ARR 3332: ≥100 µs → 24 kHz;
        // ≤48 µs → 48 kHz; 74 µs = midpoint.
        assert_eq!(carrier_arr(0, 3332), 3332);
        assert_eq!(carrier_arr(100, 3332), 3332);
        assert_eq!(carrier_arr(200, 3332), 3332);
        assert_eq!(carrier_arr(48, 3332), 1666);
        assert_eq!(carrier_arr(30, 3332), 1666);
        let mid = carrier_arr(74, 3332);
        assert!((2470..=2530).contains(&mid), "mid {mid}");
        // The ~90 µs death band runs near the BASE carrier (24-28
        // kHz), matching the reference's measured behavior there.
        let a90 = carrier_arr(90, 3332);
        let f90 = 80_000_000 / (a90 as u32 + 1);
        assert!((24_000..=29_000).contains(&f90), "f90 {f90}");
    }

    #[test]
    fn r4a_sag_samples_scale_with_carrier() {
        // 24 kHz (ARR 3332): ~64 samples = the proven 2.67 ms.
        let s24 = sag_debounce_samples(3332);
        assert!((62..=66).contains(&s24), "{s24}");
        // 48 kHz (ARR 1666): ~128 samples = the same 2.67 ms.
        let s48 = sag_debounce_samples(1666);
        assert!((124..=132).contains(&s48), "{s48}");
    }

    #[test]
    fn r1_slew_matches_am32_transit_shape() {
        // AM32's 50->70 % at 1900 Hz: 16 counts per 50 us tick on a
        // 2000 scale, linear, ~1.25 ms to target (main.c:1688-1708).
        let (max, mut d) = (2000u16, 1005u16);
        let mut ticks = 0;
        loop {
            let o = duty_slew(d, 1403, 50, max, true, 175);
            d = o.duty;
            ticks += 1;
            if !o.clamped {
                break;
            }
        }
        assert_eq!(d, 1403);
        assert_eq!(ticks, 25); // 1.25 ms, AM32-exact
        // Symmetric decel.
        let o = duty_slew(1403, 1005, 50, max, true, 175);
        assert_eq!(o.duty, 1403 - 16);
        assert!(o.clamped);
    }

    #[test]
    fn r1_slew_regimes_and_staleness() {
        // Startup regime (not locked): 2 %/ms -> 2 counts per 50 us.
        let o = duty_slew(500, 2000, 50, 2000, false, 0);
        assert_eq!(o.duty, 502);
        // Low-duty startup even under lock.
        let o = duty_slew(100, 2000, 50, 2000, true, 175);
        assert_eq!(o.duty, 102);
        // Low-speed regime: 6 %/ms.
        let o = duty_slew(1000, 2000, 50, 2000, true, 600);
        assert_eq!(o.duty, 1006);
        // Per-commutation cadence at 1900 Hz (90 us elapsed, high):
        // 2000*160*90/1e6 = 28 counts = 1.4 % per window, AM32-like.
        let o = duty_slew(1000, 2000, 90, 2000, true, 90);
        assert_eq!(o.duty, 1028);
        // Staleness (kill/idle gap): ramp restarts gently from ZERO
        // regardless of the stale last value.
        let o = duty_slew(1700, 1700, 500_000, 2000, true, 90);
        assert!(o.duty <= 40, "stale restart jumped: {}", o.duty);
        // Tiny elapsed still moves at least 1 count (no stall).
        let o = duty_slew(10, 2000, 1, 2000, false, 0);
        assert_eq!(o.duty, 11);
    }

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
        assert_eq!(
            commutation_delay_us(200, 12, 400),
            LPTIM2_MIN_DELAY_US as u32
        );
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
        assert_eq!(persistence_reads(8, false), 5);
        assert_eq!(persistence_reads(5, false), 5);
        assert_eq!(persistence_reads(4, false), 12);
        assert_eq!(persistence_reads(0, false), 12);
        // E4: established lock keeps the shallow depth at faded blank.
        assert_eq!(persistence_reads(1, true), 5);
        assert_eq!(persistence_reads(0, true), 5);
        // R2 speed map — AM32 VERBATIM (main.c ~2463):
        // map(avg, 100, 500, 3, 12). 100 µs -> 3 (the fatal band);
        // 500 -> 12; NO reacq/reseed deepening (the t100flood
        // self-tightening-recovery conviction); pre-lock forces 12
        // only at slow intervals (their zero_crosses<100 && >500).
        assert_eq!(persistence_reads_r2(500, true, false), 12);
        assert_eq!(persistence_reads_r2(100, true, false), 3);
        assert_eq!(persistence_reads_r2(90, true, false), 3);
        assert_eq!(persistence_reads_r2(300, true, false), 7);
        // Reacq/reseed does NOT deepen — the miss-recovery must not
        // tighten the filter that caused the miss.
        assert_eq!(persistence_reads_r2(90, true, true), 3);
        // Pre-lock: 12 at slow (engage), speed map once fast.
        assert_eq!(persistence_reads_r2(700, false, false), 12);
        assert_eq!(persistence_reads_r2(90, false, false), 3);
    }

    #[test]
    fn confirm_depth_regime_scoped() {
        // CL fast path 1; engage/open-loop/re-acq demand 2 (the
        // unconditional-1-confirm engage-runaway regression).
        assert_eq!(confirm_need(true, false, 1000), 1);
        assert_eq!(confirm_need(false, false, 1000), 2);
        assert_eq!(confirm_need(true, true, 1000), 2);
        assert_eq!(confirm_need(false, true, 1000), 2);
    }

    #[test]
    fn confirm_need_reacq_topend_fits_the_window() {
        // E2a: at 84-95 us intervals only ~2.2 PWM wraps fit a window
        // and a mid-window candidate sees at most 1 — depth 2 in reacq
        // was structurally unsatisfiable (the NOZ-cascade generator).
        assert_eq!(confirm_need(true, true, 90), 1);
        assert_eq!(confirm_need(true, true, 124), 1);
        // At/above TOPEND the proven depth-2 reacq stands.
        assert_eq!(confirm_need(true, true, 125), 2);
        assert_eq!(confirm_need(true, true, 160), 2);
        // Unseeded interval (0) must not take the shortcut.
        assert_eq!(confirm_need(true, true, 0), 2);
        // Not CL-active: engage/open-loop keep full depth regardless.
        assert_eq!(confirm_need(false, true, 90), 2);
    }
}
