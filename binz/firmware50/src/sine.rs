//! The open-loop startup waveform.
//!
//! During align/catch/ramp the bridge runs a three-phase sine rather than
//! six-step commutation: there is no BEMF to sense yet, so the rotor is dragged
//! by a rotating vector whose frequency and amplitude the script schedules.
//!
//! The table and the scaling are reproduced from the reference so the startup
//! transient — which is what determines whether the rotor catches at all — is
//! the same one that was qualified.
//!
//! Division-free: the `/1000` and `/255` are reciprocal multiplies.

use crate::fixed::{div_1000, umul_hi};

/// Quarter-symmetric sine, offset to mid-scale, 256 entries.
///
/// Transcribed verbatim from the reference table; the exact rounding of each
/// entry is part of the qualified startup behavior, so it is copied rather
/// than recomputed.
pub const SINE_LUT: [u8; 256] = [
    128, 131, 134, 137, 140, 143, 146, 149, 152, 155, 158, 162, 165, 167, 170, 173, 176, 179, 182, 185, 188, 190, 193,
    196, 198, 201, 203, 206, 208, 211, 213, 215, 218, 220, 222, 224, 226, 228, 230, 232, 234, 235, 237, 238, 240, 241,
    243, 244, 245, 246, 248, 249, 250, 250, 251, 252, 253, 253, 254, 254, 254, 255, 255, 255, 255, 255, 255, 255, 254,
    254, 254, 253, 253, 252, 251, 250, 250, 249, 248, 246, 245, 244, 243, 241, 240, 238, 237, 235, 234, 232, 230, 228,
    226, 224, 222, 220, 218, 215, 213, 211, 208, 206, 203, 201, 198, 196, 193, 190, 188, 185, 182, 179, 176, 173, 170,
    167, 165, 162, 158, 155, 152, 149, 146, 143, 140, 137, 134, 131, 128, 124, 121, 118, 115, 112, 109, 106, 103, 100,
    97, 93, 90, 88, 85, 82, 79, 76, 73, 70, 67, 65, 62, 59, 57, 54, 52, 49, 47, 44, 42, 40, 37, 35, 33, 31, 29, 27, 25,
    23, 21, 20, 18, 17, 15, 14, 12, 11, 10, 9, 7, 6, 5, 5, 4, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3,
    4, 5, 5, 6, 7, 9, 10, 11, 12, 14, 15, 17, 18, 20, 21, 23, 25, 27, 29, 31, 33, 35, 37, 40, 42, 44, 47, 49, 52, 54,
    57, 59, 62, 65, 67, 70, 73, 76, 79, 82, 85, 88, 90, 93, 97, 100, 103, 106, 109, 112, 115, 118, 121, 124,
];

/// Phase separation between the three windings, in table entries.
/// `256 / 3 = 85.33`, and the reference uses 85.
pub const PHASE_STEP: u32 = 85;

/// `ceil(2^32 / 255)`, exact with `umul_hi` for inputs up to `2^32 / 254`.
pub const RECIP_255: u32 = 16_843_010;

/// Exact `x / 255` without division, for `x <= 16_900_000`.
#[inline]
pub const fn div_255(x: u32) -> u32 {
    umul_hi(x, RECIP_255)
}

/// Peak compare value for a given period and duty.
///
/// `amplitude = (arr + 1) * duty_tenths / 1000`, i.e. the duty applied to the
/// full period, which the sine then modulates down from.
#[inline]
pub const fn amplitude(arr: u32, duty_tenths: u32) -> u32 {
    div_1000((arr + 1).saturating_mul(duty_tenths))
}

/// Scale one table entry by the amplitude: `compare = amplitude * sine / 255`.
#[inline]
pub const fn compare(amplitude: u32, sine: u8) -> u32 {
    div_255(amplitude.saturating_mul(sine as u32))
}

/// The three table indices for a phase accumulator, 120 degrees apart.
///
/// Only the top 8 bits of `theta` select an entry, so the accumulator can be
/// advanced by any increment without a modulo.
#[inline]
pub const fn indices(theta: u32) -> [usize; 3] {
    let base = theta >> 24;
    [
        (base & 0xFF) as usize,
        ((base + PHASE_STEP) & 0xFF) as usize,
        ((base + 2 * PHASE_STEP) & 0xFF) as usize,
    ]
}

/// The three compare values for a phase accumulator and duty.
///
/// Returned in logical phase order `[A, B, C]`; the caller maps those onto
/// `CCR3`/`CCR2`/`CCR1` respectively.
#[inline]
pub fn compares(arr: u32, duty_tenths: u32, theta: u32) -> [u32; 3] {
    let a = amplitude(arr, duty_tenths);
    let ix = indices(theta);
    // Indexing is bounds-proof: `indices` masks to 0..=255 and the table is
    // exactly 256 long, so no panic path exists here.
    [
        compare(a, SINE_LUT[ix[0]]),
        compare(a, SINE_LUT[ix[1]]),
        compare(a, SINE_LUT[ix[2]]),
    ]
}

