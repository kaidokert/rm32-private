//! The sharp-sag guard's own inputs, judgement by judgement, in two bounded
//! rings that freeze on the fault (campaign 9 step 3).
//!
//! Campaign 7 saw the guard latch in three of six runs at 50% and the cause
//! was never found. Every quantity quoted at it since has been the wrong one:
//! `bus_min` is the minimum of a **single raw scan** over the whole run
//! (`run::states`), and the report's `filt_bus` is the filter's value at the
//! *end* of the run. Neither is what the guard compares.
//!
//! What it does compare, traced in `protection::FastBusSag::observe`:
//!
//! ```text
//! bus_mean * filt_vref * SAG_DEN  <  filt_bus * vref_mean * SAG_NUM
//! ```
//!
//! **Judged on every scan**, not on a decimated block: `Ctx::scan_pass` runs
//! per `adc_due`, and `protection::RailMean` is a *sliding* mean over the last
//! `RAIL_MEAN_LEN` scans, fed every scan. The captures put that rate at
//! **9.8 kHz** (`SAGSNAP total` against `BEMFGUARD ticks`), i.e. one judgement
//! per ~101 µs. An earlier version of this module said "blocks" at ~2.1 kHz
//! and was wrong by 23x, which made its coverage claim wrong by the same
//! factor; the independent pre-run review of E175 caught it.
//!
//! **So the guard is a band-pass, and that is the central fact about what it
//! can detect.** Its numerator is an 8-scan sliding mean (~808 µs, about nine
//! sectors at 47.5%), so anything faster — a per-PWM-period dip, a switching
//! transient — is averaged away before the comparison. Its denominator is a
//! ~207 ms exponential average, so anything slower is followed and becomes
//! margin rather than fault. Only dips between about 0.8 ms and 200 ms are
//! visible to it at all. The absolute floor (`BUS_FLOOR_NUM`, `Reason::Bus`)
//! is what covers the slow side.
//!
//! **And it is primed from the bridge-off baseline** (`FastBusSag::new`), so
//! for roughly the first 207 ms of a run its reference is an *unloaded* rail
//! while the bus is already loaded: that is when the guard is at its most
//! sensitive, and it is the interval a tail-only ring never retains. That is a
//! hypothesis about the 50% trips, not a finding — the 50% runs will say.
//!
//! **Two rings, because one cannot answer both questions.**
//!
//! * [`FAST_LEN`] judgements at full rate — **~26 ms** — for the shape of the
//!   dip that trips the guard;
//! * [`SLOW_LEN`] judgements decimated by [`SLOW_EVERY`] — ~3.3 s, i.e.
//!   **sixteen** filter time constants (3.31 s / 207 ms) — for **how the
//!   reference got where it was**, which
//!   is what explaining a latch requires and what the fast ring alone cannot
//!   show.
//!
//! Both freeze on the fault, so a dump is the window that led there. Both are
//! written after `safe_off`, like every other byte. Row quantities are raw:
//! the host reproduces the cross-product (`scripts/sag.py`), so the firmware
//! does no division.
//!
//! Production runs [`NoSagLog`], whose `ON` is `false`, so every call folds
//! away; only the `sag-capture` binary installs [`SagRing`]. RAM cost there:
//! `FAST_LEN` x 28 B + `SLOW_LEN` x 8 B = **15 360 B** of the part's 36 KB,
//! which `scripts/structure_report.py` checks per image against the stack. This is diagnostic evidence and never qualifies another
//! image.

use core::cell::RefCell;

use cortex_m::interrupt::{self, Mutex};

