//! ZC-trace firmware adapter — per-commutation 15-byte records.
//!
//! Ported from the minz composition layer (`minz/core/src/zct_trace.rs`):
//! ring storage + batch-decimation state + the enable toggle live here;
//! the pack/gate/ring mechanics are host-tested in `rm32::bench_input`.
//! Producer today: `handle_tim14` (one record per commutation). The push
//! stays critical-section-guarded so a second producer priority (the
//! TIM6 polling path) can join later without a protocol change.
//!
//! Off by default; the bench `Z` key toggles the stream (host side:
//! `minz/scripts/zctrace_capture.py` — the wire format is byte-identical).

#![cfg(feature = "zctrace")]

use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use rm32::bench_input::{ZCT_REC, ZctRing, zct_batch_gate, zct_pack};

const ZCT_N: usize = 64;
static RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);
static DROP: AtomicU32 = AtomicU32::new(0);
static COMM_N: AtomicU32 = AtomicU32::new(0);
static BATCHING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);

#[inline]
fn ring() -> ZctRing<'static, ZCT_N> {
    ZctRing {
        ring: &RING,
        head: &HEAD,
        tail: &TAIL,
        drop: &DROP,
    }
}

/// Toggle the stream; returns the NEW state.
pub fn toggle() -> bool {
    !ENABLED.fetch_xor(true, Ordering::Relaxed)
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn drop_count() -> u32 {
    DROP.load(Ordering::Relaxed)
}

/// One record per commutation (ISR context). Applies the batch gate,
/// packs, and pushes under a brief critical section.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn write(
    step: u8,
    old: bool,
    thiszc: u16,
    ci: u16,
    wait: u16,
    duty: u16,
    tenkhz: u16,
    avg: u16,
) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let n = COMM_N.fetch_add(1, Ordering::Relaxed);
    let (record, batching) = zct_batch_gate(n, ci as u32, BATCHING.load(Ordering::Relaxed));
    BATCHING.store(batching, Ordering::Relaxed);
    if !record {
        return;
    }
    let rec = zct_pack(step, old, batching, thiszc, ci, wait, duty, tenkhz, avg);
    cortex_m::interrupt::free(|_| ring().push_rec(&rec));
}

/// Drain up to 3 records into a byte sink (main-loop pass cadence,
/// matching the minz foreground drain).
#[inline]
pub fn drain(sink: impl FnMut(u8)) {
    ring().drain(3, sink);
}
