//! Bench instrument: per-sector commutation-interval statistics (binz).
//!
//! firmware50's `charz.rs` method: at every accepted zero-crossing, keep the
//! last six accept intervals; bin `|interval - mean of the last six|` per
//! commutation sector (1 us bins, 16 bins, last bin = overflow), count
//! excursions (interval >= 1.5x that mean), and keep a SIGNED per-sector sum
//! of the deviation so a static long/short alternation between sectors shows
//! directly (a mean deviation of the wrong sign per sector), which an
//! `|deviation|` histogram alone cannot separate from jitter.
//!
//! Producer: the COMP ISR pushes accepts (`accept`) into a ring, only while locked (running and not
//! old_routine). Main reads/prints only when the motor is stopped and
//! `reset`s at a hold start (a reset racing an accept loses one sample).
//! Intervals are interval-timer counts (2 MHz: 0.5 us).

use core::sync::atomic::{AtomicI32, AtomicU32, Ordering::Relaxed};

pub const BINS: usize = 16;

static IV: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static K: AtomicU32 = AtomicU32::new(0);
static SUM6: AtomicU32 = AtomicU32::new(0);
static FILL: AtomicU32 = AtomicU32::new(0);
static HIST: [[AtomicU32; BINS]; 6] = [const { [const { AtomicU32::new(0) }; BINS] }; 6];
static DEV_SUM: [AtomicI32; 6] = [const { AtomicI32::new(0) }; 6];
static N: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static EXC: AtomicU32 = AtomicU32::new(0);
/// Per sector: accepts at iv <= 0.5625x the 6-step mean — i.e. at (or just
/// after) the AM32 half-interval gate opening, where a comparator already
/// camping at the post-ZC level is accepted the moment the gate opens.
static GATE: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
/// Raw context of the first EXN excursions: 8 accepts before (incl. the
/// excursion itself) and 8 after, each `iv | step << 16`. Main-loop only.
const EXN: usize = 4;
static mut LAST8: [u32; 8] = [0; 8];
static mut LAST_K: usize = 0;
static mut EXTRACE: [[u32; 16]; EXN] = [[0; 16]; EXN];
static mut EX_N: usize = 0; // traces started
static mut EX_AFTER: usize = 0; // accepts still to record into the open trace

/// Accept ring: the COMP ISR only pushes `(iv, step)`; the main loop does
/// the statistics (`drain`). Pushing costs ~10 instructions in the ISR — the
/// in-ISR version cost +94 instructions / +3.4 us and took COMP to 1.98x AM32.
const RING: usize = 256;
static RING_BUF: [AtomicU32; RING] = [const { AtomicU32::new(0) }; RING];
static HEAD: AtomicU32 = AtomicU32::new(0);
static TAIL: AtomicU32 = AtomicU32::new(0);
static DROPS: AtomicU32 = AtomicU32::new(0);

/// COMP ISR, after an accepted crossing. `iv` = this accept's interval-timer
/// count (time since the previous accept), `step` = commutation step 1..=6.
#[inline]
pub fn accept(iv: u16, step: u8) {
    let h = HEAD.load(Relaxed);
    if h.wrapping_sub(TAIL.load(Relaxed)) >= RING as u32 {
        DROPS.store(DROPS.load(Relaxed).wrapping_add(1), Relaxed);
        return;
    }
    RING_BUF[(h as usize) & (RING - 1)].store(iv as u32 | (step as u32) << 16, Relaxed);
    HEAD.store(h.wrapping_add(1), Relaxed);
}

/// Main loop: fold every queued accept into the statistics.
pub fn drain() {
    let h = HEAD.load(Relaxed);
    let mut t = TAIL.load(Relaxed);
    while t != h {
        let v = RING_BUF[(t as usize) & (RING - 1)].load(Relaxed);
        stat(v as u16, (v >> 16) as u8);
        t = t.wrapping_add(1);
    }
    TAIL.store(t, Relaxed);
}

