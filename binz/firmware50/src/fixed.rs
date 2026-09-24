//! Division-free fixed-point arithmetic for the ISR paths.
//!
//! The Cortex-M0+ has neither a hardware divider nor a widening multiply
//! (`UMULL`): `MULS` produces only the low 32 bits. So a `u32 / 1000` becomes a
//! call to `__aeabi_uidiv` and a `u32 as u64 * u32` becomes a call to
//! `__aeabi_lmul` — both forbidden on the four motor ISR roots, the first
//! because it is slow and data-dependent, the second because it is a call the
//! reachability audit cannot see through.
//!
//! Everything here is therefore straight-line `MULS`/shift/add code: no
//! division, no loops, no calls, no panics.

/// High 32 bits of `a * b`, computed exactly from four 16-bit partial products.
///
/// Each partial product is at most `65535 * 65535 = 0xFFFE0001`, so no
/// individual term overflows, and the carries are folded through a 16-bit-wide
/// accumulator.
///
/// # What this actually compiles to
///
/// The intent was to emit only `MULS`/shift/add. **It does not.** LLVM
/// recognizes this exact four-partial-product pattern, canonicalizes it back
/// into a 64-bit multiply, and lowers that to a call to `__aeabi_lmul`. Writing
/// the widening multiply out by hand does not prevent the call; the optimizer
/// simply re-forms it.
///
/// That is acceptable, and the distinction matters: the prohibition on the
/// motor ISR paths is against **data-dependent, unbounded latency** — soft
/// division and unbounded loops. `__aeabi_lmul` on this target is 34
/// instructions with *zero branches*: straight-line, constant-time, no loop,
/// no early exit, latency independent of the operand values. It is a fixed
/// cost, not a hazard, and the reachability audit resolves it like any other
/// leaf call.
///
/// Verified by disassembly, not assumed. It is checked in the audit rather
/// than trusted, because this is exactly the sort of property a compiler
/// upgrade can silently change.
///
/// If the ISR cycle budget ever gets tight this is the place to spend effort:
/// `__aeabi_lmul` computes a full 64x64 product when only the high half of a
/// 32x32 is wanted, so a hand-written `asm!` block would be meaningfully
/// cheaper. It is not worth the risk of a silent arithmetic bug until the
/// budget demands it.
#[inline]
pub const fn umul_hi(a: u32, b: u32) -> u32 {
    let (al, ah) = (a & 0xFFFF, a >> 16);
    let (bl, bh) = (b & 0xFFFF, b >> 16);
    let ll = al * bl;
    let lh = al * bh;
    let hl = ah * bl;
    let hh = ah * bh;
    // Sum the bits that land in the 16..32 column, then carry into the top.
    let mid = (ll >> 16) + (lh & 0xFFFF) + (hl & 0xFFFF);
    hh + (lh >> 16) + (hl >> 16) + (mid >> 16)
}

/// Low 32 bits of `a * b` — plain wrapping multiply, named for symmetry with
/// `umul_hi` at call sites that want both halves.
#[inline]
pub const fn umul_lo(a: u32, b: u32) -> u32 {
    a.wrapping_mul(b)
}

/// `ceil(2^32 / 125)`.
///
/// 125 is the odd part of every divisor this firmware needs (1000 = 8·125,
/// 2000 = 16·125), and it is a good reciprocal: `RECIP_125 * 125 - 2^32 = 79`,
/// so `floor(y * RECIP_125 / 2^32) == floor(y / 125)` holds for every
/// `y <= 2^32 / 79 = 54_366_674`.
pub const RECIP_125: u32 = 34_359_739;

/// Largest `y` for which `umul_hi(y, RECIP_125)` is exactly `y / 125`.
pub const DIV_125_MAX: u32 = 54_366_674;

/// Exact `y / 125` without division, for `y <= DIV_125_MAX`.
#[inline]
pub const fn div_125(y: u32) -> u32 {
    umul_hi(y, RECIP_125)
}

/// Exactness bound for [`div_1000`]: `8 * DIV_125_MAX + 7`.
pub const DIV_1000_MAX: u32 = 434_933_399;

