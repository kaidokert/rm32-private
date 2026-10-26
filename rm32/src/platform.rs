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

/// AM32's clocks (20 kHz control tick, 2 MHz timer) for the core's own unit
/// tests. Not a default: ports and the harness name their own platform.
#[cfg(test)]
pub(crate) struct TestPlatform;

#[cfg(test)]
impl Platform for TestPlatform {
    const TICK_HZ: u32 = 20_000;
    const TIMER_HZ: u32 = 2_000_000;
}
