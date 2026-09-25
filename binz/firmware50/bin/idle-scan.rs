//! **The bridge-off control for the raw-depth observer** (E291/E292).
//!
//! E284 built a raw-scan bus-depth observer on the premise that *every run,
//! pass or fail, takes a raw scan past the sharp guard's 5% trip line*, and
//! concluded that depth therefore cannot discriminate and duration must. The
//! dual review of E284-E289 showed the premise is probably measuring the ADC
//! rather than the rail:
//!
//! * over 494 corpus runs `filt_bus - bus_min` is **66-146 codes, median 91**,
//!   a tight bounded distribution with a hard floor;
//! * `corr(depth, hold_ma) = -0.204` -- current changes **40x** across the
//!   corpus and the excursion gets **shallower**, which no load-induced rail
//!   event can do;
//! * the *mean* observer (`dep0`) tracks load across four orders of magnitude
//!   over the same rungs, so the instrument is not blind in general -- only the
//!   raw bins are flat;
//! * `raw*_run` sits at the independent-Bernoulli prediction in all four bins
//!   (0.68 expected, 1 observed at the 950 bin) while `dep0_run` exceeds its own
//!   prediction tenfold, i.e. the raw excursions show **zero clustering**.
//!
//! This binary settles it by removing the load. **It never energises the
//! bridge**: `safe_off` runs before anything else and the gate capability is
//! never taken, so there is no code path here that can set `MOE`.
//!
//! # Two phases, because "bridge off" is two different controls
//!
//! The review proposed one 90 s pass with `MOE` clear and `EN` low. That
//! conflates two conditions, and the DRV8304's `ENABLE` *biases its
//! amplifiers* -- so `EN` low changes the analog environment as well as the
//! load. Both are measured here, `IDLE_PHASE_MS` each, in one image:
//!
//! | phase | `EN` | bridge | what it isolates |
//! |---|---|---|---|
//! | **A** | low | off | the ADC, DMA and divider alone |
//! | **B** | high | off (never switched) | a driving run's analog environment, minus switching |
//!
//! Discrimination, stated before the run:
//!
//! * high in **both** -> the excursions are the sampling path itself;
//! * high in **B only** -> driver-amplifier bias coupling;
//! * low in **both** -> the excursions genuinely require switching or load, and
//!   the negative load correlation needs another explanation.
//!
//! Both phases feed the **same** `BusDepth` code the run uses, against the
//! **same** reference the guard uses (`FastBusSag::filtered`, an EWMA seeded
//! from the pre-phase baseline), so a difference cannot come from the
//! arithmetic or the reference.
//!
//! # And the tear count, which was unmeasurable before
//!
//! `hw::adc` argues its unsynchronised DMA snapshot is coherent because "the
//! DMA does not rewrite until the next TIM6 trigger, ~88 us after the
//! transfer-complete interrupt". Measured `loop_gap_max_us` is **140-192 us in
//! every capture** -- over double that window -- but the report keeps only the
//! *maximum*, so the **number** of gaps past it has never been counted. A
//! straddling snapshot mixes one cycle's bus with the next cycle's VREFINT,
//! which is exactly a one-scan "bus low, vref normal" reading. `gap_over` below
//! counts them, so the mechanism can be compared against `raw1_n` directly
//! instead of argued about.

#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(
    clippy::borrow_as_ptr,
    clippy::cast_ptr_alignment,
    clippy::missing_safety_doc,
    clippy::multiple_unsafe_ops_per_block,
    clippy::ptr_as_ptr,
    clippy::ptr_cast_constness,
    clippy::transmute_ptr_to_ptr,
    clippy::undocumented_unsafe_blocks,
    clippy::unnecessary_safety_comment,
    clippy::unnecessary_safety_doc
)]

mod board;

use cortex_m_rt::entry;
use firmware50::bridge::safe_off;
use firmware50::protection::{BusDepth, BusReference, FastBusSag, RailMean};
use firmware50::report::Sink;
use firmware50::roots::Drv8304;
use firmware50::run::Hal;
use stm32g0xx_hal::stm32;