/// Full-rate judgements kept: **26.0 ms** at the measured 9.84 kHz.
///
/// **256, halved from 512 to pay for the raw bus, the three phase codes, the
/// carrier phase and the fine stamp** (campaign 11). The row went 16 -> **28 B**
/// and the two rings now take 256 x 28 + 1024 x 8 = **15 360 B**, less than the
/// 16 384 B this image took before. Paying for new fields by shortening the ring rather than by growing
/// `.bss` is E185's lesson: widening these rings once left under 4 KB of stack
/// on a part whose largest frame reserves 5076 B and which has no stack guard,
/// and every run died inside a millisecond.
///
/// 26 ms is **~32x the guard's fast edge** (~0.8 ms), the shortest dip it can
/// latch on; it is *not* longer than the slow edge (~200 ms), so this ring
/// shows the shape of a fast event while the decimated ring shows the
/// reference's history. The analysis uses the last rows before the freeze.
pub const FAST_LEN: usize = 256;
/// Decimated judgements kept, and the decimation: 1024 rows every 32nd
/// judgement is **~3.3 s** at the measured 9.8 kHz (1024 x 32 x 101 µs), i.e.
/// sixteen of the reference's 207 ms time constants. `scripts/sag.py` prints
/// the span from the dump rather than from this comment.
pub const SLOW_LEN: usize = 1024;
pub const SLOW_EVERY: u32 = 32;
const _: () = assert!(SLOW_EVERY.is_power_of_two());

/// One judgement: the guard's own inputs and the state it judged them in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Block {
    /// The raw µs clock (TIM17) when the block was judged. Kept for pairing
    /// in the unit the control path works in, exactly as `chain::Beat` does.
    pub at: u16,
    /// The **fine** stamp: low 16 bits of TIM2 at 125 ns, which is the shared
    /// diagnostic timeline this ring and `chain::Beat` now have in common.
    /// Wraps every **8.192 ms**; the host must not pair across a longer gap
    /// (`hw::fine::SPAN16_US`).
    pub at_fine: u16,
    /// **The PWM counter at this scan**, so the raw samples above are
    /// interpretable at all.
    ///
    /// The ADC trigger is deliberately de-cohered from the carrier -- TIM6 at
    /// 9901 Hz against a 48.0 kHz carrier -- so the sample point walks ~0.85 of
    /// a carrier period per scan and every raw sample below lands at a
    /// different, unknown point of the switching cycle. That is right for
    /// reconstructing a DC average (which is what the 8-tap mean is for) and
    /// useless for reading one raw sample as a bus level, because consecutive
    /// samples differ by carrier ripple aliased at an unknown phase. With this
    /// field the host can bin by phase, or restrict a comparison to samples
    /// taken at a like point in the cycle.
    pub pwm_ctr: u16,
    /// **The raw bus sample of this very scan**, not the mean.
    ///
    /// Without it nothing here can state the width of a dip: the mean below is
    /// an 8-tap boxcar, so it widens every event by 7 scans and a single-scan
    /// spike is indistinguishable from a five-scan notch. Two independent
    /// reviews found a width prediction built on the mean to be unfalsifiable.
    pub bus_raw: u16,
    /// The three shunt amplifiers of this scan, raw codes.
    ///
    /// The sag verdict returns **before** `current.accumulate`, so the block
    /// containing a trip is discarded and `worst_hold_ma` structurally cannot
    /// hold a surge co-located with it. These are the same values that would
    /// have been accumulated, recorded before the verdict is consulted.
    pub phase_a: u16,
    pub phase_b: u16,
    pub phase_c: u16,
    /// What the guard compared: the block means, and the filtered reference
    /// **as used for this test** (before its post-test update).
    pub bus_mean: u16,
    pub vref_mean: u16,
    pub filt_bus: u16,
    pub filt_vref: u16,
    /// The streak the guard holds *after* judging this block, so a 1 or 2 is a
    /// low block that did not latch.
    pub streak: u8,
    /// The sector in force, and the applied duty in tenths of a percent.
    pub step: u8,
    pub duty_tenths: u16,
    /// µs since the last **accepted zero crossing** — not since the
    /// commutation, which happens `wait_time` later and is not recorded here.
    /// Zero before the loop is closed. At ~101 µs between judgements and an
    /// 84 µs sector at 47.5% this is **aliased**, so it cannot answer whether
    /// a dip is phase-locked to switching; the review of E175 established
    /// that, and `scripts/sag.py` no longer pretends otherwise.
    pub since_zc_us: u16,
}

