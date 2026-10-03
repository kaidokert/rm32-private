//! Characterization instruments, for the `char-capture` image only (study
//! 2026-10-01). Production installs none of this: the duty schedule and the
//! recorder hang off `SagLog` (production `NoSagLog`), the commutation
//! histograms and tail rings off `ChainLog` (production `NoChain`), and both
//! fold away when `ON == false`.
//!
//! Three instruments, one wire format, dumped after `safe_off` (never a byte
//! while the bridge is live -- UART edges couple into the comparator):
//!
//! * **Profile**: up to [`MAX_WP`] waypoints `(duty, slew, hold)` replacing the
//!   production ramp after the loop closes. Slew in tenths per second, `0` a
//!   step, [`SLEW_PROD`] the production staircase (10 tenths every 500 ms).
//! * **Recorder**: one sample every `period` ADC scans (101 µs each) of applied
//!   duty, mean per-scan shunt sum, mean bus code, the published average
//!   commutation interval and the accepts in the sample. [`REC_LEN`] samples,
//!   oldest overwritten; it stops `post_ms` after the last waypoint starts.
//! * **Commutation statistics** (COMP and COM roots), counted only inside the
//!   settled window `[reach(wp0) + settle, end of wp0's hold)`: per-sector
//!   histograms of `|interval - mean of the last six|` in 1 µs bins, a
//!   histogram of commutation-service lateness, excursions (interval at least
//!   1.5x that mean) and late arms. Plus two tail rings (accepts, services) of
//!   [`TAIL_LEN`], frozen at the first `freeze()` (a foldback or any stop).

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering::Relaxed};

use cortex_m::interrupt::{self, Mutex};

use crate::chain::{Arm, ChainLog};
use crate::report::Sink;
use crate::sagtrace::{Block, SagLog};
use crate::shared::{CompPrio, Motor, Root, SHARED as S, Seam};

pub const MAX_WP: usize = 8;
/// Slew value meaning "the production staircase" (`ramp::duty_at`'s shape).
pub const SLEW_PROD: u16 = u16::MAX;
/// 640 samples: the run path nests `rung` (~6.9 KB) and `finish` (~4.2 KB)
/// frames, so RAM for rings is what the stack leaves (36 KB part).
pub const REC_LEN: usize = 640;
/// Accept tail: 1024 crossings, >= 50 ms even at full duty (~20 k/s).
pub const TAIL_LEN: usize = 1024;
/// Service tail: shorter; lateness is fully histogrammed regardless.
pub const SVC_LEN: usize = 256;
pub const BINS: usize = 64;
/// One ADC scan, µs (TIM6 at 9901 Hz).
pub const SCAN_US: u32 = 101;
/// The duty the loop closes at (`ramp::START_TENTHS`): the schedule's origin.
const START: u16 = 100;

#[derive(Clone, Copy, Default)]
pub struct Waypoint {
    pub duty: u16,
    pub slew_tps: u16,
    pub hold_ms: u32,
}

/// The schedule, parsed by the bin and installed before the run.
#[derive(Clone, Copy)]
pub struct Profile {
    pub n: usize,
    pub wp: [Waypoint; MAX_WP],
    pub period_scans: u16,
    pub settle_ms: u32,
    pub post_ms: u32,
}

impl Profile {
    pub const EMPTY: Self = Self {
        n: 0,
        wp: [Waypoint {
            duty: 0,
            slew_tps: 0,
            hold_ms: 0,
        }; MAX_WP],
        period_scans: 100,
        settle_ms: 0,
        post_ms: 0,
    };
    /// The highest duty the schedule asks for (the run's nominal target).
    pub fn max_duty(&self) -> u16 {
        let mut m = START;
        for w in &self.wp[..self.n] {
            m = m.max(w.duty);
        }
        m
    }
}

/// Segment boundaries in ms since close, precomputed at install.
#[derive(Clone, Copy, Default)]
struct Seg {
    from: u16,
    to: u16,
    slew: u16,
    start_ms: u32,
    reach_ms: u32,
    end_ms: u32,
}

/// Ramp time from `a` to `b` at `slew` tenths/s, ms.
const fn ramp_ms(a: u16, b: u16, slew: u16) -> u32 {
    let d = if a > b { a - b } else { b - a } as u32;
    if slew == 0 || d == 0 {
        0
    } else if slew == SLEW_PROD {
        // 10 tenths per 500 ms step, last step partial: ceil(d / 10) * 500.
        d.div_ceil(10) * 500
    } else {
        (d * 1000).div_ceil(slew as u32)
    }
}

