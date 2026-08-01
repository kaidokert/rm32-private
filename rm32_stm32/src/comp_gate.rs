//! COMP acceptance-gate stale average — the AM32-verbatim gate source.
//!
//! This is CONTROL behavior and MUST be available in EVERY build. It
//! previously lived in `edge_probe`, which is `#![cfg(zctrace)]`, so a
//! build compiled without the zctrace instrumentation silently fell back
//! to the FRESH `e_com/3` gate average — a known divergence that opens
//! the acceptance gate early, releases camped edges into early accepts,
//! and breaks BEMF lock down to low throttle. (Regression found
//! 2026-07-27: the zctrace-free build failed to reach Running at 20-40%.)
//! Keeping the latch here makes the firmware lock identically whether or
//! not instrumentation is compiled in — whether the motor runs must
//! never depend on a debug feature flag.

use core::sync::atomic::{AtomicU32, Ordering};

/// 20 kHz-latched average interval for the COMP gate (AM32-verbatim
/// staleness: main.c:2283 computes average_interval in the slow loop;
/// the ISR gate it.c:280 reads that stale value). rm32 previously fed
/// the gate per-commutation-fresh e_com/3 — "fresher" is a divergence:
/// in comp-mode transients a freshly-shrunk gate opens early and
/// releases camped edges into early accepts.
static GATE_AVG: AtomicU32 = AtomicU32::new(0);

/// Latch the current average — called from the 20 kHz control tick in
/// ALL builds (one relaxed store, negligible cost).
#[inline]
pub fn latch(v: u32) {
    GATE_AVG.store(v, Ordering::Relaxed);
}

/// Read the latched average (0 = still cold; caller falls back to fresh).
#[inline]
pub fn get() -> u32 {
    GATE_AVG.load(Ordering::Relaxed)
}