/// Where the guard's blocks go. `ON` false folds every call away.
pub trait SagLog {
    const ON: bool;
    fn block(b: &Block);
    /// Freeze the ring: the trip has happened and the pre-trip window is what
    /// matters.
    fn freeze();
    /// ENV-76 (campaign C): the current accumulator closed a 100-scan block.
    /// The recorder sums the same scans' shunts and bus itself, so production's
    /// accumulator is untouched; production's `ON == false` folds the call away.
    fn current_block_end() {}
}

/// Production: nothing is recorded.
pub struct NoSagLog;

impl SagLog for NoSagLog {
    const ON: bool = false;
    #[inline(always)]
    fn block(_: &Block) {}
    #[inline(always)]
    fn freeze() {}
}

/// The ring itself. Foreground-only — the guard is judged in `Ctx::pass`, not
/// in an interrupt — so a plain `static mut`-free cell with a critical section
/// is unnecessary: the recorder is only ever touched from thread mode.
pub struct Trace {
    pub on: bool,
    pub frozen: bool,
    /// Judgements offered to the fast ring, and to the slow one.
    pub total: u32,
    pub fast_len: usize,
    fast_next: usize,
    pub fast: [Block; FAST_LEN],
    pub slow_len: usize,
    slow_next: usize,
    pub slow: [Slow; SLOW_LEN],
}

/// A decimated row: only what the reference's history needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slow {
    pub bus_mean: u16,
    pub vref_mean: u16,
    pub filt_bus: u16,
    pub filt_vref: u16,
}

impl Trace {
    pub const fn empty() -> Self {
        const EMPTY: Block = Block {
            at: 0,
            at_fine: 0,
            pwm_ctr: 0,
            bus_raw: 0,
            phase_a: 0,
            phase_b: 0,
            phase_c: 0,
            bus_mean: 0,
            vref_mean: 0,
            filt_bus: 0,
            filt_vref: 0,
            streak: 0,
            step: 0,
            duty_tenths: 0,
            since_zc_us: 0,
        };
        const SLOW0: Slow = Slow {
            bus_mean: 0,
            vref_mean: 0,
            filt_bus: 0,
            filt_vref: 0,
        };
        Self {
            on: false,
            frozen: false,
            total: 0,
            fast_len: 0,
            fast_next: 0,
            fast: [EMPTY; FAST_LEN],
            slow_len: 0,
            slow_next: 0,
            slow: [SLOW0; SLOW_LEN],
        }
    }

    /// The next circular index, written out: a modulo would be a helper
    /// division on this M0+ if the length ever stopped being a power of two
    /// (E154's finding, in the capture ring).
    const fn bump(i: usize, len: usize) -> usize {
        if i + 1 == len { 0 } else { i + 1 }
    }

    pub fn push(&mut self, b: &Block) {
        if !self.on || self.frozen {
            return;
        }
        self.fast[self.fast_next] = *b;
        self.fast_next = Self::bump(self.fast_next, FAST_LEN);
        if self.fast_len < FAST_LEN {
            self.fast_len += 1;
        }
        // A mask, not a modulo: the decimation is a power of two by assertion
        // below, and a modulo invites the division helper this crate forbids.
        if self.total & (SLOW_EVERY - 1) == 0 {
            self.slow[self.slow_next] = Slow {
                bus_mean: b.bus_mean,
                vref_mean: b.vref_mean,
                filt_bus: b.filt_bus,
                filt_vref: b.filt_vref,
            };
            self.slow_next = Self::bump(self.slow_next, SLOW_LEN);
            if self.slow_len < SLOW_LEN {
                self.slow_len += 1;
            }
        }
        self.total = self.total.wrapping_add(1);
    }