struct Sched {
    on: bool,
    n: usize,
    seg: [Seg; MAX_WP],
    total_ms: u32,
    hist_from_ms: u32,
    hist_to_ms: u32,
    rec_stop_ms: u32,
}

struct Rec {
    on: bool,
    period: u16,
    acc_n: u16,
    acc_cur: u32,
    acc_bus: u32,
    last_seq: u32,
    scans: u32,
    close_scan: u32,
    closed: bool,
    total: u32,
    /// µs since the loop closed, as of the latest closed-loop pass (0 before).
    since_close: u32,
    /// The run clock at the instant the loop closed.
    close_at_us: u32,
    t_us: [u32; REC_LEN],
    duty: [u16; REC_LEN],
    cur: [u16; REC_LEN],
    bus: [u16; REC_LEN],
    avg: [u16; REC_LEN],
    acc: [u8; REC_LEN],
}

static SCHED: Mutex<RefCell<Sched>> = Mutex::new(RefCell::new(Sched {
    on: false,
    n: 0,
    seg: [Seg {
        from: 0,
        to: 0,
        slew: 0,
        start_ms: 0,
        reach_ms: 0,
        end_ms: 0,
    }; MAX_WP],
    total_ms: 0,
    hist_from_ms: 0,
    hist_to_ms: 0,
    rec_stop_ms: u32::MAX,
}));

static REC: Mutex<RefCell<Rec>> = Mutex::new(RefCell::new(Rec {
    on: false,
    period: 100,
    acc_n: 0,
    acc_cur: 0,
    acc_bus: 0,
    last_seq: 0,
    scans: 0,
    close_scan: 0,
    closed: false,
    total: 0,
    since_close: 0,
    close_at_us: 0,
    t_us: [0; REC_LEN],
    duty: [0; REC_LEN],
    cur: [0; REC_LEN],
    bus: [0; REC_LEN],
    avg: [0; REC_LEN],
    acc: [0; REC_LEN],
}));

/// Read by both motor roots: inside the settled window.
static HIST_ON: AtomicBool = AtomicBool::new(false);

/// The schedule's segment table for a profile (pure; host-tested).
fn build(p: &Profile) -> Sched {
    let mut s = Sched {
        on: p.n > 0,
        n: p.n,
        seg: [Seg {
            from: 0,
            to: 0,
            slew: 0,
            start_ms: 0,
            reach_ms: 0,
            end_ms: 0,
        }; MAX_WP],
        total_ms: 0,
        hist_from_ms: u32::MAX,
        hist_to_ms: 0,
        rec_stop_ms: u32::MAX,
    };
    let mut t = 0u32;
    let mut prev = START;
    for (i, w) in p.wp[..p.n].iter().enumerate() {
        let r = ramp_ms(prev, w.duty, w.slew_tps);
        s.seg[i] = Seg {
            from: prev,
            to: w.duty,
            slew: w.slew_tps,
            start_ms: t,
            reach_ms: t + r,
            end_ms: t + r + w.hold_ms,
        };
        t += r + w.hold_ms;
        prev = w.duty;
    }
    s.total_ms = t;
    if p.n > 0 {
        s.hist_from_ms = s.seg[0].reach_ms + p.settle_ms;
        s.hist_to_ms = s.seg[0].end_ms;
        if p.post_ms > 0 {
            s.rec_stop_ms = s.seg[p.n - 1].start_ms + p.post_ms;
        }
    }
    s
}

/// Install a profile and arm every instrument for the next run (bridge off).
pub fn arm(p: &Profile) {
    let built = build(p);
    interrupt::free(|cs| {
        *SCHED.borrow(cs).borrow_mut() = built;
        let mut r = REC.borrow(cs).borrow_mut();
        r.on = true;
        r.period = p.period_scans.max(1);
        r.acc_n = 0;
        r.acc_cur = 0;
        r.acc_bus = 0;
        r.last_seq = S.det().accept_seq.load(Relaxed);
        r.scans = 0;
        r.close_scan = 0;
        r.closed = false;
        r.total = 0;
        r.since_close = 0;
    });
    HIST_ON.store(false, Relaxed);
    let _ = ACC.lock(|a| {
        *a = AccStats::EMPTY;
        a.on = true;
    });
    let _ = SVC.lock(|v| {
        *v = SvcStats::EMPTY;
        v.on = true;
    });
}

