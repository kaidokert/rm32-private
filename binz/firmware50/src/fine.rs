//! The shared diagnostic timeline's units and its wrap-safe arithmetic.
//!
//! The *counter* lives in `hw::fine` (TIM2, target-only). Everything here is
//! pure arithmetic, so it sits on the host side of the `target_os = "none"`
//! gate and is unit-tested — which is the point: the campaign's brief asks for
//! wrap-safe subtraction and pairing to be *tested*, and arithmetic that only
//! exists inside a target-only module cannot be.
//!
//! **8 MHz, 125 ns per tick** (campaign 11). E180 ran TIM2 free at 64 MHz for
//! 15.625 ns ticks, and that was wrong for this campaign in one specific way:
//! its 32-bit counter wraps at **67.11 s** while runs are commanded
//! `total_ms = 80000`, so a 32-bit "run anchor" wrapped *inside a run*. At
//! 8 MHz the wrap is **536.87 s** — 6.7× the longest run — and the **16-bit**
//! stamps that `chain::Beat` and `sagtrace::Block` store span **8.192 ms**
//! instead of 1.024 ms, which a 71 µs sector at 55% needs.
//!
//! Two rules this module exists to keep:
//!
//! * **Division-free.** 8 MHz is 2³ MHz, so ticks→µs is a shift by three. The
//!   ISR arithmetic audit forbids the division helper, and `opt-level="s"` on
//!   this M0+ turns even a constant `/10` into `__aeabi_uidiv`.
//! * **Modular subtraction, always.** A free-running counter read twice must be
//!   subtracted with `wrapping_sub`; the naive difference goes negative across
//!   the wrap. This bench has already been bitten by a software-extended clock
//!   using the wrong modulus, which manufactured 1010 µs of phantom time per
//!   wrap and fired protections that were correct about a lying clock.

/// Ticks per second: 64 MHz core / (PSC + 1).
pub const FINE_HZ: u32 = 8_000_000;

/// The prescaler that produces [`FINE_HZ`]. 64/8 = 8, so PSC = 7.
pub const FINE_PSC: u16 = 7;

const _: () = assert!(64_000_000 / (FINE_PSC as u32 + 1) == FINE_HZ);
/// Division-free conversion requires a power-of-two MHz.
const _: () = assert!((FINE_HZ / 1_000_000).is_power_of_two());

/// Shift that converts ticks to whole µs, derived from [`FINE_HZ`] rather than
/// written down.
pub const US_SHIFT: u32 = (FINE_HZ / 1_000_000).trailing_zeros();

/// Ticks to whole µs: a shift, never a division.
#[inline(always)]
#[must_use]
pub const fn to_us(ticks: u32) -> u32 {
    ticks >> US_SHIFT
}

/// Ticks to tenths of a µs, for the host's own arithmetic. `t * 10` stays
/// inside `u32` for any delta below 53.7 s at this rate, which every recorded
/// delta is far inside — the widest ring spans 36 ms.
#[inline(always)]
#[must_use]
pub const fn to_tenths(ticks: u32) -> u32 {
    (ticks * 10) >> US_SHIFT
}

/// **Wrap-safe elapsed ticks** between two reads of the 32-bit counter.
///
/// Correct for any true interval below the 536.87 s modulus, which every
/// interval this firmware measures is.
#[inline(always)]
#[must_use]
pub const fn since(earlier: u32, later: u32) -> u32 {
    later.wrapping_sub(earlier)
}

/// **Wrap-safe elapsed ticks for the 16-bit stamps** that `chain::Beat` and
/// `sagtrace::Block` keep.
///
/// Correct for any true interval below [`SPAN16_US`]; a longer gap **aliases**,
/// and the host must not pair two rows across one. That is a real limit, not a
/// formality: at 15.625 ns the span was 1.024 ms, only 14 sectors at 55%.
#[inline(always)]
#[must_use]
pub const fn since16(earlier: u16, later: u16) -> u16 {
    later.wrapping_sub(earlier)
}

/// The span a 16-bit stamp can represent, in µs: the pairing limit.
pub const SPAN16_US: u32 = to_us(0x1_0000);