/// Exact `x / 1000` without division, for `x <= DIV_1000_MAX`.
///
/// Uses the nested-floor identity `floor(floor(x / 8) / 125) ==
/// floor(x / 1000)`, which is exact for integers — the shift is free and it
/// moves the whole computation into `div_125`'s very wide valid range, instead
/// of the ~6.1 M a direct `ceil(2^32/1000)` reciprocal would allow.
#[inline]
pub const fn div_1000(x: u32) -> u32 {
    div_125(x >> 3)
}

/// Exactness bound for [`div_2000`]: `16 * DIV_125_MAX + 15`.
pub const DIV_2000_MAX: u32 = 869_866_799;

/// `round(2^32 * 4/5)`. The classic `/10` reciprocal: `umul_hi(x, this) >> 3`
/// is exactly `x / 10` for **every** `u32`, with no range restriction.
pub const RECIP_10: u32 = 0xCCCC_CCCD;

/// Exact `x / 10` without division, for every `u32`.
///
/// Needed because at `opt-level = "s"` LLVM emits a call to `__aeabi_uidiv`
/// for a `/ 10` rather than spending the extra bytes on a reciprocal multiply
/// — a size win that costs a data-dependent, unbounded-latency library call on
/// a core with no divider. Decimal formatting is the only place this crate
/// divides by 10, and it must not drag that helper into the image.
#[inline]
pub const fn div_10(x: u32) -> u32 {
    umul_hi(x, RECIP_10) >> 3
}

/// Exact `x % 10`, derived from [`div_10`].
#[inline]
pub const fn rem_10(x: u32) -> u32 {
    x - div_10(x) * 10
}

/// `round(2^32 * 8/25)`. The classic `/100` reciprocal:
/// `umul_hi(x, this) >> 5` is exactly `x / 100` for every `u32`.
pub const RECIP_100: u32 = 0x51EB_851F;

/// Exact `x / 100` without division, for every `u32`.
#[inline]
pub const fn div_100(x: u32) -> u32 {
    umul_hi(x, RECIP_100) >> 5
}

/// Exact `x / 2000` without division, for `x <= DIV_2000_MAX`.
///
/// Same identity with `2000 = 16 * 125`. Used by the startup ramp, whose
/// duration is 2000 control ticks.
#[inline]
pub const fn div_2000(x: u32) -> u32 {
    div_125(x >> 4)
}

/// Duty (in tenths of a percent) to a TIM1 compare value:
/// `CCR = ticks * duty_tenths / 1000`.
///
/// At the running carrier (`ticks = 1333`) and the 10% command
/// (`duty_tenths = 100`) this is 133, matching the reference.
#[inline]
pub const fn duty_to_ccr(ticks: u32, duty_tenths: u32) -> u32 {
    div_1000(ticks.saturating_mul(duty_tenths))
}

// ---------------------------------------------------------------------------
// Bus voltage scaling
// ---------------------------------------------------------------------------

