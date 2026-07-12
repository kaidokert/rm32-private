//! Zero-cross discrimination pieces (seed of the future full ZC
//! candidate/confirm state machine — roadmap E1). Pure arithmetic
//! extracted from the TIM1_UP confirm path.

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
