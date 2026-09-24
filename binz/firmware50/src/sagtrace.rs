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
//! * [`FAST_LEN`] judgements at full rate — ~52 ms — for the shape of the dip
//!   that trips the guard;
//! * [`SLOW_LEN`] judgements decimated by [`SLOW_EVERY`] — ~3.3 s, i.e. eight
//!   filter time constants — for **how the reference got where it was**, which
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
//! `FAST_LEN` x 16 B + `SLOW_LEN` x 8 B = 16 384 B, which the entry states against the
//! part's 36 KB. This is diagnostic evidence and never qualifies another
//! image.

use core::cell::RefCell;

use cortex_m::interrupt::{self, Mutex};

/// Full-rate judgements kept: ~52 ms at the measured 9.8 kHz.
pub const FAST_LEN: usize = 512;
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
    /// The raw clock when the block was judged.
    pub at: u16,
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
        if i + 1 == len {
            0
        } else {
            i + 1
        }
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
        if self.fast_len == FAST_LEN {
            self.fast_next
        } else {
            0
        }
    }

    pub const fn slow_start(&self) -> usize {
        if self.slow_len == SLOW_LEN {
            self.slow_next
        } else {
            0
        }
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

impl SagLog for SagRing {
    const ON: bool = true;

    #[inline]
    fn block(b: &Block) {
        interrupt::free(|cs| TRACE.borrow(cs).borrow_mut().push(b));
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
        });
    }

    /// Stop recording, so the dump cannot race a late block.
    pub fn disarm() {
        interrupt::free(|cs| TRACE.borrow(cs).borrow_mut().on = false);
    }

    /// Read the ring, interrupts masked for the closure.
    pub fn read<R>(f: impl FnOnce(&Trace) -> R) -> R {
        interrupt::free(|cs| f(&TRACE.borrow(cs).borrow()))
    }
}

/// The ring as text, oldest block first, parsed by `scripts/sag.py`.
///
/// `SAGROW at bus vref filt_bus filt_vref streak step duty since_com_us` --
/// **raw quantities only**: the host computes the margin with the guard's own
/// cross-product, so no division happens here and nothing is rounded twice.
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
        use crate::protection::{BusReference, FastBusSag, ADC_RAIL};

        for vref in [0u16, ADC_RAIL, ADC_RAIL + 1] {
            let mut g = FastBusSag::new(BusReference { bus: 1200, vref: 1500 });
            assert!(g.observe(1200, vref).is_some(), "vref={vref} must latch");
            assert!(g.tripped());
        }
    }
}