/// The scheduled duty at `t` ms since close.
fn duty_at_ms(s: &Sched, t: u32) -> u16 {
    let mut i = 0;
    while i < s.n {
        let g = s.seg[i];
        if t < g.end_ms || i + 1 == s.n {
            if t >= g.reach_ms {
                return g.to;
            }
            let el = t - g.start_ms;
            let moved = if g.slew == 0 {
                u32::MAX
            } else if g.slew == SLEW_PROD {
                (el / 500 + 1) * 10
            } else {
                el * u32::from(g.slew) / 1000
            };
            return if g.to >= g.from {
                (u32::from(g.from) + moved).min(u32::from(g.to)) as u16
            } else {
                (u32::from(g.from).saturating_sub(moved)).max(u32::from(g.to)) as u16
            };
        }
        i += 1;
    }
    START
}

/// The `SagLog` the char image installs: schedule + recorder.
pub struct CharRec;

impl SagLog for CharRec {
    const ON: bool = true;

    #[inline]
    fn block(b: &Block) {
        interrupt::free(|cs| {
            let mut r = REC.borrow(cs).borrow_mut();
            if !r.on {
                return;
            }
            r.scans = r.scans.wrapping_add(1);
            r.acc_cur += u32::from(b.phase_a) + u32::from(b.phase_b) + u32::from(b.phase_c);
            r.acc_bus += u32::from(b.bus_raw);
            r.acc_n += 1;
            if r.acc_n >= r.period {
                let n = u32::from(r.acc_n);
                let i = (r.total as usize) % REC_LEN;
                r.t_us[i] = r.since_close;
                r.duty[i] = b.duty_tenths;
                r.cur[i] = (r.acc_cur / n) as u16;
                r.bus[i] = (r.acc_bus / n) as u16;
                r.avg[i] = S.det().accept_avg.load(Relaxed).min(0xFFFF) as u16;
                let seq = S.det().accept_seq.load(Relaxed);
                r.acc[i] = seq.wrapping_sub(r.last_seq).min(255) as u8;
                r.last_seq = seq;
                r.total = r.total.wrapping_add(1);
                r.acc_n = 0;
                r.acc_cur = 0;
                r.acc_bus = 0;
            }
        });
    }

    #[inline]
    fn freeze() {
        // The tail rings keep the window that led into the first foldback or
        // the stop. The recorder keeps running: a foldback is part of the trace.
        let _ = ACC.lock(|a| a.ring_on = false);
        let _ = SVC.lock(|v| v.ring_on = false);
    }

    #[inline]
    fn duty(now_us: u32, since_close_us: u32) -> Option<u16> {
        let t = since_close_us / 1000;
        interrupt::free(|cs| {
            let s = SCHED.borrow(cs).borrow();
            let mut r = REC.borrow(cs).borrow_mut();
            if !r.closed {
                r.closed = true;
                r.close_scan = r.scans;
                r.close_at_us = now_us.wrapping_sub(since_close_us);
            }
            r.since_close = since_close_us;
            if t >= s.rec_stop_ms {
                r.on = false;
            }
            HIST_ON.store(s.on && t >= s.hist_from_ms && t < s.hist_to_ms, Relaxed);
            if s.on { Some(duty_at_ms(&s, t)) } else { None }
        })
    }

    #[inline]
    fn done(since_close_us: u32) -> bool {
        let t = since_close_us / 1000;
        interrupt::free(|cs| {
            let s = SCHED.borrow(cs).borrow();
            s.on && t >= s.total_ms
        })
    }
}

/// COMP-owned: accept tail ring + interval histograms.
pub struct AccStats {
    on: bool,
    ring_on: bool,
    total: u32,
    at_us: [u16; TAIL_LEN],
    wait: [u16; TAIL_LEN],
    flag: [u8; TAIL_LEN],
    prev_us: u16,
    iv: [u16; 6],
    k: u8,
    sum6: u32,
    pub hist: [[u32; BINS]; 6],
    pub accepts: u32,
    pub excursions: u32,
    pub late_arms: u32,
}

impl AccStats {
    const EMPTY: Self = Self {
        on: false,
        ring_on: true,
        total: 0,
        at_us: [0; TAIL_LEN],
        wait: [0; TAIL_LEN],
        flag: [0; TAIL_LEN],
        prev_us: 0,
        iv: [0; 6],
        k: 0,
        sum6: 0,
        hist: [[0; BINS]; 6],
        accepts: 0,
        excursions: 0,
        late_arms: 0,
    };
}

/// COM-owned: service tail ring + lateness histogram.
pub struct SvcStats {
    on: bool,
    ring_on: bool,
    total: u32,
    sched: [u16; SVC_LEN],
    late: [u8; SVC_LEN],
    flag: [u8; SVC_LEN],
    pub late_hist: [u32; BINS],
    pub services: u32,
}

