//! 64-event flight recorder. Events carry a 100 us timestamp (from the
//! harvest tick counter), a kind, and one u16 payload. Ring semantics:
//! newest 64 survive. Dump on any kill.

use core::sync::atomic::{AtomicU32, Ordering};
use rtt_target::rprintln;

pub const KIND_RUNG_START: u8 = 1;
pub const KIND_RUNG_END: u8 = 2;
pub const KIND_COAST_START: u8 = 3;
pub const KIND_COAST_RESULT_HZ10: u8 = 4; // value = measured eHz * 10
pub const KIND_COAST_RESULT_MV: u8 = 5; // value = BEMF amplitude, terminal mV
pub const KIND_STRIKE_OC: u8 = 6;
pub const KIND_STRIKE_SAG: u8 = 7;
pub const KIND_KILL_OC: u8 = 8;
pub const KIND_KILL_SAG: u8 = 9;
pub const KIND_KILL_NFLT: u8 = 10;
pub const KIND_KILL_NTC: u8 = 11;
pub const KIND_KILL_HOST: u8 = 12;
pub const KIND_KILL_DONE: u8 = 13;
pub const KIND_RESUME: u8 = 14;
pub const KIND_INFO: u8 = 15;
pub const KIND_PROVOKE: u8 = 16;

#[derive(Clone, Copy)]
struct Event {
    t100: u32,
    kind: u8,
    val: u16,
}

const N: usize = 64;
static mut RING: [Event; N] = [Event {
    t100: 0,
    kind: 0,
    val: 0,
}; N];
static HEAD: AtomicU32 = AtomicU32::new(0); // total pushes (mod for index)

/// Push from ISR context (never preempted on this build: single IRQ).
#[inline]
pub fn push_isr(t100: u32, kind: u8, val: u16) {
    let h = HEAD.load(Ordering::Relaxed);
    unsafe {
        RING[(h as usize) % N] = Event { t100, kind, val };
    }
    HEAD.store(h.wrapping_add(1), Ordering::Relaxed);
}

/// Push from main context (masks IRQs around the ring update).
pub fn push(t100: u32, kind: u8, val: u16) {
    cortex_m::interrupt::free(|_| push_isr(t100, kind, val));
}

fn kind_name(k: u8) -> &'static str {
    match k {
        KIND_RUNG_START => "RUNG_START",
        KIND_RUNG_END => "RUNG_END",
        KIND_COAST_START => "COAST",
        KIND_COAST_RESULT_HZ10 => "COAST_EHZ10",
        KIND_COAST_RESULT_MV => "COAST_MV",
        KIND_STRIKE_OC => "STRIKE_OC",
        KIND_STRIKE_SAG => "STRIKE_SAG",
        KIND_KILL_OC => "KILL_OC",
        KIND_KILL_SAG => "KILL_SAG",
        KIND_KILL_NFLT => "KILL_NFLT",
        KIND_KILL_NTC => "KILL_NTC",
        KIND_KILL_HOST => "KILL_HOST",
        KIND_KILL_DONE => "DONE",
        KIND_RESUME => "RESUME",
        KIND_INFO => "INFO",
        KIND_PROVOKE => "PROVOKE",
        _ => "?",
    }
}

/// Dump oldest->newest to RTT, and as `BB,`-prefixed CSV text lines via
/// the provided writer (host parser treats non-sync bytes as text).
pub fn dump<W: core::fmt::Write>(w: &mut W) {
    let total = HEAD.load(Ordering::Relaxed);
    let n = core::cmp::min(total as usize, N);
    let start = total as usize - n;
    let _ = writeln!(w, "\n=BLACKBOX {} events=", n);
    rprintln!("=BLACKBOX {} events=", n);
    for i in 0..n {
        let e = unsafe { RING[(start + i) % N] };
        let _ = writeln!(w, "BB,{},{},{}", e.t100, kind_name(e.kind), e.val);
        rprintln!("BB t={}00us {} {}", e.t100, kind_name(e.kind), e.val);
    }
    let _ = writeln!(w, "=BLACKBOX END=");
    rprintln!("=BLACKBOX END=");
}
