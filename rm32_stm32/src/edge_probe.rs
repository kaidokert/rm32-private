//! Edge/veto probe — per-commutation-window counters of what the COMP
//! acceptance machinery SAW, plus the TIM16 fire latency. The instrument
//! for the 70%-wall orbit hunt: entry into the z = ci ± wait period-2
//! mode is decided by edges the outcome metrics never show (swallowed
//! pre-ZC edges, camp storms, persistence refusals).
//!
//! Cost discipline (constant per-tick ISR work): each COMP entry adds a
//! handful of relaxed atomic ops; the snapshot/reset runs once per
//! commutation in the TIM16 handler. All counters saturate — a camp
//! storm must clamp, not wrap.
//!
//! Aliasing safety: plain atomics, no `ISR_LOCAL` access — safe from any
//! priority (see notes/ISR_STATE_INVARIANT.md).

#![cfg(feature = "zctrace")]

use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};

/// Sentinel: no COMP edge seen this window.
pub const NO_EDGE: u16 = 0xFFFF;

static FIRST_EDGE: AtomicU16 = AtomicU16::new(NO_EDGE);
static ENTRIES: AtomicU16 = AtomicU16::new(0);
static GATED_CLEARS: AtomicU16 = AtomicU16::new(0);
static PERSIST_REJECTS: AtomicU16 = AtomicU16::new(0);
static TIM16_LAT: AtomicU16 = AtomicU16::new(NO_EDGE);
static LAST_ARM: AtomicU16 = AtomicU16::new(NO_EDGE);
/// Lifetime totals for the heartbeat (drops-style visibility even when
/// the zct stream is off).
static TOTAL_GATED: AtomicU32 = AtomicU32::new(0);
static TOTAL_REJECTS: AtomicU32 = AtomicU32::new(0);
/// Comp-engagement violation counters: driven phase's N-pin MODER was
/// NOT AF right after a comp-mode commutation (per phase A/B/C).
static NPIN_VIOL: [AtomicU32; 3] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

/// Check the driven phase's N-pin right after com_step (TIM16 ISR).
/// phase_idx: 0=A(PB1) 1=B(PB0) 2=C(PA7).
#[inline]
pub fn npin_check(phase_idx: usize, ok: bool) {
    if !ok {
        NPIN_VIOL[phase_idx].fetch_add(1, Ordering::Relaxed);
    }
}

/// Mid-window violation info: count + last (step | cnt<<8) snapshot.
static MIDW_VIOL: AtomicU32 = AtomicU32::new(0);
static MIDW_INFO: AtomicU32 = AtomicU32::new(0);

#[inline]
pub fn midw_violation(step: u8, cnt: u32) {
    MIDW_VIOL.fetch_add(1, Ordering::Relaxed);
    MIDW_INFO.store((step as u32) | (cnt.min(0xFFFFFF) << 8), Ordering::Relaxed);
}

pub fn midw() -> (u32, u32) {
    (
        MIDW_VIOL.load(Ordering::Relaxed),
        MIDW_INFO.load(Ordering::Relaxed),
    )
}

/// Self-hosted watchpoint: writer PC/LR captured by DebugMonitor.
static WATCH_PC: AtomicU32 = AtomicU32::new(0);
static WATCH_LR: AtomicU32 = AtomicU32::new(0);

static WATCH_HITS: AtomicU32 = AtomicU32::new(0);

pub fn watch_store(pc: u32, lr: u32) {
    WATCH_PC.store(pc, Ordering::Relaxed);
    WATCH_LR.store(lr, Ordering::Relaxed);
    WATCH_HITS.fetch_add(1, Ordering::Relaxed);
}

pub fn watch_hits() -> u32 {
    WATCH_HITS.load(Ordering::Relaxed)
}

pub fn watch_read() -> (u32, u32) {
    (
        WATCH_PC.load(Ordering::Relaxed),
        WATCH_LR.load(Ordering::Relaxed),
    )
}

pub fn npin_violations() -> (u32, u32, u32) {
    (
        NPIN_VIOL[0].load(Ordering::Relaxed),
        NPIN_VIOL[1].load(Ordering::Relaxed),
        NPIN_VIOL[2].load(Ordering::Relaxed),
    )
}

/// COMP ISR entry with a confirmed pending edge. `cnt` = interval-timer
/// count at classification time. COMP-ISR context.
#[inline]
pub fn edge_seen(cnt: u32) {
    let e = ENTRIES.load(Ordering::Relaxed);
    ENTRIES.store(e.saturating_add(1), Ordering::Relaxed);
    if FIRST_EDGE.load(Ordering::Relaxed) == NO_EDGE {
        FIRST_EDGE.store(cnt.min(0xFFFE) as u16, Ordering::Relaxed);
    }
}

/// Gate-closed pre-ZC edge swallowed (acked without evaluation).
#[inline]
pub fn gated_clear() {
    let g = GATED_CLEARS.load(Ordering::Relaxed);
    GATED_CLEARS.store(g.saturating_add(1), Ordering::Relaxed);
    TOTAL_GATED.fetch_add(1, Ordering::Relaxed);
}

/// Gate-open entry refused by the persistence filter.
#[inline]
pub fn persist_reject() {
    let p = PERSIST_REJECTS.load(Ordering::Relaxed);
    PERSIST_REJECTS.store(p.saturating_add(1), Ordering::Relaxed);
    TOTAL_REJECTS.fetch_add(1, Ordering::Relaxed);
}

/// COM-timer arm value (set_and_enable timeout). 1 = the CommutateKick
/// arm; wait+1 = a normal accept/polling arm. Any ISR context.
#[inline]
pub fn armed(timeout: u16) {
    LAST_ARM.store(timeout, Ordering::Relaxed);
}

/// TIM16 commutation ISR entry. `cnt` = interval-timer count at entry;
/// scheduled fire was wait_time+1, so `cnt - (wait+1)` = fire latency
/// (includes NVIC arbitration + any same-priority tail-chain delay —
/// the invisible-resource axis).
#[inline]
pub fn tim16_fired(cnt: u32) {
    TIM16_LAT.store(cnt.min(0xFFFE) as u16, Ordering::Relaxed);
}

/// Snapshot the window that just ended and reset for the next one.
/// Returns (first_edge, entries, tim16_lat, last_arm, gated_clears,
/// persist_rejects). Call ONCE per commutation (TIM16 handler, after
/// the step logic).
#[inline]
pub fn take() -> (u16, u16, u16, u16, u8, u8) {
    let fe = FIRST_EDGE.swap(NO_EDGE, Ordering::Relaxed);
    let en = ENTRIES.swap(0, Ordering::Relaxed);
    let tl = TIM16_LAT.swap(NO_EDGE, Ordering::Relaxed);
    let la = LAST_ARM.load(Ordering::Relaxed);
    let gc = GATED_CLEARS.swap(0, Ordering::Relaxed);
    let pr = PERSIST_REJECTS.swap(0, Ordering::Relaxed);
    (fe, en, tl, la, gc.min(255) as u8, pr.min(255) as u8)
}

/// Lifetime (gated_clears, persist_rejects) for the heartbeat.
pub fn totals() -> (u32, u32) {
    (
        TOTAL_GATED.load(Ordering::Relaxed),
        TOTAL_REJECTS.load(Ordering::Relaxed),
    )
}