impl SvcStats {
    const EMPTY: Self = Self {
        on: false,
        ring_on: true,
        total: 0,
        sched: [0; SVC_LEN],
        late: [0; SVC_LEN],
        flag: [0; SVC_LEN],
        late_hist: [0; BINS],
        services: 0,
    };
}

static ACC: Seam<AccStats, CompPrio> = Seam::new(AccStats::EMPTY);
static SVC: Seam<SvcStats, Motor> = Seam::new(SvcStats::EMPTY);

/// The `ChainLog` the char image installs.
pub struct CharChain;

impl ChainLog for CharChain {
    const ON: bool = true;

    #[inline(always)]
    fn accept(at: &mut Root<CompPrio>, a: &Arm) {
        ACC.root(at, |s| {
            if !s.on {
                return;
            }
            let iv = a.crossing_us.wrapping_sub(s.prev_us);
            s.prev_us = a.crossing_us;
            let k = s.k as usize;
            s.sum6 = s.sum6 + u32::from(iv) - u32::from(s.iv[k]);
            s.iv[k] = iv;
            s.k = if k == 5 { 0 } else { s.k + 1 };
            if HIST_ON.load(Relaxed) {
                // |6*iv - sum6| is six times the deviation from the mean of the
                // last six intervals; x43 >> 8 divides by ~5.95 without a call.
                let six = 6 * u32::from(iv);
                let dev6 = if six > s.sum6 { six - s.sum6 } else { s.sum6 - six };
                let bin = ((dev6 * 43) >> 8).min(BINS as u32 - 1) as usize;
                let sec = (a.step.saturating_sub(1) as usize).min(5);
                s.hist[sec][bin] += 1;
                s.accepts += 1;
                if 4 * u32::from(iv) >= s.sum6 {
                    s.excursions += 1;
                }
                if a.late {
                    s.late_arms += 1;
                }
            }
            if s.ring_on {
                let i = (s.total as usize) % TAIL_LEN;
                s.at_us[i] = a.crossing_us;
                s.wait[i] = a.wait_us.min(0xFFFF) as u16;
                s.flag[i] = (a.step & 0x07) | ((a.stage & 0x03) << 4) | if a.late { 0x80 } else { 0 };
                s.total = s.total.wrapping_add(1);
            }
        });
    }

    #[inline(always)]
    fn service(at: &mut Root<Motor>, _fire: u16, _bridge: u16, sched_us: u16, late_us: u32, phase: u32, step: u8) {
        SVC.root(at, |s| {
            if !s.on {
                return;
            }
            if phase == 1 && HIST_ON.load(Relaxed) {
                s.late_hist[(late_us as usize).min(BINS - 1)] += 1;
                s.services += 1;
            }
            if s.ring_on {
                let i = (s.total as usize) % SVC_LEN;
                s.sched[i] = sched_us;
                s.late[i] = late_us.min(255) as u8;
                s.flag[i] = (step & 0x07) | ((phase as u8 & 0x0F) << 4);
                s.total = s.total.wrapping_add(1);
            }
        });
    }
}

fn say_bins(out: &mut impl Sink, tag: &str, h: &[u32; BINS]) {
    out.say(tag);
    for v in h {
        out.say(" ");
        out.say_u32(*v);
    }
    out.say("\r\n");
    out.flush();
}

