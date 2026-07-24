//! Blackbox firmware adapter — timestamps, critical section, dump.
//!
//! Ported from `minz/src/bb.rs`. Wraps the portable ring
//! (`rm32::blackbox`) in a `Mutex<RefCell<..>>` so every context (COMP /
//! TIM16 / TIM6 ISRs at their split priorities, plus the main loop) can
//! record safely — the brief `interrupt::free` is the price of a
//! producer set that spans priority levels. Timestamps are
//! DWT.CYCCNT/800 = 10 µs units truncated to u16 (deltas are what the
//! dump renders; wraps at 655 ms, far above per-commutation spacing).
//!
//! M4 targets only (DWT).

#![cfg(all(
    feature = "blackbox",
    any(feature = "stm32l431", feature = "stm32g431")
))]

use core::cell::RefCell;

use cortex_m::interrupt::Mutex;
use rm32::blackbox::{BB_LEN, BlackBox, EMPTY, Event, format_dump};

static BB: Mutex<RefCell<BlackBox>> = Mutex::new(RefCell::new(BlackBox::new()));

#[inline]
fn now_10us() -> u16 {
    let cyc = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    (cyc / 800) as u16
}

/// Record one event (no-op while frozen). Callable from any context.
#[inline]
pub fn record(ty: u8, sector: u8, data: u16) {
    cortex_m::interrupt::free(|cs| {
        BB.borrow(cs).borrow_mut().record(Event {
            t: now_10us(),
            ty,
            sector,
            data,
        });
    });
}

/// Freeze the ring so the dump shows the lead-up to a fault, not the
/// aftermath. Only `thaw` (not wired to any command yet) re-enables.
pub fn freeze() {
    cortex_m::interrupt::free(|cs| BB.borrow(cs).borrow_mut().freeze());
}

/// Snapshot + render the ring, oldest first. The copy happens under the
/// critical section (64 × 6 B); formatting and the sink run outside it.
/// Returns the number of events dumped.
pub fn dump(sink: impl FnMut(&[u8])) -> usize {
    let mut snap = [EMPTY; BB_LEN];
    let mut n = 0;
    cortex_m::interrupt::free(|cs| {
        let bb = BB.borrow(cs).borrow();
        for (i, e) in bb.replay().enumerate() {
            snap[i] = *e;
            n = i + 1;
        }
    });
    format_dump(snap.into_iter().take(n), sink);
    n
}
