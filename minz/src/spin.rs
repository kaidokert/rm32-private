//! Bounded busy-wait — the ONLY sanctioned way to wait on a hardware
//! flag. Unbounded `while flag {}` loops are banned in this codebase
//! (operator directive, 2026-07-19): three distinct unbounded spins
//! were implicated in wedge/IWDG-reboot classes this campaign (the
//! microloop pacing spin, the TX DMA EN readback, the ADC ADSTP
//! waits — one of them reachable from ISR context via the WAX
//! trigger). A hardware flag that misbehaves must cost a bounded
//! number of cycles and a counter increment, never the firmware.

use core::sync::atomic::{AtomicU32, Ordering};

/// Total bounded-spin timeouts since boot. Nonzero = some hardware
/// flag failed to settle inside its budget — surfaced in the `i`
/// diagnostics; investigate, but the firmware stayed alive.
pub static SPIN_TIMEOUTS: AtomicU32 = AtomicU32::new(0);

/// Spin until `cond()` returns true or `max_spins` iterations pass.
/// Returns whether the condition was met. Budget rule of thumb: a
/// few-µs hardware settle at 80 MHz is thousands of iterations;
/// 100_000 (~ms) is a generous ceiling for anything legitimate.
#[inline]
pub fn spin_until(max_spins: u32, mut cond: impl FnMut() -> bool) -> bool {
    let mut n = 0u32;
    while n < max_spins {
        if cond() {
            return true;
        }
        n += 1;
    }
    SPIN_TIMEOUTS.fetch_add(1, Ordering::Relaxed);
    false
}
