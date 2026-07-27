//! ISR-duration histograms — constant-cost distribution capture.
//!
//! Single-sample DWT stores (`dbg_*_last_cyc`) are blind to rare long
//! blips: a point-read catches a random tick, not the worst case. This
//! captures the whole distribution instead, at ~10 cycles/record (sub,
//! shr, min, atomic add) with NO value-dependent branching — so it can
//! never itself create the blips it measures (the constant-per-tick-ISR
//! rule; the `[t16]` print that manufactured deaf windows was the
//! opposite of this).
//!
//! Binning (operator's design): `idx = min(delta >> SHIFT, NBINS-1)`.
//! The `min` is the "above expected max" overflow bin — every blip lands
//! in the top bin and is counted. Bin 0 = `[0, 2^SHIFT)` catches the
//! low tail. With SHIFT=8 (256-cycle = 3.2 µs bins) × 16 bins the range
//! is 0-4096 cycles = 0-51 µs ≈ one 57 µs wall window, so the top bin
//! ("≥48 µs") is a whole-window-eater smoking gun.

#![cfg(all(
    feature = "benchuart",
    any(feature = "stm32l431", feature = "stm32g431")
))]

use core::sync::atomic::{AtomicU32, Ordering};

pub const NBINS: usize = 16;
/// 256-cycle bins (3.2 µs at 80 MHz); 16 bins → 0-4096 cyc (0-51 µs).
pub const SHIFT: u32 = 8;

/// One histogram per instrumented site. Index by [`Site`].
pub const N_SITES: usize = 3;
#[repr(usize)]
pub enum Site {
    Tim6 = 0,  // ten_khz_tick (20 kHz control ISR)
    Tim16 = 1, // commutation_timer_expired
    Comp = 2,  // bemf_zero_cross (acceptance)
}

static HIST: [[AtomicU32; NBINS]; N_SITES] =
    [const { [const { AtomicU32::new(0) }; NBINS] }; N_SITES];
/// Cumulative cycle-sum per site (for utilization = sum / wall-cycles).
static SUM: [AtomicU32; N_SITES] = [const { AtomicU32::new(0) }; N_SITES];
static CNT: [AtomicU32; N_SITES] = [const { AtomicU32::new(0) }; N_SITES];

/// Record one ISR duration (DWT cycles). Constant cost, ISR context.
#[inline(always)]
pub fn record(site: Site, delta_cyc: u32) {
    let s = site as usize;
    let idx = ((delta_cyc >> SHIFT) as usize).min(NBINS - 1);
    HIST[s][idx].fetch_add(1, Ordering::Relaxed);
    SUM[s].fetch_add(delta_cyc, Ordering::Relaxed);
    CNT[s].fetch_add(1, Ordering::Relaxed);
}

/// Snapshot a site's bins + (sum, count). Main-loop context.
pub fn snapshot(site: usize) -> ([u32; NBINS], u32, u32) {
    let mut bins = [0u32; NBINS];
    for (i, b) in bins.iter_mut().enumerate() {
        *b = HIST[site][i].load(Ordering::Relaxed);
    }
    (
        bins,
        SUM[site].load(Ordering::Relaxed),
        CNT[site].load(Ordering::Relaxed),
    )
}

/// Zero all histograms (call at the start of a measured hold so the
/// captured distribution isn't diluted by spin-up).
pub fn reset() {
    for s in 0..N_SITES {
        for b in &HIST[s] {
            b.store(0, Ordering::Relaxed);
        }
        SUM[s].store(0, Ordering::Relaxed);
        CNT[s].store(0, Ordering::Relaxed);
    }
}
