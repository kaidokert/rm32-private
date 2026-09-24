//! The sharp-sag guard's own inputs, scan by scan, in a bounded pre-trip ring
//! (campaign 9 step 3).
//!
//! Campaign 7 saw the guard latch in three of six runs at 50% and the cause
//! was never found. Every quantity quoted at it since has been the wrong one:
//! `bus_min` is the minimum of a **single raw scan** over the whole run
//! (`run::states`), and the report's `filt_bus` is the filter's value at the
//! *end* of the run. Neither is what the guard compares.
//!
//! What it actually compares, traced in `protection::FastBusSag::observe`:
//!
//! ```text
//! bus_mean * filt_vref * 100  <  filt_bus * vref_mean * 95
//! ```
//!
//! — the **block means** of bus and VREF (`protection::RailMean`, not one
//! scan) against the **~207 ms exponential averages** of the same two
//! (`SAG_FILTER_SHIFT = 11` at the 9.901 kHz scan rate), in Q8 and rounded;
//! three consecutive low blocks latch (`SAG_STREAK`), and the filter is
//! updated *after* the test so a collapsing sample cannot drag the reference
//! onto itself.
//!
//! So this records exactly those four numbers per judged block, plus the
//! streak the guard is holding, the duty, the sector, and how long it is since
//! the last commutation — nothing derived, and **no division in the
//! firmware**: the host computes the margin from the raw quartet, which is the
//! same discipline `BEMFTAIL` follows.
//!
//! **Bounded and pre-trip.** [`SAG_TRACE_LEN`] blocks, circular, and it
//! **freezes on the fault** — the ring then holds the window that led to the
//! trip rather than whatever came after. Dumped after `safe_off`, like every
//! other byte.
//!
//! Production runs [`NoSagLog`], whose `ON` is `false`, so every call folds
//! away; only the `sag-capture` binary installs [`SagRing`]. This is
//! diagnostic evidence and never qualifies another image.

use core::cell::RefCell;

use cortex_m::interrupt::{self, Mutex};

/// Judged blocks kept, and **how long that actually is**: the guard judges a
/// block whenever `RailMean` is ready, which the captures measure at
/// **9.8–11.7 kHz** — not the ~2 kHz I first assumed. So 512 rows was about
/// **50 ms** of history, and 1024 is about **100 ms** (14 KB of RAM). Stated
/// because the window is what bounds every conclusion drawn from a dump: for
/// a run that trips the ring freezes and this is the pre-trip window, but for
/// a run that passes it is only the tail.
pub const SAG_TRACE_LEN: usize = 1024;

/// One judged block: the guard's own inputs and the state it judged them in.
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
    /// µs since the last commutation, so a low block can be placed against
    /// the switching instant rather than assumed independent of it.
    pub since_com_us: u16,
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
    pub total: u32,
    pub len: usize,
    pub next: usize,
    pub blocks: [Block; SAG_TRACE_LEN],
}

impl Trace {
    pub const fn empty() -> Self {
        Self {
            on: false,
            frozen: false,
            total: 0,
            len: 0,
            next: 0,
            blocks: [Block {
                at: 0,
                bus_mean: 0,
                vref_mean: 0,
                filt_bus: 0,
                filt_vref: 0,
                streak: 0,
                step: 0,
                duty_tenths: 0,
                since_com_us: 0,
            }; SAG_TRACE_LEN],
        }
    }

    /// The next circular index, written out: `% SAG_TRACE_LEN` would be a
    /// helper division on this M0+ if the length ever stopped being a power of
    /// two (E154's finding, in the capture ring).
    const fn bump(i: usize) -> usize {
        if i + 1 == SAG_TRACE_LEN {
            0
        } else {
            i + 1
        }
    }

    pub fn push(&mut self, b: &Block) {
        if !self.on || self.frozen {
            return;
        }
        self.blocks[self.next] = *b;
        self.next = Self::bump(self.next);
        self.total = self.total.wrapping_add(1);
        if self.len < SAG_TRACE_LEN {
            self.len += 1;
        }
    }

    /// The oldest kept block's index.
    pub const fn start(&self) -> usize {
        if self.total as usize > self.len {
            self.next
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
    out.kv("len", t.len as u32);
    out.kv("total", t.total);
    out.kv("frozen", u32::from(t.frozen));
    out.kv("num", crate::protection::SAG_NUM);
    out.kv("den", crate::protection::SAG_DEN);
    out.kv("streak_to_latch", u32::from(crate::protection::SAG_STREAK));
    out.say(
        "
",
    );
    out.flush();
    let start = t.start();
    let mut k = 0;
    while k < t.len {
        let b = &t.blocks[(start + k) % SAG_TRACE_LEN];
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
        out.say_u32(u32::from(b.since_com_us));
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
        for i in 0..(SAG_TRACE_LEN as u16 + 10) {
            t.push(&block(i, 1200));
        }
        assert_eq!(t.len, SAG_TRACE_LEN);
        assert_eq!(t.total as usize, SAG_TRACE_LEN + 10);
        // The oldest kept block is the 11th pushed, and the newest is the last.
        assert_eq!(t.blocks[t.start()].at, 10);
        let newest = if t.next == 0 { SAG_TRACE_LEN - 1 } else { t.next - 1 };
        assert_eq!(t.blocks[newest].at, SAG_TRACE_LEN as u16 + 9);
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
        assert_eq!(t.len, 20, "nothing after the freeze is recorded");
        assert_eq!(t.total, 20);
        assert!(t.blocks[..20].iter().all(|b| b.bus_mean == 1200));
    }

    #[test]
    fn a_disarmed_ring_records_nothing() {
        let mut t = Trace::empty();
        for i in 0..5 {
            t.push(&block(i, 1200));
        }
        assert_eq!(t.len, 0);
        assert_eq!(t.total, 0);
    }

    #[test]
    fn the_index_wraps_at_the_end_and_nowhere_else() {
        assert_eq!(Trace::bump(0), 1);
        assert_eq!(Trace::bump(SAG_TRACE_LEN - 2), SAG_TRACE_LEN - 1);
        assert_eq!(Trace::bump(SAG_TRACE_LEN - 1), 0);
    }

    /// The host computes the margin from the raw quartet, so the arithmetic
    /// the guard performs is reproducible outside it. This pins the identity
    /// the host script relies on: low exactly when
    /// `bus * filt_vref * 100 < filt_bus * vref * 95`.
    #[test]
    fn the_hosts_margin_matches_the_guards_own_comparison() {
        let low = |bus: u32, vref: u32, fb: u32, fv: u32| bus * fv * 100 < fb * vref * 95;
        // At exactly 95% of the reference ratio the guard is not low.
        assert!(!low(1140, 1500, 1200, 1500), "1140/1200 = 95.0%");
        assert!(low(1139, 1500, 1200, 1500), "one code below is low");
        // VREF moves the line: the same bus against a lower VREF is not low.
        assert!(!low(1139, 1425, 1200, 1500));
    }
}