/// The span of the full 32-bit counter, in whole seconds.
pub const SPAN32_S: u32 = to_us(u32::MAX) / 1_000_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_is_eight_megahertz_and_division_free() {
        assert_eq!(FINE_HZ, 8_000_000);
        assert_eq!(FINE_PSC, 7);
        assert_eq!(US_SHIFT, 3, "8 MHz = 2^3 MHz, so ticks to us is >>3");
        assert_eq!(to_us(8), 1, "8 ticks is one microsecond");
        assert_eq!(to_us(7), 0, "and it truncates rather than rounding");
        assert_eq!(to_tenths(8), 10);
        assert_eq!(to_tenths(1), 1, "one tick is 0.125 us, reported as 1 tenth");
    }

    /// The defect that motivated the rate change: a 32-bit anchor must outlast
    /// a run, and at 64 MHz it did not.
    #[test]
    fn the_thirty_two_bit_anchor_outlasts_the_longest_run() {
        assert_eq!(SPAN32_S, 536, "536.87 s at 125 ns");
        let longest_run_s = 80; // `total_ms = 80000`, the campaign's script
        assert!(
            SPAN32_S > longest_run_s * 6,
            "the anchor must not wrap inside a run, with room to spare"
        );
        // What it was before, for the record: 2^32 ticks at 15.625 ns is
        // 67.1 s, which is *less* than an 80 s run -- the defect. Ticks are
        // counted in picoseconds here (15.625 ns = 15 625 ps), so the divisor
        // is 1e12 and not 1e9; getting that wrong the first time is what this
        // assertion is for.
        let old_span_s = u64::from(u32::MAX) * 15_625 / 1_000_000_000_000;
        assert_eq!(old_span_s, 67, "2^32 at 15.625 ns is 67.1 s");
        assert!(
            old_span_s < u64::from(longest_run_s),
            "the old rate's anchor wrapped inside a run: that is why the rate changed"
        );
    }

    #[test]
    fn the_sixteen_bit_span_covers_many_sectors() {
        assert_eq!(SPAN16_US, 8192);
        let sector_us_at_55pct = 71;
        assert!(
            SPAN16_US / sector_us_at_55pct > 100,
            "a 16-bit delta must span many sectors, not a handful"
        );
    }

    #[test]
    fn subtraction_is_modular_across_the_thirty_two_bit_wrap() {
        // Straddling the wrap: 100 ticks before it to 50 after.
        let earlier = u32::MAX - 99;
        let later = 50u32;
        assert_eq!(since(earlier, later), 150);
        // And the ordinary case is unchanged.
        assert_eq!(since(1000, 1150), 150);
        // Zero-length, and the full modulus reading as zero.
        assert_eq!(since(42, 42), 0);
        assert_eq!(since(0, 0), 0);
    }

    #[test]
    fn subtraction_is_modular_across_the_sixteen_bit_wrap() {
        assert_eq!(since16(u16::MAX - 9, 5), 15);
        assert_eq!(since16(1000, 1150), 150);
        assert_eq!(since16(7, 7), 0);
    }

    /// The pairing rule the host must honour: a gap at or beyond the 16-bit
    /// span is indistinguishable from a short one, so rows must not be paired
    /// across it. This test states the aliasing rather than hiding it.
    #[test]
    fn sixteen_bit_deltas_alias_beyond_their_span() {
        let span_ticks = 0x1_0000u32;
        let true_gap = span_ticks + 200; // just past one wrap
        let earlier = 1000u16;
        let later = (u32::from(earlier).wrapping_add(true_gap)) as u16;
        // The delta reads as 200 ticks, not span+200 -- the information is gone.
        assert_eq!(u32::from(since16(earlier, later)), 200);
        assert!(to_us(true_gap) > SPAN16_US, "the true gap exceeded the span");
    }

    /// Pairing an accept with the commutation it scheduled is the one thing
    /// the coarse µs clock was kept for, and the fine delta must agree with it
    /// to within a tick's truncation.
    #[test]
    fn a_fine_delta_agrees_with_the_coarse_wait_it_scheduled() {
        let wait_us = 33u32; // a plausible `wait_time` at this rung
        let crossing = 0xFFF0u16; // deliberately just before a 16-bit wrap
        let armed = (u32::from(crossing).wrapping_add(wait_us * 8)) as u16;
        let ticks = u32::from(since16(crossing, armed));
        assert_eq!(to_us(ticks), wait_us);
    }
}
