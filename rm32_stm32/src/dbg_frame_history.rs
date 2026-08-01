//! Bench-debug ring buffer of recent DSHOT decode attempts.
//!
//! Captures the most recent `CAP` frames seen by `handle_exti_frame` along
//! with their first few captured edges, pass/fail status, and bidir state.
//! Dumped from the main-loop heartbeat to diagnose bidir DSHOT decode
//! failures (e.g. DMA buffer alignment, edge polarity issues).
//!
//! Module is enabled only under the `debuguart` cargo feature so it doesn't
//! bloat production builds.

#![cfg(feature = "debuguart")]

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use heapless::HistoryBuffer;

/// One frame snapshot. ~52 bytes each.
#[derive(Clone, Copy, Default)]
pub struct FrameSnap {
    /// Monotonic ISR frame counter at capture time.
    pub n: u32,
    /// First 8 captured edge timestamps (16-bit TIM counter, stored as u32).
    pub buf: [u32; 8],
    /// Whether `decode_frame` returned a valid frame.
    pub crc_pass: bool,
    /// Whether `dshot_telemetry` (bidir) was set when decode ran.
    pub bidir: bool,
}

const CAP: usize = 8;

static HIST: Mutex<RefCell<HistoryBuffer<FrameSnap, CAP>>> =
    Mutex::new(RefCell::new(HistoryBuffer::new()));

/// Push a snapshot. Safe to call from ISR; takes a critical section.
pub fn push(snap: FrameSnap) {
    cortex_m::interrupt::free(|cs| {
        HIST.borrow(cs).borrow_mut().write(snap);
    });
}

/// Drain the buffer into a heapless Vec (safe to use outside critical section).
pub fn take() -> heapless::Vec<FrameSnap, CAP> {
    cortex_m::interrupt::free(|cs| {
        let mut hist = HIST.borrow(cs).borrow_mut();
        let out: heapless::Vec<FrameSnap, CAP> = hist.oldest_ordered().copied().collect();
        hist.clear();
        out
    })
}