fn stat(iv: u16, step: u8) {
    // SAFETY: main-loop only (drain/reset/dump), never from an ISR.
    unsafe {
        let rec = iv as u32 | (step as u32) << 16;
        LAST8[LAST_K & 7] = rec;
        LAST_K = LAST_K.wrapping_add(1);
        if EX_AFTER > 0 {
            EXTRACE[EX_N - 1][16 - EX_AFTER] = rec;
            EX_AFTER -= 1;
        }
    }
    let iv = iv as u32;
    let k = K.load(Relaxed) as usize;
    let sum6 = SUM6.load(Relaxed) + iv - IV[k].load(Relaxed);
    IV[k].store(iv, Relaxed);
    SUM6.store(sum6, Relaxed);
    K.store(if k == 5 { 0 } else { k as u32 + 1 }, Relaxed);
    let fill = FILL.load(Relaxed);
    if fill < 6 {
        FILL.store(fill + 1, Relaxed);
        return; // ring not full yet: the mean is not defined
    }
    let s = (step.clamp(1, 6) - 1) as usize;
    // 6*iv - sum6 = six times the deviation in counts (0.5 us): 12 per us.
    let d6 = (6 * iv) as i32 - sum6 as i32;
    let bin = ((d6.unsigned_abs() * 21) >> 8).min(BINS as u32 - 1) as usize; // /12.2
    HIST[s][bin].store(HIST[s][bin].load(Relaxed) + 1, Relaxed);
    DEV_SUM[s].store(DEV_SUM[s].load(Relaxed).wrapping_add(d6), Relaxed);
    N[s].store(N[s].load(Relaxed) + 1, Relaxed);
    if 4 * iv >= sum6 {
        EXC.store(EXC.load(Relaxed) + 1, Relaxed);
        // SAFETY: main-loop only.
        unsafe {
            if EX_AFTER == 0 && EX_N < EXN {
                for i in 0..8 {
                    EXTRACE[EX_N][i] = LAST8[(LAST_K.wrapping_add(i)) & 7];
                }
                EX_N += 1;
                EX_AFTER = 8;
            }
        }
    }
    if 32 * iv <= 3 * sum6 {
        GATE[s].store(GATE[s].load(Relaxed) + 1, Relaxed);
    }
}

/// Main loop: clear the statistics (hold start).
pub fn reset() {
    for s in 0..6 {
        for b in &HIST[s] {
            b.store(0, Relaxed);
        }
        DEV_SUM[s].store(0, Relaxed);
        N[s].store(0, Relaxed);
        GATE[s].store(0, Relaxed);
    }
    // SAFETY: main-loop only.
    unsafe {
        EX_N = 0;
        EX_AFTER = 0;
    }
    EXC.store(0, Relaxed);
    FILL.store(0, Relaxed);
    DROPS.store(0, Relaxed);
}

/// Main loop, motor stopped: one line per sector plus a summary.
/// `h<s> n=<accepts> dev_x6=<signed mean of 6*deviation, counts> b=<16 bins>`
pub fn dump() {
    for s in 0..6 {
        let n = N[s].load(Relaxed);
        let mean6 = if n > 0 {
            DEV_SUM[s].load(Relaxed) / n as i32
        } else {
            0
        };
        let h = &HIST[s];
        crate::dprintln!(
            "h{} n={} dev_x6={} b={},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            s + 1,
            n,
            mean6,
            h[0].load(Relaxed),
            h[1].load(Relaxed),
            h[2].load(Relaxed),
            h[3].load(Relaxed),
            h[4].load(Relaxed),
            h[5].load(Relaxed),
            h[6].load(Relaxed),
            h[7].load(Relaxed),
            h[8].load(Relaxed),
            h[9].load(Relaxed),
            h[10].load(Relaxed),
            h[11].load(Relaxed),
            h[12].load(Relaxed),
            h[13].load(Relaxed),
            h[14].load(Relaxed),
            h[15].load(Relaxed)
        );
    }
    crate::dprintln!(
        "hx excursions={} drops={}",
        EXC.load(Relaxed),
        DROPS.load(Relaxed)
    );
    // SAFETY: main-loop only.
    let traces = unsafe { &*core::ptr::addr_of!(EXTRACE) };
    for (i, t) in traces.iter().enumerate().take(unsafe { EX_N }) {
        // step:iv (0.5 us counts), oldest first; entry 7 is the excursion.
        crate::dprintln!(
            "he{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{} {}:{}",
            i,
            t[0] >> 16,
            t[0] & 0xFFFF,
            t[1] >> 16,
            t[1] & 0xFFFF,
            t[2] >> 16,
            t[2] & 0xFFFF,
            t[3] >> 16,
            t[3] & 0xFFFF,
            t[4] >> 16,
            t[4] & 0xFFFF,
            t[5] >> 16,
            t[5] & 0xFFFF,
            t[6] >> 16,
            t[6] & 0xFFFF,
            t[7] >> 16,
            t[7] & 0xFFFF,
            t[8] >> 16,
            t[8] & 0xFFFF,
            t[9] >> 16,
            t[9] & 0xFFFF,
            t[10] >> 16,
            t[10] & 0xFFFF,
            t[11] >> 16,
            t[11] & 0xFFFF,
            t[12] >> 16,
            t[12] & 0xFFFF,
            t[13] >> 16,
            t[13] & 0xFFFF,
            t[14] >> 16,
            t[14] & 0xFFFF,
            t[15] >> 16,
            t[15] & 0xFFFF
        );
    }
    crate::dprintln!(
        "hg gate_accepts={},{},{},{},{},{}",
        GATE[0].load(Relaxed),
        GATE[1].load(Relaxed),
        GATE[2].load(Relaxed),
        GATE[3].load(Relaxed),
        GATE[4].load(Relaxed),
        GATE[5].load(Relaxed)
    );
}