    /// The oldest kept row's index in each ring.
    pub const fn fast_start(&self) -> usize {
        if self.fast_len == FAST_LEN { self.fast_next } else { 0 }
    }

    pub const fn slow_start(&self) -> usize {
        if self.slow_len == SLOW_LEN { self.slow_next } else { 0 }
    }
}

/// The diagnostic image's recorder.
///
/// Foreground-only, so it needs no priority ceiling: the sag guard is judged
/// in `Ctx::scan_pass`, which runs in thread mode. It is a
/// `Mutex<RefCell<..>>` behind `interrupt::free` all the same, because this
/// crate forbids `static mut` and the dump reads the ring after the run.
pub struct SagRing;

static TRACE: Mutex<RefCell<Trace>> = Mutex::new(RefCell::new(Trace::empty()));

/// **Per-block ring (ENV-76, campaign C).** One entry per 100-scan current
/// block: the raw shunt sum over those scans (the host subtracts the run's own
/// `zero_start` to get the firmware's residual), the bus mean over the same
/// scans, and how many scans the recorder saw (100 when aligned). The last
/// `BLK_LEN` blocks (~5.2 s) are kept, oldest overwritten; never frozen.
pub const BLK_LEN: usize = 512;

pub struct BlkRing {
    pub sum: [u32; BLK_LEN],
    pub bus: [u16; BLK_LEN],
    pub n: [u8; BLK_LEN],
    /// ENV-79: the first half-block's (scans 1..50) shunt sum, >> 3.
    pub half: [u16; BLK_LEN],
    next: usize,
    pub len: usize,
    pub total: u32,
    acc_sum: u32,
    acc_bus: u32,
    acc_n: u32,
    acc_half: u32,
    on: bool,
}

impl BlkRing {
    const fn empty() -> Self {
        Self {
            sum: [0; BLK_LEN],
            bus: [0; BLK_LEN],
            n: [0; BLK_LEN],
            half: [0; BLK_LEN],
            next: 0,
            len: 0,
            total: 0,
            acc_sum: 0,
            acc_bus: 0,
            acc_n: 0,
            acc_half: 0,
            on: false,
        }
    }
    fn scan(&mut self, b: &Block) {
        if !self.on {
            return;
        }
        self.acc_sum = self
            .acc_sum
            .wrapping_add(u32::from(b.phase_a) + u32::from(b.phase_b) + u32::from(b.phase_c));
        self.acc_bus = self.acc_bus.wrapping_add(u32::from(b.bus_raw));
        self.acc_n += 1;
        if self.acc_n == 50 {
            self.acc_half = self.acc_sum;
        }
    }
    fn close(&mut self) {
        if !self.on {
            return;
        }
        let i = self.next;
        if i < BLK_LEN {
            self.sum[i] = self.acc_sum;
            self.bus[i] = if self.acc_n == 0 {
                0
            } else {
                (self.acc_bus / self.acc_n) as u16
            };
            self.n[i] = self.acc_n.min(255) as u8;
            self.half[i] = (self.acc_half >> 3).min(0xFFFF) as u16;
        }
        self.next = if i + 1 >= BLK_LEN { 0 } else { i + 1 };
        if self.len < BLK_LEN {
            self.len += 1;
        }
        self.total = self.total.wrapping_add(1);
        self.acc_sum = 0;
        self.acc_bus = 0;
        self.acc_n = 0;
        self.acc_half = 0;
    }
    fn reset(&mut self) {
        self.next = 0;
        self.len = 0;
        self.total = 0;
        self.acc_sum = 0;
        self.acc_bus = 0;
        self.acc_n = 0;
        self.on = true;
    }
}

static BLK: Mutex<RefCell<BlkRing>> = Mutex::new(RefCell::new(BlkRing::empty()));