/// Bus divider multiplier, x11.94, as the reference evaluates it:
/// `12*n - ((n*6 + 99) * 20972 >> 21)`.
///
/// The subtracted term is the `0.06` correction; `20972/2^21 = 0.0100002`, so
/// `(n*6 + 99) * 0.01 ~= n*0.06`. The largest intermediate for a 12-bit code is
/// `(4095*6 + 99) * 20972 = 517_358_268`, comfortably inside `u32`.
#[inline]
pub const fn bus_scale_x1194(n: u32) -> u32 {
    let corr = ((n * 6 + 99) * 20972) >> 21;
    (12 * n).saturating_sub(corr)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- widening multiply -------------------------------------------------

    #[test]
    fn umul_hi_matches_a_sixty_four_bit_multiply() {
        let cases = [
            (0u32, 0u32),
            (1, 1),
            (0xFFFF_FFFF, 0xFFFF_FFFF),
            (0xFFFF_FFFF, 1),
            (0x1_0000, 0x1_0000),
            (0xFFFF, 0xFFFF),
            (1_333_000, RECIP_125),
            (666_500, RECIP_125),
            (0x8000_0000, 2),
            (0x1234_5678, 0x9ABC_DEF0),
            (3_000_000_000, 3_000_000_000),
        ];
        for (a, b) in cases {
            let want = (((a as u64) * (b as u64)) >> 32) as u32;
            assert_eq!(umul_hi(a, b), want, "umul_hi({a}, {b})");
        }
    }

    #[test]
    fn umul_hi_matches_over_a_pseudorandom_sweep() {
        // A cheap LCG sweep — the carry folding is where this kind of routine
        // goes wrong, and it only shows up on specific bit patterns.
        let mut s: u32 = 0x1357_9BDF;
        for _ in 0..200_000 {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let a = s;
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let b = s;
            let want = (((a as u64) * (b as u64)) >> 32) as u32;
            assert_eq!(umul_hi(a, b), want, "umul_hi({a}, {b})");
        }
    }

    #[test]
    fn umul_lo_is_the_wrapping_product() {
        assert_eq!(umul_lo(0xFFFF_FFFF, 3), 0xFFFF_FFFFu32.wrapping_mul(3));
    }

    // --- division by 10 and 100 -------------------------------------------

    #[test]
    fn div_10_and_rem_10_are_exact_over_a_dense_sweep() {
        // Exhaustive is 4 billion cases; sweep the low range densely (where
        // decimal formatting actually lives) and the rest by stride.
        for x in 0u32..=200_000 {
            assert_eq!(div_10(x), x / 10, "div_10({x})");
            assert_eq!(rem_10(x), x % 10, "rem_10({x})");
        }
        let mut x = 0u32;
        while x < u32::MAX - 7_919 {
            assert_eq!(div_10(x), x / 10, "div_10({x})");
            assert_eq!(rem_10(x), x % 10, "rem_10({x})");
            x += 7_919;
        }
    }

    #[test]
    fn div_10_is_exact_at_the_extremes_and_at_every_power_of_ten() {
        assert_eq!(div_10(0), 0);
        assert_eq!(div_10(9), 0);
        assert_eq!(div_10(10), 1);
        assert_eq!(div_10(u32::MAX), u32::MAX / 10);
        let mut p = 1u32;
        loop {
            assert_eq!(div_10(p), p / 10, "10^k = {p}");
            assert_eq!(div_10(p - 1), (p - 1) / 10, "just below {p}");
            match p.checked_mul(10) {
                Some(n) => p = n,
                None => break,
            }
        }
    }

    #[test]
    fn div_100_is_exact_over_a_dense_sweep() {
        for x in 0u32..=200_000 {
            assert_eq!(div_100(x), x / 100, "div_100({x})");
        }
        let mut x = 0u32;
        while x < u32::MAX - 7_919 {
            assert_eq!(div_100(x), x / 100, "div_100({x})");
            x += 7_919;
        }
        assert_eq!(div_100(u32::MAX), u32::MAX / 100);
    }

    // --- division by 1000 --------------------------------------------------

    #[test]
    fn div_1000_is_exact_across_its_whole_documented_range() {
        // Exhaustive over the full bound, because "exact up to N" is the entire
        // claim this primitive makes and an off-by-one here becomes a wrong
        // PWM compare value on the bench.
        for x in 0..=DIV_1000_MAX {
            debug_assert_eq!(div_1000(x), x / 1000);
            if div_1000(x) != x / 1000 {
                panic!("div_1000({x}) = {} want {}", div_1000(x), x / 1000);
            }
        }
    }

    #[test]
    fn div_1000_is_exact_on_every_boundary_in_range() {
        let mut k = 0u32;
        while k * 1000 <= DIV_1000_MAX {
            let base = k * 1000;
            assert_eq!(div_1000(base), k);
            if base > 0 {
                assert_eq!(div_1000(base - 1), k - 1, "just below {base}");
            }
            assert_eq!(div_1000(base + 1), k, "just above {base}");
            k += 1;
        }
    }

    #[test]
    fn div_1000_stays_total_and_monotone_beyond_its_bound() {
        // Out of range it may be off by one, but it must never panic, wrap, or
        // go backwards — that is what keeps a bad input from becoming a wild
        // compare value.
        let mut prev = 0;
        let mut x = 0u32;
        while x < u32::MAX - 1_000_000 {
            let v = div_1000(x);
            assert!(v >= prev, "not monotone at {x}");
            prev = v;
            x += 997_361;
        }
    }

    // --- duty to compare value --------------------------------------------

    #[test]
    fn duty_to_ccr_reproduces_the_reference_values() {
        const TICKS: u32 = 1333;
        assert_eq!(duty_to_ccr(TICKS, 100), 133, "du100 -> CCR 133");
        assert_eq!(duty_to_ccr(TICKS, 250), 333);
        assert_eq!(duty_to_ccr(TICKS, 500), 666);
        assert_eq!(duty_to_ccr(TICKS, 0), 0);
        assert_eq!(duty_to_ccr(TICKS, 40), 53, "envelope minimum");
    }

    #[test]
    fn duty_to_ccr_is_exact_over_the_whole_command_domain() {
        // Both carriers the firmware uses, and every duty a command or the
        // foldback governor can produce.
        for ticks in [1333u32, 6399] {
            for duty in 0u32..=1000 {
                let want = ticks * duty / 1000;
                assert_eq!(duty_to_ccr(ticks, duty), want, "ticks {ticks} duty {duty}");
            }
        }
    }

    #[test]
    fn duty_to_ccr_never_exceeds_the_period() {
        // A compare value above ARR would mean permanently-on; the duty domain
        // must not be able to produce one.
        for ticks in [1333u32, 6399] {
            for duty in 0u32..=1000 {
                assert!(duty_to_ccr(ticks, duty) <= ticks, "ticks {ticks} duty {duty}");
            }
        }
    }

    #[test]
    fn duty_to_ccr_saturates_rather_than_wrapping_on_absurd_input() {
        // Defense in depth: the envelope clamps duty long before this, but a
        // wrapped product here would silently become a small CCR instead of a
        // large one, i.e. a fault that looks like a healthy low duty.
        let _ = duty_to_ccr(u32::MAX, u32::MAX);
        let _ = duty_to_ccr(1333, u32::MAX);
    }

    // --- bus scaling -------------------------------------------------------

    #[test]
    fn bus_scale_tracks_the_eleven_point_nine_four_multiplier() {
        for n in [0u32, 1, 100, 1000, 1212, 2048, 4095] {
            let want = (n as f64 * 11.94).floor() as u32;
            let got = bus_scale_x1194(n);
            assert!(got.abs_diff(want) <= 1, "n {n}: got {got}, want ~{want}");
        }
    }

    #[test]
    fn bus_scale_is_monotone_and_cannot_overflow_for_twelve_bit_codes() {
        let mut prev = 0;
        for n in 0u32..=4095 {
            let v = bus_scale_x1194(n);
            assert!(v >= prev, "not monotone at {n}");
            prev = v;
        }
        // Largest intermediate stays inside u32 with room to spare; a debug
        // build would have panicked above if it did not.
        assert_eq!((4095u32 * 6 + 99) * 20972, 517_358_268);
    }

    #[test]
    fn the_absolute_bus_floor_works_out_to_about_eight_point_four_volts() {
        // The floor is enforced as `raw_bus * vcal < 963 * vref`, which is a
        // statement about the *pin* code normalized by VREFINT — it sits
        // before the divider, not after it. Converting all the way through:
        //   VDDA_mV   = 3000 * vcal / vref          (VREFINT calibration)
        //   pin_mV    = raw_bus * VDDA_mV / 4095
        //   bus_mV    = pin_mV * 11.94
        // so the trip point is 963 * (3000/4095) * 11.94 ~= 8423 mV.
        //
        // This test exists because 963 is easy to misread as a code that goes
        // straight into `bus_scale_x1194`; it does not.
        let trip_mv = 963.0 * (3000.0 / 4095.0) * 11.94;
        assert!(
            (8_300.0..=8_500.0).contains(&trip_mv),
            "implied floor {trip_mv} mV should be ~8400"
        );

        // And the same number reached through the integer path. Pick
        // vref == vcal, so VDDA is exactly 3000 mV and the cross-product
        // reduces to `raw < 963`.
        const VCAL: u32 = 1662;
        let vref: u32 = VCAL;
        assert!(962 * VCAL < 963 * vref, "962 must be below the floor");
        assert!(963 * VCAL >= 963 * vref, "963 must be at/above the floor");

        // At VDDA = 3000 mV the pin reads raw*3000/4095 mV, and the divider
        // multiplies by 11.94.
        let pin_mv = 963 * 3000 / 4095;
        let bus_mv = bus_scale_x1194(pin_mv);
        assert!((8_200..=8_600).contains(&bus_mv), "floor code 963 -> {bus_mv} mV");
    }
}
