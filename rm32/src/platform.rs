//! Static facts a port supplies about its chip.
//!
//! One implementation per port (a zero-sized type). No associated const has
//! a default: a default equal to one chip's value is exactly the coupling
//! this trait exists to remove (docs/generic_core_design.md, principle 5).

/// Clock facts the control core is written against.
pub trait Platform {
    /// Control-tick rate (AM32 `tenKhzRoutine`, `LOOP_FREQUENCY_HZ`), Hz.
    const TICK_HZ: u32;
    /// Interval / commutation timer count rate, Hz.
    const TIMER_HZ: u32;
}

/// Microseconds to interval/commutation timer ticks (truncating). Use in a
/// const context (an associated const of [`crate::constants::Timing`]) so it
/// folds at compile time: the 64-bit arithmetic must never reach a core
/// without hardware divide.
pub const fn us_to_ticks<P: Platform>(us: u32) -> u32 {
    ((us as u64 * P::TIMER_HZ as u64) / 1_000_000) as u32
}

/// Milliseconds to control ticks (truncating); const-context only, as
/// [`us_to_ticks`].
pub const fn ms_to_control_ticks<P: Platform>(ms: u32) -> u32 {
    ((ms as u64 * P::TICK_HZ as u64) / 1000) as u32
}

/// Sum of the last six commutation intervals (timer ticks) to `e_com_time`,
/// the electrical period in microseconds. At a 2 MHz timer this is AM32's
/// `(sum + 4) >> 1` (the +4 is AM32's rounding bias, kept); other clocks
/// round to nearest. The branch is on a const and folds away.
#[inline(always)]
pub fn six_intervals_to_us<P: Platform>(sum: u32) -> i32 {
    if P::TIMER_HZ == 2_000_000 {
        ((sum + 4) >> 1) as i32
    } else {
        ((sum as u64 * 1_000_000 + P::TIMER_HZ as u64 / 2) / P::TIMER_HZ as u64) as i32
    }
}

/// Electrical period in microseconds to the average single-step interval in
/// timer ticks (one sixth of the period). At a 2 MHz timer this is AM32's
/// `e_com_time / 3`, computed divide-free ([`crate::fast_math::div3_i32`],
/// exact for every `i32`). A port with another clock on a core without
/// hardware divide must supply a divide-free form (its link audit catches
/// the 64-bit division).
#[inline(always)]
pub fn e_com_us_to_avg_ticks<P: Platform>(e_com_us: i32) -> i32 {
    if P::TIMER_HZ == 2_000_000 {
        crate::fast_math::div3_i32(e_com_us)
    } else {
        (e_com_us as i64 * P::TIMER_HZ as i64 / 6_000_000) as i32
    }
}

/// AM32's clocks (20 kHz control tick, 2 MHz timer) for the core's own unit
/// tests. Not a default: ports and the harness name their own platform.
#[cfg(test)]
pub(crate) struct TestPlatform;

#[cfg(test)]
impl Platform for TestPlatform {
    const TICK_HZ: u32 = 20_000;
    const TIMER_HZ: u32 = 2_000_000;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At 2 MHz both conversions are AM32's literal arithmetic, for every
    /// value the six-slot u16 sum and its e_com can take.
    #[test]
    fn two_mhz_conversions_are_am32_arithmetic() {
        for sum in (0..=6 * u16::MAX as u32)
            .step_by(7)
            .chain([6 * u16::MAX as u32])
        {
            assert_eq!(
                six_intervals_to_us::<TestPlatform>(sum),
                ((sum + 4) >> 1) as i32
            );
        }
        for e in (-200_000..=200_000).chain([i32::MIN + 1, i32::MAX]) {
            assert_eq!(e_com_us_to_avg_ticks::<TestPlatform>(e), e / 3);
        }
    }
}
