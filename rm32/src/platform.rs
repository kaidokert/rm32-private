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