/// Dump the per-block ring, oldest first: `SAGBLK <shunt_sum> <bus_mean> <scans>`.
pub fn emit_blocks(out: &mut impl crate::report::Sink) {
    interrupt::free(|cs| BLK.borrow(cs).borrow_mut().on = false);
    let (len, next, total) = interrupt::free(|cs| {
        let b = BLK.borrow(cs).borrow();
        (b.len, b.next, b.total)
    });
    out.say("SAGBLKSNAP ");
    out.kv("len", len as u32);
    out.kv("total", total);
    out.kv("block_scans", crate::protection::BLOCK_SCANS);
    out.say(
        "
",
    );
    out.flush();
    let start = if len < BLK_LEN { 0 } else { next };
    let mut k = 0;
    while k < len {
        let i = (start + k) % BLK_LEN;
        let (s, b, n, h) = interrupt::free(|cs| {
            let r = BLK.borrow(cs).borrow();
            (r.sum[i], r.bus[i], r.n[i], r.half[i])
        });
        out.say("SAGBLK ");
        out.say_u32(s);
        out.say(" ");
        out.say_u32(u32::from(b));
        out.say(" ");
        out.say_u32(u32::from(n));
        out.say(" ");
        out.say_u32(u32::from(h));
        out.say(
            "
",
        );
        out.flush();
        k += 1;
    }
    out.say(
        "SAGBLKEND
",
    );
    out.flush();
}

impl SagLog for SagRing {
    const ON: bool = true;

    #[inline]
    fn block(b: &Block) {
        interrupt::free(|cs| {
            TRACE.borrow(cs).borrow_mut().push(b);
            BLK.borrow(cs).borrow_mut().scan(b);
        });
    }

    #[inline]
    fn current_block_end() {
        interrupt::free(|cs| BLK.borrow(cs).borrow_mut().close());
    }

    #[inline]
    fn freeze() {
        interrupt::free(|cs| TRACE.borrow(cs).borrow_mut().frozen = true);
    }
}

impl SagRing {
    /// Arm the ring for the next run: empty, recording, not frozen.
    pub fn arm_next_run() {
        interrupt::free(|cs| {
            let mut t = TRACE.borrow(cs).borrow_mut();
            *t = Trace::empty();
            t.on = true;
            BLK.borrow(cs).borrow_mut().reset();
        });
    }

    /// Stop recording, so the dump cannot race a late block.
    pub fn disarm() {
        interrupt::free(|cs| TRACE.borrow(cs).borrow_mut().on = false);
    }

    /// Read the ring.
    ///
    /// **Not inside `interrupt::free`** (E269). The caller emits ~1280 lines
    /// through a 115 200-baud link with a blocking flush per line, so masking
    /// for the closure held PRIMASK for roughly **2.7 seconds**. Nothing was
    /// bought by it: [`Self::disarm`] runs first and clears `on`, so no writer
    /// remains, and the bridge is already off. A mask that long is also the
    /// one thing this campaign's own notes say never to do while the guard's
    /// tick-gap threshold is 200 µs.
    ///
    /// The borrow is still taken through the `Mutex`, which needs a token, so
    /// this takes the shortest possible critical section to obtain the
    /// reference and emits outside it.
    pub fn read<R>(f: impl FnOnce(&Trace) -> R) -> R {
        // SAFETY-equivalent reasoning, no `unsafe` needed: `disarm` has
        // cleared `on`, so `push` returns before touching the ring, and the
        // recorder is foreground-only in any case. The short critical section
        // only satisfies `Mutex`'s token requirement.
        let ptr = interrupt::free(|cs| TRACE.borrow(cs) as *const RefCell<Trace>);
        // SAFETY: `TRACE` is a `'static` and the pointer is derived from it;
        // no writer can run (see above), so the shared borrow is sound.
        let cell = unsafe { &*ptr };
        f(&cell.borrow())
    }
}