/// Each phase's duration. Two of these plus the baselines stay well inside the
/// 90 s a driving run takes, so the scan counts are directly comparable.
const IDLE_PHASE_MS: u32 = 45_000;

/// Scans averaged into each phase's own baseline before observing begins. The
/// guard's EWMA is seeded from this, exactly as a run seeds it.
const BASELINE_SCANS: u32 = 2_048;

/// The no-rewrite window `hw::adc` claims for its DMA snapshot, in µs. A
/// foreground gap longer than this means the DMA re-triggered while the reader
/// was between iterations, so the snapshot it then reads may straddle two scans.
const SNAPSHOT_WINDOW_US: u32 = 88;

/// One phase's measurement. Field names match the run's report so the two can
/// be compared without a translation table.
struct Phase {
    scans: u32,
    raw: BusDepth,
    mean: BusDepth,
    bus_min: u16,
    bus_max: u16,
    /// Foreground passes whose gap exceeded [`SNAPSHOT_WINDOW_US`].
    gap_over: u32,
    gap_max_us: u32,
    /// Did the sharp guard's own comparison latch during this phase? With the
    /// bridge off it must not, and if it does that is the headline result, not
    /// a reason to stop -- so the verdict is recorded here and the loop runs on.
    tripped: bool,
    ref_bus: u16,
    ref_vref: u16,
    filt_bus: u16,
    filt_vref: u16,
}

/// Average `BASELINE_SCANS` fresh scans into a reference, the way a run does.
fn baseline(board: &mut board::Board) -> BusReference {
    let (mut bus, mut vref, mut n) = (0u64, 0u64, 0u32);
    while n < BASELINE_SCANS {
        board.tick_clock();
        board.tx_drain();
        if board.adc_due() {
            if let Some(s) = board.scan() {
                bus += u64::from(s.bus);
                vref += u64::from(s.vref);
                n += 1;
            }
        }
    }
    BusReference {
        bus: (bus / u64::from(n)) as u16,
        vref: (vref / u64::from(n)) as u16,
    }
}

/// Scan for `IDLE_PHASE_MS`, observing but never driving.
fn measure(board: &mut board::Board) -> Phase {
    let reference = baseline(board);
    // The guard's own filter, seeded from the baseline exactly as a run seeds
    // it, so `filtered()` below is the line `FastBusSag` would have judged.
    let mut sag = FastBusSag::new(reference);
    let mut rail = RailMean::new();
    let mut p = Phase {
        scans: 0,
        raw: BusDepth::new_raw(),
        mean: BusDepth::new(),
        bus_min: u16::MAX,
        bus_max: 0,
        gap_over: 0,
        gap_max_us: 0,
        tripped: false,
        ref_bus: reference.bus,
        ref_vref: reference.vref,
        filt_bus: reference.bus,
        filt_vref: reference.vref,
    };
    let start = board.tick_clock();
    let mut last = start;
    while board.tick_clock().wrapping_sub(start) < IDLE_PHASE_MS * 1_000 {
        let now = board.tick_clock();
        let gap = now.wrapping_sub(last);
        last = now;
        if gap > p.gap_max_us {
            p.gap_max_us = gap;
        }
        if gap > SNAPSHOT_WINDOW_US {
            p.gap_over = p.gap_over.saturating_add(1);
        }
        board.tx_drain();
        if !board.adc_due() {
            continue;
        }
        let Some(s) = board.scan() else { continue };
        p.scans = p.scans.saturating_add(1);
        p.bus_min = p.bus_min.min(s.bus);
        p.bus_max = p.bus_max.max(s.bus);
        rail.feed(s.bus, s.vref);
        if !rail.ready() {
            continue;
        }
        // Read the filter **before** the guard updates it, as the run does, so
        // the reference is the one this scan would have been judged against.
        let (fb, fv) = sag.filtered();
        p.filt_bus = fb;
        p.filt_vref = fv;
        p.mean.observe(rail.bus_mean(), rail.vref_mean(), reference.bus, reference.vref);
        p.raw.observe(s.bus, s.vref, fb, fv);
        // The verdict is recorded, not discarded and not acted on: with the
        // bridge off a trip would be a measurement about the instrument, and
        // this control must not stop early on one.
        if sag.observe(rail.bus_mean(), rail.vref_mean()).is_some() {
            p.tripped = true;
        }
    }
    p
}