/// Dump everything after `safe_off`. One wire format for every run.
pub fn emit(out: &mut impl Sink, run_start_us: u32) {
    let _ = ACC.lock(|a| a.on = false);
    let _ = SVC.lock(|v| v.on = false);
    HIST_ON.store(false, Relaxed);
    let (total, close_scan, period, scans, close_at) = interrupt::free(|cs| {
        let mut r = REC.borrow(cs).borrow_mut();
        r.on = false;
        (
            r.total,
            r.close_scan,
            r.period,
            r.scans,
            if r.closed { Some(r.close_at_us) } else { None },
        )
    });
    out.say("CHARSNAP ");
    out.kv("v", 2);
    out.kv("rec_total", total);
    out.kv("rec_len", REC_LEN as u32);
    out.kv("period_scans", u32::from(period));
    out.kv("scan_us", SCAN_US);
    out.kv("scans", scans);
    out.kv("close_scan", close_scan);
    // Bridge-on to loop-closed, µs: the startup the restart time is made of.
    out.kv("startup_us", close_at.map_or(0, |c| c.wrapping_sub(run_start_us)));
    out.kv("closed", u32::from(close_at.is_some()));
    out.say("\r\n");
    out.flush();
    let n = (total as usize).min(REC_LEN);
    let start = if total as usize > REC_LEN {
        total as usize % REC_LEN
    } else {
        0
    };
    for k in 0..n {
        let i = (start + k) % REC_LEN;
        let (t, d, c, b, a, q) = interrupt::free(|cs| {
            let r = REC.borrow(cs).borrow();
            (r.t_us[i], r.duty[i], r.cur[i], r.bus[i], r.avg[i], r.acc[i])
        });
        out.say("CHARREC ");
        out.say_u32(t);
        out.say(" ");
        out.say_u32(u32::from(d));
        out.say(" ");
        out.say_u32(u32::from(c));
        out.say(" ");
        out.say_u32(u32::from(b));
        out.say(" ");
        out.say_u32(u32::from(a));
        out.say(" ");
        out.say_u32(u32::from(q));
        out.say("\r\n");
        out.flush();
    }
    let _ = ACC.lock(|a| {
        out.say("CHARCOUNT ");
        out.kv("accepts", a.accepts);
        out.kv("excursions", a.excursions);
        out.kv("late_arms", a.late_arms);
        out.kv("acc_ring_total", a.total);
        out.say("\r\n");
        out.flush();
        for s in 0..6 {
            out.say("CHARHIST ");
            out.say_u32(s as u32 + 1);
            say_bins(out, "", &a.hist[s]);
        }
        let n = (a.total as usize).min(TAIL_LEN);
        let st = if a.total as usize > TAIL_LEN {
            a.total as usize % TAIL_LEN
        } else {
            0
        };
        for k in 0..n {
            let i = (st + k) % TAIL_LEN;
            out.say("CHARACC ");
            out.say_u32(u32::from(a.at_us[i]));
            out.say(" ");
            out.say_u32(u32::from(a.wait[i]));
            out.say(" ");
            out.say_u32(u32::from(a.flag[i]));
            out.say("\r\n");
            out.flush();
        }
    });
    let _ = SVC.lock(|v| {
        out.say("CHARSVCCOUNT ");
        out.kv("services", v.services);
        out.kv("svc_ring_total", v.total);
        out.say("\r\n");
        say_bins(out, "CHARLATE", &v.late_hist);
        let n = (v.total as usize).min(SVC_LEN);
        let st = if v.total as usize > SVC_LEN {
            v.total as usize % SVC_LEN
        } else {
            0
        };
        for k in 0..n {
            let i = (st + k) % SVC_LEN;
            out.say("CHARSVC ");
            out.say_u32(u32::from(v.sched[i]));
            out.say(" ");
            out.say_u32(u32::from(v.late[i]));
            out.say(" ");
            out.say_u32(u32::from(v.flag[i]));
            out.say("\r\n");
            out.flush();
        }
    });
    out.say("CHAREND\r\n");
    out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sched(wps: &[(u16, u16, u32)]) -> Sched {
        let mut p = Profile::EMPTY;
        for (i, w) in wps.iter().enumerate() {
            p.wp[i] = Waypoint {
                duty: w.0,
                slew_tps: w.1,
                hold_ms: w.2,
            };
        }
        p.n = wps.len();
        build(&p)
    }

    #[test]
    fn a_step_lands_at_once_and_a_slew_ramps_linearly() {
        let s = sched(&[(500, 0, 1000), (800, 300, 1000)]);
        assert_eq!(duty_at_ms(&s, 0), 500);
        assert_eq!(duty_at_ms(&s, 999), 500);
        // 300 tenths at 300 tenths/s: 1 s ramp, halfway at +500 ms.
        assert_eq!(duty_at_ms(&s, 1500), 650);
        assert_eq!(duty_at_ms(&s, 2000), 800);
        assert_eq!(s.total_ms, 3000);
    }

    #[test]
    fn the_production_staircase_matches_ramp_shape_and_goes_down() {
        let s = sched(&[(300, SLEW_PROD, 0), (100, SLEW_PROD, 0)]);
        // 200 tenths up: 20 steps of 500 ms; the first step lands immediately.
        assert_eq!(duty_at_ms(&s, 0), 110);
        assert_eq!(duty_at_ms(&s, 499), 110);
        assert_eq!(duty_at_ms(&s, 500), 120);
        assert_eq!(duty_at_ms(&s, 9_999), 300);
        assert_eq!(duty_at_ms(&s, 10_000 + 500), 280);
        assert_eq!(s.total_ms, 20_000);
    }
}
