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

use rm32::bench_input::{ZCT_REC, ZctRing, zct_batch_gate, zct_pack, zct_probe_pack};

// 128: the probe row doubles pushes per commutation; headroom so a camp
// storm's paired rows don't evict each other before the drain.
const ZCT_N: usize = 128;
static RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);
static DROP: AtomicU32 = AtomicU32::new(0);
static COMM_N: AtomicU32 = AtomicU32::new(0);
static BATCHING: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);
/// Freeze-on-fall: once set, producers stop pushing so the ring +
/// in-flight wire hold the last pre-fall records intact (camp storms
/// starve the drain and previously garbaged the fall boundary).
/// Cleared by the Z toggle.
static FROZEN: AtomicBool = AtomicBool::new(false);

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
    FROZEN.store(false, Ordering::Relaxed);
    !ENABLED.fetch_xor(true, Ordering::Relaxed)
}

/// Called from the commutation ISR on a fall signature.
pub fn freeze() {
    FROZEN.store(true, Ordering::Relaxed);
}

pub fn frozen() -> bool {
    FROZEN.load(Ordering::Relaxed)
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn drop_count() -> u32 {
    DROP.load(Ordering::Relaxed)
}

/// One record per commutation (ISR context). Applies the batch gate,
/// packs, and pushes under a brief critical section. Returns
/// `Some(batching)` when the record was pushed (the paired edge-probe
/// record must ride the SAME gate decision), `None` when gated off.
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
) -> Option<bool> {
    if !ENABLED.load(Ordering::Relaxed) || FROZEN.load(Ordering::Relaxed) {
        return None;
    }
    let n = COMM_N.fetch_add(1, Ordering::Relaxed);
    let (record, batching) = zct_batch_gate(n, ci as u32, BATCHING.load(Ordering::Relaxed));
    BATCHING.store(batching, Ordering::Relaxed);
    if !record {
        return None;
    }
    let rec = zct_pack(step, old, batching, thiszc, ci, wait, duty, tenkhz, avg);
    cortex_m::interrupt::free(|_| ring().push_rec(&rec));
    Some(batching)
}

/// The edge-probe companion row (`5B A6`) for a commutation whose zct
/// row was recorded. Same ISR context, same ring.
#[inline]
#[allow(clippy::too_many_arguments)]
pub fn write_probe(
    batching: bool,
    step: u8,
    old: bool,
    first_edge: u16,
    comp_entries: u16,
    tim16_lat: u16,
    avg: u16,
    gated_clears: u8,
    persist_rejects: u8,
    last_arm: u16,
) {
    let rec = zct_probe_pack(
        step,
        old,
        batching,
        first_edge,
        comp_entries,
        tim16_lat,
        avg,
        gated_clears,
        persist_rejects,
        last_arm,
    );
    cortex_m::interrupt::free(|_| ring().push_rec(&rec));
}

/// Drain up to 3 records into a byte sink (main-loop pass cadence,
/// matching the minz foreground drain).
#[inline]
pub fn drain(sink: impl FnMut(u8)) {
    ring().drain(3, sink);
}