fn emit(board: &mut board::Board, tag: &str, p: &Phase) {
    board.say("IDLESCAN phase=");
    board.say(tag);
    board.kv(" scans", p.scans);
    board.kv(" ms", IDLE_PHASE_MS);
    board.kv(" ref_bus", u32::from(p.ref_bus));
    board.kv(" ref_vref", u32::from(p.ref_vref));
    board.kv(" filt_bus", u32::from(p.filt_bus));
    board.kv(" filt_vref", u32::from(p.filt_vref));
    board.kv(" bus_min", u32::from(p.bus_min));
    board.kv(" bus_max", u32::from(p.bus_max));
    board.kv(" sag_tripped", u32::from(p.tripped));
    board.kv(" gap_max_us", p.gap_max_us);
    board.kv(" gap_over_88us", p.gap_over);
    board.flush();
    // Same field names and order as `BEMFCURRENT`, so the comparison against a
    // driving run is a diff and not a translation.
    let mut i = 0;
    while i < 4 {
        board.say(" raw");
        board.say_u32(i as u32);
        board.say("_pm=");
        board.say_u32(p.raw.fraction(i));
        board.say(" raw");
        board.say_u32(i as u32);
        board.say("_n=");
        board.say_u32(p.raw.below(i));
        board.say(" raw");
        board.say_u32(i as u32);
        board.say("_run=");
        board.say_u32(p.raw.longest(i));
        i += 1;
    }
    let mut i = 0;
    while i < 4 {
        board.say(" dep");
        board.say_u32(i as u32);
        board.say("_pm=");
        board.say_u32(p.mean.fraction(i));
        board.say(" dep");
        board.say_u32(i as u32);
        board.say("_n=");
        board.say_u32(p.mean.below(i));
        board.say(" dep");
        board.say_u32(i as u32);
        board.say("_run=");
        board.say_u32(p.mean.longest(i));
        i += 1;
    }
    board.say("\r\n");
    board.tx_flush();
}

#[entry]
fn main() -> ! {
    let Some((mut board, adc_ok)) = stm32::Peripherals::take().and_then(board::init) else {
        loop {
            cortex_m::asm::nop();
        }
    };
    // Before anything else, and never undone: this image has no path that
    // takes the gate capability, so the bridge cannot be driven from here.
    safe_off(&mut Drv8304);
    board::banner(&mut board, adc_ok);
    if !adc_ok {
        board.say("FATAL adc_init_failed -- the control measures nothing\r\n");
        loop {
            board.tick_clock();
            board.tx_drain();
        }
    }
    board.say("IDLESCAN begin -- bridge never energised; two phases\r\n");
    board.tx_flush();
    board.resync_adc();

    // Phase A: ENABLE low. The driver is asleep and its amplifiers unbiased,
    // so this is the ADC, the DMA and the bus divider on their own.
    board.enable(false);
    let a = measure(&mut board);
    emit(&mut board, "A_en_low", &a);

    // Phase B: ENABLE high. The driver is awake and biasing its shunt
    // amplifiers -- a driving run's analog environment without any switching.
    // `MOE` stays clear and no compare is ever written.
    board.enable(true);
    let b = measure(&mut board);
    emit(&mut board, "B_en_high", &b);

    board.enable(false);
    safe_off(&mut Drv8304);
    let pf = board.preflight();
    board.say("IDLESCAN done");
    board.kv(" moe", u32::from(pf.moe));
    board.kv(" gates_low", u32::from(pf.gates_low));
    board.kv(" en", u32::from(pf.enable));
    board.kv(" nfault", u32::from(pf.nfault_high));
    board.kv(" ccr1", pf.ccr[0]);
    board.kv(" ccr2", pf.ccr[1]);
    board.kv(" ccr3", pf.ccr[2]);
    board.kv(" preflight_passed", u32::from(pf.passed()));
    board.say("\r\n");
    board.tx_flush();
    loop {
        board.tick_clock();
        board.tx_drain();
    }
}