/// `2^32 / 100_000`, split so the phase increment needs no division.
///
/// A full electrical cycle is one wrap of the accumulator. At `freq_chz`
/// centihertz and `CONTROL_HZ` ticks per second the per-tick increment is
/// `2^32 * freq_chz / (100 * CONTROL_HZ)`; with `CONTROL_HZ = 1000` that
/// denominator is 100_000 and `2^32 / 100_000 = 42949.67296`.
const INC_WHOLE: u32 = 42_949;
const INC_FRAC_NUM: u32 = 673;

/// Phase-accumulator increment per control tick for a frequency in centihertz.
///
/// Accurate to 1 LSB of the accumulator over the whole admissible frequency
/// range, which is far finer than the motor can resolve.
#[inline]
pub const fn theta_increment(freq_chz: u32) -> u32 {
    let whole = freq_chz.saturating_mul(INC_WHOLE);
    let frac = div_1000(freq_chz.saturating_mul(INC_FRAC_NUM));
    whole.saturating_add(frac)
}

/// On-time in microseconds for a compare value at the 10 kHz startup carrier,
/// for telemetry only: `(ccr + 32) >> 6`.
#[inline]
pub const fn on_us_10k(ccr: u32) -> u32 {
    (ccr + 32) >> 6
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARR: u32 = 6399;

    // --- the table ---------------------------------------------------------

    #[test]
    fn the_table_is_a_full_period_centered_on_mid_scale() {
        assert_eq!(SINE_LUT.len(), 256);
        assert_eq!(SINE_LUT[0], 128);
        assert_eq!(SINE_LUT[128], 128, "half way through is back at center");
        assert_eq!(*SINE_LUT.iter().max().unwrap(), 255);
        assert_eq!(*SINE_LUT.iter().min().unwrap(), 0);
    }

    #[test]
    fn the_table_peaks_and_troughs_are_a_half_period_apart() {
        let peak = SINE_LUT.iter().position(|v| *v == 255).unwrap();
        let trough = SINE_LUT.iter().position(|v| *v == 0).unwrap();
        assert!(trough > peak, "trough must follow the peak");
        // both plateaus are a handful of entries wide; their centers are ~128 apart
        assert!(
            (110..=150).contains(&(trough - peak)),
            "peak at {peak}, trough at {trough}"
        );
    }

    #[test]
    fn the_table_is_monotone_between_its_extremes() {
        // Rising from 0 to the peak plateau, then falling. A transcription
        // error in the middle of the table would break this and nothing else.
        let peak = SINE_LUT.iter().position(|v| *v == 255).unwrap();
        for i in 1..peak {
            assert!(SINE_LUT[i] >= SINE_LUT[i - 1], "not rising at {i}");
        }
        let trough = SINE_LUT.iter().position(|v| *v == 0).unwrap();
        let last_peak = SINE_LUT.iter().rposition(|v| *v == 255).unwrap();
        for i in (last_peak + 1)..=trough {
            assert!(SINE_LUT[i] <= SINE_LUT[i - 1], "not falling at {i}");
        }
    }

    // --- division by 255 ---------------------------------------------------

    #[test]
    fn div_255_is_exact_over_every_reachable_product() {
        // The largest product is amplitude(6399, 1000) * 255.
        let max = amplitude(ARR, 1000) * 255;
        assert!(max < 16_900_000, "product {max} must be inside the exact range");
        for x in 0..=max {
            if div_255(x) != x / 255 {
                panic!("div_255({x}) = {} want {}", div_255(x), x / 255);
            }
        }
    }

    // --- amplitude and compare ---------------------------------------------

    #[test]
    fn amplitude_is_the_duty_applied_to_the_full_period() {
        assert_eq!(amplitude(ARR, 1000), 6400, "100% of the period");
        assert_eq!(amplitude(ARR, 100), 640, "10%");
        assert_eq!(amplitude(ARR, 70), 448, "the catch duty");
        assert_eq!(amplitude(ARR, 10), 64, "the align duty");
        assert_eq!(amplitude(ARR, 0), 0);
    }

    #[test]
    fn amplitude_matches_the_reference_expression_exactly() {
        for duty in 0u32..=1000 {
            assert_eq!(amplitude(ARR, duty), (ARR + 1) * duty / 1000, "duty {duty}");
        }
    }

    #[test]
    fn compare_matches_the_reference_two_step_floor() {
        // ccr = floor(floor((arr+1)*duty/1000) * sine / 255)
        for duty in [10u32, 70, 100, 250, 300] {
            let a = amplitude(ARR, duty);
            for (i, s) in SINE_LUT.iter().enumerate() {
                let want = a * (*s as u32) / 255;
                assert_eq!(compare(a, *s), want, "duty {duty} index {i}");
            }
        }
    }

    #[test]
    fn a_compare_never_exceeds_the_period() {
        // A compare above ARR would be a permanently-on leg.
        for duty in 0u32..=1000 {
            for s in SINE_LUT {
                assert!(compare(amplitude(ARR, duty), s) <= ARR + 1, "duty {duty}");
            }
        }
    }

    #[test]
    fn the_ten_percent_peak_compare_is_a_tenth_of_the_period() {
        let a = amplitude(ARR, 100);
        assert_eq!(compare(a, 255), 640);
        assert_eq!(compare(a, 128), 321);
        assert_eq!(compare(a, 0), 0);
    }

    // --- phase indices -----------------------------------------------------

    #[test]
    fn the_three_indices_are_a_third_of_a_turn_apart() {
        let ix = indices(0);
        assert_eq!(ix, [0, 85, 170]);
        let ix = indices(0x8000_0000);
        assert_eq!(ix, [128, (128 + 85) & 0xFF, (128 + 170) & 0xFF]);
    }

    #[test]
    fn indices_are_always_in_bounds_for_any_accumulator_value() {
        // This is what makes `compares` panic-free.
        let mut theta: u32 = 0;
        for _ in 0..10_000 {
            for i in indices(theta) {
                assert!(i < SINE_LUT.len());
            }
            theta = theta.wrapping_add(0x00A3_5F17);
        }
        for t in [0u32, 1, u32::MAX, 0x00FF_FFFF, 0xFF00_0000] {
            for i in indices(t) {
                assert!(i < SINE_LUT.len(), "theta {t}");
            }
        }
    }

    #[test]
    fn the_three_phases_sum_to_roughly_a_constant() {
        // A genuine three-phase set has a near-constant sum; a phase-offset
        // error shows up here immediately.
        let a = amplitude(ARR, 100);
        let mut theta: u32 = 0;
        let mut min = u32::MAX;
        let mut max = 0;
        for _ in 0..256 {
            let ix = indices(theta);
            let sum: u32 = ix.iter().map(|i| compare(a, SINE_LUT[*i])).sum();
            min = min.min(sum);
            max = max.max(sum);
            theta = theta.wrapping_add(1 << 24);
        }
        // Three 120-degree-spaced sines on a mid-scale offset sum to 3*offset;
        // the 85/256 rounding leaves a small ripple.
        let ripple = max - min;
        assert!(ripple * 20 < max, "sum ripple {ripple} too large for max {max}");
    }

    // --- frequency ---------------------------------------------------------

    #[test]
    fn the_phase_increment_matches_the_exact_ratio() {
        for chz in [0u32, 1, 100, 5_000, 10_000, 25_000] {
            let want = ((1u64 << 32) * chz as u64 / 100_000) as u32;
            let got = theta_increment(chz);
            assert!(got.abs_diff(want) <= 1, "chz {chz}: got {got}, want {want}");
        }
    }

    #[test]
    fn the_catch_frequency_completes_a_cycle_in_the_expected_time() {
        // 100.0 eHz at 1000 ticks/s is one electrical cycle per 10 ticks.
        let inc = theta_increment(10_000);
        let mut theta: u32 = 0;
        let mut ticks = 0;
        loop {
            let next = theta.wrapping_add(inc);
            ticks += 1;
            if next < theta {
                break; // wrapped: one full electrical cycle
            }
            theta = next;
            assert!(ticks < 100, "did not complete a cycle");
        }
        assert_eq!(ticks, 10, "100 eHz at 1 kHz control rate");
    }

    #[test]
    fn a_zero_frequency_holds_a_static_vector() {
        // This is what the align stage relies on.
        assert_eq!(theta_increment(0), 0);
    }

    #[test]
    fn the_increment_cannot_overflow_at_the_frequency_ceiling() {
        let inc = theta_increment(crate::startup::MAX_EHZ * 100);
        // 250 eHz at 1 kHz is a quarter turn per tick
        assert!(inc < u32::MAX / 3, "increment {inc} implausibly large");
        let _ = theta_increment(u32::MAX);
    }

    #[test]
    fn on_time_telemetry_tracks_the_compare() {
        assert_eq!(on_us_10k(0), 0);
        assert_eq!(on_us_10k(640), 10, "10% of a 100 us period");
        assert_eq!(on_us_10k(6400), 100, "full period");
    }
}