/// The ring as text, oldest block first, parsed by `scripts/sag.py`.
///
/// `SAGROW at at_fine pwm_ctr bus_raw pa pb pc bus vref filt_bus filt_vref
/// streak step duty since_zc_us` -- **raw quantities only**: the host computes
/// the margin
/// with the guard's own cross-product, so no division happens here and nothing
/// is rounded twice.
///
/// The header states the fine clock's rate as `fine_hz` and the 16-bit pairing
/// limit as `span16_us`. **They are part of the format, not commentary**: this
/// ring's fine stamp changed from 15.625 ns to 125 ns between campaigns, which
/// reinterprets every recorded delta by 8x, and `scripts/sag.py` refuses a
/// capture that does not declare the rate rather than assuming either one.
pub fn emit(t: &Trace, out: &mut impl crate::report::Sink) {
    out.say("SAGSNAP ");
    out.kv("judged", t.total);
    out.kv("fast_len", t.fast_len as u32);
    out.kv("slow_len", t.slow_len as u32);
    out.kv("slow_every", SLOW_EVERY);
    out.kv("frozen", u32::from(t.frozen));
    out.kv("num", crate::protection::SAG_NUM);
    out.kv("den", crate::protection::SAG_DEN);
    out.kv("streak_to_latch", u32::from(crate::protection::SAG_STREAK));
    out.kv("adc_rail", u32::from(crate::protection::ADC_RAIL));
    // Versioned capture units: the fine clock's rate travels with the rows.
    out.kv("fine_hz", crate::fine::FINE_HZ);
    out.kv("span16_us", crate::fine::SPAN16_US);
    out.kv("row_v", 3);
    out.say(
        "
",
    );
    out.flush();
    let start = t.fast_start();
    let mut k = 0;
    while k < t.fast_len {
        let b = &t.fast[(start + k) % FAST_LEN];
        out.say("SAGROW ");
        out.say_u32(u32::from(b.at));
        out.say(" ");
        out.say_u32(u32::from(b.at_fine));
        out.say(" ");
        out.say_u32(u32::from(b.pwm_ctr));
        out.say(" ");
        out.say_u32(u32::from(b.bus_raw));
        out.say(" ");
        out.say_u32(u32::from(b.phase_a));
        out.say(" ");
        out.say_u32(u32::from(b.phase_b));
        out.say(" ");
        out.say_u32(u32::from(b.phase_c));
        out.say(" ");
        out.say_u32(u32::from(b.bus_mean));
        out.say(" ");
        out.say_u32(u32::from(b.vref_mean));
        out.say(" ");
        out.say_u32(u32::from(b.filt_bus));
        out.say(" ");
        out.say_u32(u32::from(b.filt_vref));
        out.say(" ");
        out.say_u32(u32::from(b.streak));
        out.say(" ");
        out.say_u32(u32::from(b.step));
        out.say(" ");
        out.say_u32(u32::from(b.duty_tenths));
        out.say(" ");
        out.say_u32(u32::from(b.since_zc_us));
        out.say(
            "
",
        );
        out.flush();
        k += 1;
    }
    // The decimated history: how the reference got where it was. Four columns,
    // because that is all this question needs.
    let start = t.slow_start();
    let mut k = 0;
    while k < t.slow_len {
        let r = &t.slow[(start + k) % SLOW_LEN];
        out.say("SAGSLOW ");
        out.say_u32(u32::from(r.bus_mean));
        out.say(" ");
        out.say_u32(u32::from(r.vref_mean));
        out.say(" ");
        out.say_u32(u32::from(r.filt_bus));
        out.say(" ");
        out.say_u32(u32::from(r.filt_vref));
        out.say(
            "
",
        );
        out.flush();
        k += 1;
    }
    out.say(
        "SAGEND
",
    );
    out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(at: u16, bus: u16) -> Block {
        Block {
            at,
            bus_mean: bus,
            vref_mean: 1500,
            filt_bus: 1200,
            filt_vref: 1500,
            ..Block::default()
        }
    }

    #[test]
    fn the_ring_keeps_the_last_blocks_and_says_how_many_it_dropped() {
        let mut t = Trace::empty();
        t.on = true;
        for i in 0..(FAST_LEN as u16 + 10) {
            t.push(&block(i, 1200));
        }
        assert_eq!(t.fast_len, FAST_LEN);
        assert_eq!(t.total as usize, FAST_LEN + 10);
        // The oldest kept block is the 11th pushed, and the newest is the last.
        assert_eq!(t.fast[t.fast_start()].at, 10);
        let newest = if t.fast_start() == 0 {
            FAST_LEN - 1
        } else {
            t.fast_start() - 1
        };
        assert_eq!(t.fast[newest].at, FAST_LEN as u16 + 9);
    }

    #[test]
    fn a_freeze_keeps_the_pre_trip_window_and_nothing_after_it() {
        let mut t = Trace::empty();
        t.on = true;
        for i in 0..20 {
            t.push(&block(i, 1200));
        }
        t.frozen = true;
        for i in 100..120 {
            t.push(&block(i, 900));
        }
        assert_eq!(t.fast_len, 20, "nothing after the freeze is recorded");
        assert_eq!(t.total, 20);
        assert!(t.fast[..20].iter().all(|b| b.bus_mean == 1200));
    }

    #[test]
    fn a_disarmed_ring_records_nothing() {
        let mut t = Trace::empty();
        for i in 0..5 {
            t.push(&block(i, 1200));
        }
        assert_eq!(t.fast_len, 0);
        assert_eq!(t.total, 0);
    }

    #[test]
    fn the_index_wraps_at_the_end_and_nowhere_else() {
        assert_eq!(Trace::bump(0, FAST_LEN), 1);
        assert_eq!(Trace::bump(FAST_LEN - 2, FAST_LEN), FAST_LEN - 1);
        assert_eq!(Trace::bump(FAST_LEN - 1, FAST_LEN), 0);
    }

    /// **The host's margin must agree with the guard itself**, so this drives
    /// `protection::FastBusSag::observe` rather than re-implementing its
    /// predicate: the review of E175 pointed out that a test which restates
    /// the comparison cannot catch a firmware/host divergence, which is the
    /// only thing it was there for.
    #[test]
    fn the_hosts_margin_agrees_with_the_guard_itself() {
        use crate::protection::{BusReference, FastBusSag, SAG_DEN, SAG_NUM};

        // The host's rule, exactly as `scripts/sag.py` computes it.
        let host_low = |bus: u32, vref: u32, fb: u32, fv: u32| {
            let lhs = f64::from(bus * fv * SAG_DEN);
            let rhs = f64::from(fb * vref * SAG_NUM);
            1000.0 * lhs / rhs < 1000.0
        };
        // A guard primed at 1200/1500, judged on its very first sample so the
        // reference is exactly the priming value.
        for (bus, vref) in [(1140u16, 1500u16), (1139, 1500), (1201, 1500), (1139, 1425)] {
            let mut g = FastBusSag::new(BusReference { bus: 1200, vref: 1500 });
            let (fb, fv) = g.filtered();
            let fired = g.observe(bus, vref).is_some();
            let guard_low = g.streak() > 0 || fired;
            assert_eq!(
                guard_low,
                host_low(u32::from(bus), u32::from(vref), u32::from(fb), u32::from(fv)),
                "bus={bus} vref={vref} fb={fb} fv={fv}"
            );
        }
    }

    /// The fail-closed branch the host script must also reproduce: a VREF of
    /// zero or at the rail latches at once, whatever the ratio would say.
    #[test]
    fn a_bad_vref_latches_whatever_the_ratio_says() {
        use crate::protection::{ADC_RAIL, BusReference, FastBusSag};

        for vref in [0u16, ADC_RAIL, ADC_RAIL + 1] {
            let mut g = FastBusSag::new(BusReference { bus: 1200, vref: 1500 });
            assert!(g.observe(1200, vref).is_some(), "vref={vref} must latch");
            assert!(g.tripped());
        }
    }
}
