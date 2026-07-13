//! Bench motor tester **v2**: same as `motor_tester.rs` but the soft-UART
//! RX is dropped in favour of a second hardware UART, and both directions
//! run at 115200.
//!
//! - **RX**: hardware **USART2 on PA2** (the `S` pin of the J3 header).
//!   PA2's AF7 function is USART2_TX, so we set `USART2.CR2.SWAP` to route
//!   the receiver onto it; the transmitter stays disabled. This frees PA0
//!   (`HSE` pin) and removes the LPTIM1 + EXTI0 soft-UART machinery and
//!   its constant 4×baud sample-ISR load.
//! - **TX**: hardware USART1 on **PB6** (`TX1` pin), half-duplex
//!   open-drain — unchanged from `motor_tester.rs` except the baud.
//! - **Motor PWM**: TIM1 driving HIN1-3 / LIN1-3 via the AM32 pin map
//!   (same as `output_pins.rs`).
//!
//! Wire to host (115200 8N1 both directions):
//! - USB-TTL TX  → S   (PA2, host → ESC, key commands)
//! - USB-TTL RX  → TX1 (PB6, ESC → host, status messages)
//! - GND ↔ G
//!
//! Primary control is **electrical frequency** — that's how an open-loop
//! drive moves the rotor. Amplitude is the secondary knob and is capped
//! to a deliberately conservative ceiling (see `AMP_MAX`) because outrunners
//! melt fast when the voltage you apply isn't being cancelled by back-EMF.
//!
//! Keys (paired like vertical neighbours on QWERTY — top key adds, bottom
//! subtracts):
//! - `d`       : electrical frequency +1 Hz
//! - `c`       : electrical frequency -1 Hz
//! - `f`       : electrical frequency +10 Hz
//! - `v`       : electrical frequency -10 Hz
//! - `a`       : amplitude +1 (capped, see `AMP_MAX`)
//! - `z`       : amplitude -1
//! - `s`       : amplitude +10
//! - `x`       : amplitude -10
//! - `m`       : toggle waveform (sine ↔ 6-step BLDC commutation)
//! - `r`       : reset to six-step, `f = FREQ_START`, `amp = AMP_START` (re-arms if killed)
//! - `q`       : reset to six-step, **f = 50 Hz**, `amp = AMP_START` (re-arms if killed)
//! - `w`       : hard kill — clear `MOE` in BDTR (all FETs off)
//! - `i`       : print ADC reads (PA3 = IN8 current, PA6 = IN11 battery)
//! - `b`       : print COMP2 edge rate (events/sec) and current level
//! - `p`       : cycle which phase is observed on COMP2 (A → B → C → A)
//! - `l`       : dump PWM-sampled COMP2 ring buffer, split per 60° sector
//! - `e`       : dump edge-binned COMP2 buffer (2 revs, 10 µs bins; valid for f ≥ 50 Hz)
//! - `h`       : cycle COMP2 hysteresis level
//! - `k`       : cycle COMP2 EXTI edge selection
//! - `t`       : cycle commutation advance (0/20/40/-40/-20°)
//! - `n`/`N`   : ±1 µs fine adjust of software PWM-edge blanking window
//! - `.`/`,`   : ±10 µs coarse adjust of same

#![no_std]
#![no_main]

use core::cell::RefCell;
use core::fmt::Write;

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;
use heapless::spsc::{Producer, Queue};
use minz::adc_sync;
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2;
use minz::current_adc::SenseAdc;
use minz::hal::pac::interrupt;
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::idle_loop::IdleLoop;
use minz::open_loop::{self, Waveform};
use minz::priority;
use minz::tim1_motor_pwm::{self, max_duty};
use minz::{PWM_FREQUENCY_HZ, tim7_drive};
use portable_atomic::{AtomicBool, AtomicI8, AtomicU8, AtomicU16, AtomicU32, Ordering};
use rtt_target::rprintln;

/// 2 Mbaud is the highest rate where both clock chains are exact:
/// L431 BRR = 80 MHz / 40, FT232R divisor = 3 M × 2/3. Verified
/// byte-perfect with the `u` blast test (64 KiB, zero errors).
/// The FT232R tops out at 3 M, but hitting it from an 80 MHz kernel
/// clock needs 8× oversampling and off-frequency divisors — not
/// worth it for a bench link. 200 kB/s ≈ 4 × 16-bit values per
/// 24 kHz PWM cycle, plenty for streaming dumps.
const BAUD: u32 = 2_000_000;
const RX_BUF_LEN: usize = 32;

/// Amplitude clamps as % of full ARR swing. The cap is a bound on how
/// hard a guarded failure can transiently hit — it is NOT a speed/
/// voltage limiter (see the AMP_MAX note below: under a verified closed
/// loop the cap just sets a BEMF equilibrium that masquerades as a
/// "voltage wall"). The OPEN-LOOP heater risk is the real reason it
/// exists: open-loop sine/six-step drive has no rotor sync and no
/// current limit — at low electrical frequency the applied voltage
/// divides across the milliohm winding resistance and turns straight
/// into copper losses (already cost us one motor on this bench). Under
/// the closed loop the guard stack (OC trip, ZC-starvation, runaway
/// floor, sag kill, desync) is the protection, not this cap.
///
/// Does **not** protect against e.g. duty getting stuck high if the loop
/// hangs, or shoot-through during dead-time misconfiguration — there are
/// other ways to fry the windings that this clamp doesn't cover.
const AMP_MIN: u16 = 0;
// Raised progressively 20 → 25 → 50 (2026-07-08/09). Lesson learned
// the embarrassing way: under a verified closed loop, each amp cap
// just sets a BEMF equilibrium speed that then masquerades as a
// "voltage wall" (we characterized our own 25 % cap's equilibrium
// at ~640 Hz with 76 k-window precision and interrogated the poor
// power supply twice). AM32 runs this hardware to 100 % duty. The
// protection is the guard stack (1.5 A OC trip, ZC-starvation,
// runaway floor, desync), NOT this cap; the cap only bounds how
// hard a guarded failure can transiently hit. Open-loop use above
// ~16 remains a heater risk — mind the `q` key at high amp.
const AMP_MAX: u16 = 75;
/// Bench observation: at 5 V supply this motor refuses to start
/// (synchronise to the commanded field) below ~15 %. Set the default at
/// the empirical floor so the user doesn't have to ramp up after boot
/// just to confirm the loop is alive.
const AMP_START: u16 = 15;

/// Electrical-frequency clamps in Hz. 60 Hz × 7 pole-pairs ≈ 514 RPM mech.
/// Lower bound 1 to avoid divide-by-zero on the step-cycle compute. The
/// boot-time displayed `electrical_hz` is `0` to advertise "we are not
/// driving anything yet"; the first freq key clamps it back up to `1`.
const FREQ_MIN: u32 = 1;
const FREQ_MAX: u32 = 600;
const FREQ_START: u32 = 60;

// Sense calibration (vbat divider, INA gain) lives in
// minz_core::sense — ONE authoritative copy, host-anchored; the
// schematic-vs-bench-cal caveats are documented there and in
// motor_tester.rs history.

type RxQueue = Queue<u8, RX_BUF_LEN>;

/// Main-loop microloop period, expressed in 10 µs SysTick ticks.
/// 100 ticks = 1 ms. Each microloop = one slack spin via
/// `IdleLoop::run_until` + one active-work pass (RX dequeue + key
/// dispatch + TX service). 1 ms is short enough to keep UART RX
/// responsiveness sub-millisecond while still giving the slack
/// counter plenty of fetch_adds to count.
const MICROLOOP_TICKS: u64 = 100;

/// One second of 10 µs SysTick ticks. Both `IdleLoop` calibration
/// and the per-second latch / rate-window snapshot are sized to
/// this — `busy_percentage` requires `calibration_window ==
/// latch_window`.
const SECOND_TICKS: u64 = 100_000;

/// Core cycles per µs / 10 µs at 80 MHz — the DWT.CYCCNT scale
/// factors. CYCCNT increments once per core clock.
const CYC_PER_US: u64 = 80;
const CYC_PER_10US: u64 = 800;

/// DWT.CYCCNT → 64-bit software extension (lever #1b, 2026-07-13).
///
/// The wall clock is now DWT.CYCCNT: a free-running 32-bit counter at
/// the full 80 MHz core clock, single-instruction read, ZERO ISR.
/// SysTick is retired entirely. CYCCNT wraps every 2³²/80e6 ≈ 53.7 s
/// — too short for absolute timestamps — so we extend it to 64 bits:
///
/// - `CYC_HIGH` holds the wrap count (upper 32 bits of a 64-bit cycle
///   count). It is bumped by the **extender** in the 24 kHz TIM1_UP
///   ISR ([`cyc_extend`]), which runs every ~41 µs and therefore
///   cannot miss a 53.7 s wrap.
/// - `CYC_LAST` is the CYCCNT value at the extender's last run.
///
/// A reader ([`now_cyc64`]) self-compensates for a wrap that landed
/// AFTER the extender's last run but before the read: if the current
/// CYCCNT is below `CYC_LAST`, one wrap has occurred that `CYC_HIGH`
/// doesn't yet reflect, so the reader adds it locally. This closes the
/// only race (reader between wrap and catch) WITHOUT a pending-bit —
/// DWT has none, unlike SysTick. Two AtomicU32 (not one AtomicU64:
/// thumbv7em has no native 64-bit atomic) with a double-read of
/// `CYC_HIGH` to reject a torn (HIGH, LAST) pair; `CYC_HIGH` only
/// changes every 53.7 s so the retry essentially never fires.
///
/// The derived [`ticks_1us`]/[`ticks_10us`] therefore keep their
/// clean power-of-two wrap (71 min / 12 h) — every `wrapping_sub`
/// consumer and the MAGPIE wire timestamp work UNCHANGED. Absolute
/// second-scale time (main-loop pacing) reads the full u64 via
/// [`now_10us_64`] so it never wraps in practice.
static CYC_HIGH: AtomicU32 = AtomicU32::new(0);
static CYC_LAST: AtomicU32 = AtomicU32::new(0);

/// Clock monotonicity tripwire — bumped from the main loop whenever
/// the 64-bit DWT clock reads lower than the previous sample. Must
/// stay 0; a non-zero value in `i` means the wrap extension has a
/// bug. Makes a would-be silent time glitch loud.
static CLOCK_BACK: AtomicU32 = AtomicU32::new(0);

/// COMP2 transition counter — incremented in the COMP ISR on every
/// EXTI line-22 edge (both rising and falling).
static COMP_COUNT: AtomicU32 = AtomicU32::new(0);

/// Per-ISR fire counters. Each ISR bumps its counter at entry; the `i`
/// key handler snapshots them with `TICKS_10US` and prints the rate
/// since the previous press. Used to diagnose ISR storms (e.g. COMP
/// thrashing on undriven phases when the motor is off).
static USART2_COUNT: AtomicU32 = AtomicU32::new(0);
static TIM7_COUNT: AtomicU32 = AtomicU32::new(0);
static TIM1_UP_COUNT: AtomicU32 = AtomicU32::new(0);
static TIM1_CC_COUNT: AtomicU32 = AtomicU32::new(0);

/// TIM1_UP update-event MISS detector. The ISR reads DWT.CYCCNT at
/// every entry; a gap of ≥2 PWM periods since the previous entry
/// means update events were coalesced while the ISR couldn't run
/// (blocked by higher-priority work) — each such gap adds
/// `periods − 1` to `TIM1_UP_MISSED`. The confirm rule, GECKO
/// current, and the OC/sag failsafes all live in this ISR, so a
/// missed cycle is a blackout of that machinery. Cumulative counter +
/// max-gap aggregate (µs, swap-reset by the `i` print) per the
/// no-point-sampling bench rule; both shown in `i`.
static TIM1_UP_MISSED: AtomicU32 = AtomicU32::new(0);
static TIM1_UP_LAST_CYC: AtomicU32 = AtomicU32::new(0);
static TIM1_UP_MAXGAP_CYC: AtomicU32 = AtomicU32::new(0);

/// Per-ISR duration sampling (DWT cycles, plain store of the most
/// recent completed pass — the rm32 `*_last_cyc` pattern; a
/// `fetch_max` would retain instrumentation spikes). Shown in `i` as
/// `dur:`. Guard struct stores on Drop so early returns are covered.
static DUR_T1U: AtomicU32 = AtomicU32::new(0);
static DUR_T1CC: AtomicU32 = AtomicU32::new(0);
static DUR_COMP: AtomicU32 = AtomicU32::new(0);
static DUR_TIM7: AtomicU32 = AtomicU32::new(0);
static DUR_LPTIM2: AtomicU32 = AtomicU32::new(0);
static DUR_MAIN: AtomicU32 = AtomicU32::new(0);

struct DurGuard(u32, &'static AtomicU32);
impl DurGuard {
    #[inline(always)]
    fn new(slot: &'static AtomicU32) -> Self {
        Self(cortex_m::peripheral::DWT::cycle_count(), slot)
    }
}
impl Drop for DurGuard {
    #[inline(always)]
    fn drop(&mut self) {
        self.1.store(
            cortex_m::peripheral::DWT::cycle_count().wrapping_sub(self.0),
            Ordering::Relaxed,
        );
    }
}

/// Software PWM-edge blanking window in **microseconds**. `0`
/// disables the gate. Live-tunable via `n`/`N` (±1 µs) and `.`/`,`
/// (±10 µs coarse) keys. Clamped to [0, 50] — one full PWM period
/// at 24 kHz is 41.67 µs; blanking longer than that is meaningless.
static BLANK_US: AtomicU16 = AtomicU16::new(0);

/// Last PWM-edge timestamp in **microseconds** (`ticks_1us()` units).
/// The `TIM1_CC` ISR latches `ticks_1us()` here every time TIM1.CCRx
/// matches (i.e. at every PWM channel transition). The COMP ISR
/// reads this and skips the EDGE_BUF / SECTOR_EDGE_COUNT writes if
/// `(now_us - LAST_PWM_EDGE_US) < BLANK_US`. Both timestamps use
/// `wrapping_sub` for the delta so the u32 wrap (~71 min) is benign.
static LAST_PWM_EDGE_US: AtomicU32 = AtomicU32::new(0);

/// COMP2 transition rate in events/sec, computed in the main loop
/// over rolling ~1-second windows. Read by the `b` key handler.
static COMP_RATE: AtomicU32 = AtomicU32::new(0);

/// AM32-style time-window filter: only edges that fire in the
/// **second half** of the current float sector are counted as valid.
/// The first half is dominated by FET-transition transients and
/// PWM-coupling ringing; real BEMF zero-crossings cluster around the
/// midpoint of the float window once the rotor is somewhat in sync.
static VALID_COMP_COUNT: AtomicU32 = AtomicU32::new(0);
static VALID_COMP_RATE: AtomicU32 = AtomicU32::new(0);

/// `ticks_1us()` captured at the start of the current float window.
/// The COMP ISR uses this with `SECTOR_GATE_US` to gate edge
/// acceptance. Units changed 10 µs → 1 µs (roadmap Phase 1): at the
/// high-speed end a window is ~66-200 µs, so 10 µs granularity was
/// up to 15 % quantization on the gate position.
static SECTOR_START_US: AtomicU32 = AtomicU32::new(0);

/// Gate position in µs from window start — edges earlier than this
/// are rejected. Open loop: half the commanded sector (50 %). Closed
/// loop: 30 % of the measured interval (8 % in re-acquisition).
static SECTOR_GATE_US: AtomicU32 = AtomicU32::new(0);

// Open-loop half-sector gate: minz_core::drive::open_loop_gate_us.

/// Free-running ring buffer of COMP2.VALUE sampled at the TIM1
/// update event (= once per PWM period, 24 kHz). One byte per
/// sample so the ISR is a single store, no shift / mask / clear.
///
/// Byte encoding: `bit 0 = COMP2 value (0 / 1)`, `bits 1..=3 =
/// six-step sector (0..=5)`. The sector is co-recorded so the
/// dump can split the trace into 60° chunks.
///
/// 2048 bytes = ~85.3 ms of history. At f=24 Hz that's ~2.05 full
/// electrical revs; at f=50 Hz it's ~4.3 revs. Sized so the new
/// "last 2 revs aligned to sector 0" dump (see the `L` key handler)
/// can always find three 5→0 sector transitions in the buffer down
/// to the bench-floor f of ~25 Hz. Length is a power of two so the
/// ring-buffer index wrap is one AND.
const PWM_SAMPLE_LEN: usize = 2048;
const PWM_SAMPLE_MASK: u32 = (PWM_SAMPLE_LEN - 1) as u32;
static PWM_SAMPLE_BUF: [AtomicU8; PWM_SAMPLE_LEN] = [const { AtomicU8::new(0) }; PWM_SAMPLE_LEN];
static PWM_SAMPLE_IDX: AtomicU32 = AtomicU32::new(0);

/// Per-cycle context rings for the GECKO-scope hybrid: the injected
/// mid-ON `(A, B, current)` triplet plus the free-run current-ring
/// head at each cycle wrap, all captured in `TIM1_UP` at the SAME
/// index/cadence as `PWM_SAMPLE_BUF` (status). The `j` WAXWING dump
/// reads A/B/current from here (the old regular-DMA analog ring is
/// now repurposed for the oversampled free-run current). `CTX_MARK`
/// maps each cycle onto its slice of `adc_sync::CUR_RING` for the
/// oversampled `J` autopsy.
static CTX_A: [AtomicU16; PWM_SAMPLE_LEN] = [const { AtomicU16::new(0) }; PWM_SAMPLE_LEN];
static CTX_B: [AtomicU16; PWM_SAMPLE_LEN] = [const { AtomicU16::new(0) }; PWM_SAMPLE_LEN];
static CTX_I: [AtomicU16; PWM_SAMPLE_LEN] = [const { AtomicU16::new(0) }; PWM_SAMPLE_LEN];
static CTX_MARK: [AtomicU16; PWM_SAMPLE_LEN] = [const { AtomicU16::new(0) }; PWM_SAMPLE_LEN];

/// Free-run ring head at the previous `TIM1_UP`, so the WAX trigger can
/// scan the ~150 intra-cycle current samples added since last cycle for
/// their PEAK — the mid-ON `i_raw` sees one point per PWM cycle and
/// misses a spike that doesn't land on it. Only used while armed (the
/// free-run oversample is gated on then).
static LAST_CUR_HEAD: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------
// Edge-based COMP2 sampling — captures every COMP EXTI fire by its
// 10 µs timestamp within the current 2-rev window.
// Double-buffered: COMP ISR writes the "active" half; TIM7 ISR
// flips on every **second** rev wrap so the "inactive" half is a
// frozen TWO-rev snapshot the dump reads from. Newly-active half
// is memset clean inside the TIM7 ISR on the flip.
//
// One buffer only — we record "an edge happened in this bin" with
// no direction label. STM32L4's EXTI line 22 (COMP2) has a single
// pending bit shared by rising and falling triggers (no separate
// `RPR`/`FPR` registers like G0/G4), so the only way to infer
// direction from the ISR is to read `COMP2_CSR.VALUE` afterwards.
// That bit reflects the comparator's *current* state, not the
// state that latched the edge — by the time the ISR enters, a
// short PWM-coupled pulse has often already bounced back. The
// post-bounce read just shows the dwell state and gives a
// misleading direction label, so we don't bother attempting one.
//
// **Valid for f_elec ≥ 50 Hz.** At lower f 2 revs exceeds 4096
// ticks and late events overwrite earlier ones at the same index
// (events wrap around the half modulo 4096). The dump will look
// like garbage in that range; we accept it because 50 Hz is the
// bench floor anyway.
//
// 10 µs resolution matches `TICKS_10US` directly and gives 4× the
// PWM period (41.6 µs) — enough to see commutation transients and
// real BEMF crossings without recording sub-PWM-edge ringing.
//
// Stored as bytes (not bit-packed) to keep the COMP ISR a single
// store. At 50 Hz the per-flip memset cost is ~13 µs in TIM7
// every 40 ms (every 2 revs) — well under 0.1 % CPU.
// ---------------------------------------------------------------

/// Cells per `EDGE_BUF` half. 4096 × 10 µs = 40.96 ms = exactly 2
/// electrical revs at f=49 Hz. At higher f only the leading
/// `2 * rev_ticks` cells are populated; the rest stay zero from the
/// per-flip memset.
const HALF_TICKS: usize = 4096;
const HALF_TICK_MASK: u32 = (HALF_TICKS as u32) - 1;

/// Double-buffered edge presence map. Outer dimension selects the
/// "half" (0 or 1) controlled by [`ACTIVE_HALF`]; inner cell is
/// `0` (no edge captured in this 10 µs window this 2-rev span) or
/// `1` (≥1 edge captured).
static EDGE_BUF: [[AtomicU8; HALF_TICKS]; 2] = [
    [const { AtomicU8::new(0) }; HALF_TICKS],
    [const { AtomicU8::new(0) }; HALF_TICKS],
];

/// Which half of `EDGE_BUF` the COMP ISR writes to. Toggled by the
/// TIM7 ISR on every second electrical rev wrap. The *other* half
/// is the just-completed 2-rev snapshot — that's what the dump
/// reads. The new "last 2 revs aligned to sector 0" dump in the
/// `E` handler relies on the flip happening at exact rev-pair
/// boundaries (which coincide with sector-0 entry because TIM7
/// detects rev wrap when the angle accumulator wraps to 0).
static ACTIVE_HALF: AtomicU8 = AtomicU8::new(0);

/// Tracks position within the current 2-rev pair. `0` = first rev
/// in progress, `1` = second rev in progress. TIM7 XORs this on
/// each rev wrap; only when it goes 1 → 0 (i.e. the second rev
/// just completed) does the half-flip fire.
static REV_PHASE: AtomicU8 = AtomicU8::new(0);

/// 10 µs tick value captured at the start of the current 2-rev
/// window (= the moment of the last half-flip). COMP ISR computes
/// its buffer index as `(TICKS_10US - HALF_START_TICK) & HALF_TICK_MASK`.
/// Updated by TIM7 on each rev-pair completion.
static HALF_START_TICK: AtomicU32 = AtomicU32::new(0);

/// Per-sector start ticks within each `EDGE_BUF` half. 12 entries
/// per half = 6 sectors × 2 revs. TIM7 records the 10 µs tick at
/// every sector entry into the slot `rev_phase * 6 + sector` of the
/// active half. The `E` dump uses these to compute exact cell ranges
/// for each of the 12 sector chunks, instead of dividing
/// `rev_ticks/6` synthetically (which accumulates drift up to ~1
/// sector at high f).
///
/// Slot layout for one half:
///   [0] = start of rev 0 sec 0 (== HALF_START_TICK at the flip)
///   [1] = start of rev 0 sec 1
///   ...
///   [6] = start of rev 1 sec 0
///   ...
///   [11] = start of rev 1 sec 5
/// End of slot 11 (= end of the 2-rev window) is the CURRENT
/// HALF_START_TICK at dump time (= the next flip).
static SECTOR_BOUNDARIES: [[AtomicU32; 12]; 2] = [
    [const { AtomicU32::new(0) }; 12],
    [const { AtomicU32::new(0) }; 12],
];

/// Per-sector total COMP2 edge counts within each `EDGE_BUF` half.
/// Same 12-slot indexing as [`SECTOR_BOUNDARIES`]. Unlike `EDGE_BUF`
/// (which collapses multiple edges in a single 10 µs window to one
/// `#`), this counter sees EVERY EXTI fire — so a sector with bursts
/// of sub-10 µs ringing shows up with a much higher count than a
/// quiet sector with widely-spaced edges, even when their `#`-cell
/// densities look the same. Used by the `e` dump to print
/// `[N events]` alongside each sector line.
static SECTOR_EDGE_COUNT: [[AtomicU32; 12]; 2] = [
    [const { AtomicU32::new(0) }; 12],
    [const { AtomicU32::new(0) }; 12],
];

/// Current six-step sector (0..=5), published by the motor-drive
/// (`TIM7`) ISR and read by the COMP-sample (`TIM1_UP_TIM16`) ISR.
/// Always reflects the *current* angle regardless of waveform —
/// even in sine mode we record sector boundaries so the PWM-sample
/// dump still has a 60° partitioning to chunk on.
static CURRENT_SECTOR: AtomicU8 = AtomicU8::new(0);

/// When `true`, the TIM7 ISR skips `ACTIVE_HALF` flips and
/// `SECTOR_BOUNDARIES` updates so the frozen half stays intact.
/// Set / cleared by the `E` key. Motor keeps running in either state.
static EDGE_DUMP_FREEZE: AtomicBool = AtomicBool::new(false);

/// PB3 scope-trigger pin state. Toggled by the TIM7 ISR on every
/// electrical revolution so the output square wave runs at frequency F.
static PB3_LEVEL: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------
// Motor-drive shared state — written by main on key events, read by
// the TIM7 ISR each tick. ISR-driven motor commutation means main
// never touches CCRs / MODER directly; it just publishes intent.
// ---------------------------------------------------------------

/// `true` → TIM7 ISR writes new duties / commutation each tick.
/// `false` → ISR returns early (PWM still runs, but no fresh CCRs).
/// `w` (kill) clears this; `r` / `q` (reset) sets it.
static MOTOR_ENABLED: AtomicBool = AtomicBool::new(true);

/// `true` → six-step (`tim1_motor_pwm::set_six_step`), one phase
/// floats, BEMF window is meaningful. `false` → sine
/// (`tim1_motor_pwm::set_duties`), all three phases driven, no
/// BEMF window. Toggled by `m`.
static SIX_STEP_MODE: AtomicBool = AtomicBool::new(true);

/// Electrical angle accumulator in 16.16 fixed point modulo
/// `360 << 16`. Advanced by `ANGLE_INC` each TIM7 ISR fire. The
/// integer part `(ANGLE_ACCUM >> 16)` is the current electrical
/// angle in degrees (0..=359).
static ANGLE_ACCUM: AtomicU32 = AtomicU32::new(0);

/// Per-ISR-fire angle increment in 16.16 fixed point.
/// `ANGLE_INC = (360 << 16) × electrical_hz / DRIVE_HZ`.
/// Recomputed by main on every `f` / `v` / `d` / `c` key.
static ANGLE_INC: AtomicU32 = AtomicU32::new(0);

/// Amplitude as percent of ARR, 0..=`AMP_MAX`. Read each tick.
/// APPLIED value — slewed by TIM7 toward `AMP_TARGET_PCT` at
/// 1 %/50 ms so throttle changes never step (item 5: step
/// transients at speed tripped the current envelope).
static AMPLITUDE_PCT: AtomicU8 = AtomicU8::new(0);
/// Where the keys want the amplitude to be.
static AMP_TARGET_PCT: AtomicU8 = AtomicU8::new(0);

/// Bitfield: bit `s` set iff sector `s` is a float window for the
/// currently observed phase. Used by the ISR to decide whether to
/// unmask EXTI / latch sector-start state for the COMP filter.
///
/// Updated by main whenever the observed phase changes (`p` key) or
/// when six-step mode is entered/left. Phase ↔ float-sector mapping
/// follows the textbook 6-step BLDC convention (see `comp2::ObservedPhase`):
///   Phase A (PA4): sectors 2, 5 → 0b00100100
///   Phase B (PA5): sectors 1, 4 → 0b00010010
///   Phase C (PB7): sectors 0, 3 → 0b00001001
static FLOAT_SECTOR_MASK: AtomicU8 = AtomicU8::new(0b00100100);

/// Tracks whether the previous TIM7 tick was inside the observed
/// phase's float window, so we only mask/unmask EXTI on transitions.
static WAS_IN_FLOAT_SECTOR: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------
// MAGPIE — per-float-window record capture + binary streaming.
//
// The COMP ISR maintains three per-window accumulators (reset by
// TIM7 at each sector entry); TIM7 packages the completed window
// into a `WindowRec` and hands it to main through an SPSC queue;
// main encodes 16-byte framed records into the FERRET TX ring when
// streaming is on (`g` key). One record per sector per electrical
// rev — 3.6 k records/s at f=600 → 57.6 kB/s, ~29 % of the 2 M link.
// ---------------------------------------------------------------

/// Latest PWM-synchronous current sample (raw 12-bit, INA180 via
/// ADC1 ch8), harvested once per PWM cycle by the TIM1_UP ISR.
static LAST_I_RAW: AtomicU16 = AtomicU16::new(0);
/// Per-window current accumulators. TIM1_UP writes; TIM7 snapshots
/// and resets them in the same `free` block as the COMP accumulators.
static WINDOW_I_SUM: AtomicU32 = AtomicU32::new(0);
static WINDOW_I_N: AtomicU32 = AtomicU32::new(0);
static WINDOW_I_MIN: AtomicU16 = AtomicU16::new(0x0FFF);
static WINDOW_I_MAX: AtomicU16 = AtomicU16::new(0);

/// Overcurrent failsafe (firmware-side, host-independent). The
/// TIM1_UP ISR accumulates the per-PWM-cycle current samples over
/// `1 << I_TRIP_SHIFT` PWM cycles (4096 ≈ 85 ms @ 48 kHz); if the
/// window average exceeds [`I_TRIP_RAW`] while the drive is armed,
/// the ISR itself kills the output (same actions as the `w` key) and
/// raises [`OC_TRIPPED`] so main can report it. Averaging makes it a
/// stall/heating guard, deliberately blind to sub-window spikes.
///
// Overcurrent thresholds live in minz_core::guards::overcurrent
// (host-tested; the 2.0 A open-loop / 4 A CL history in its docs).
// Window size: minz_core::guards::TRIP_WINDOW_SHIFT (2^11 cycles).
static I_TRIP_ACC: AtomicU32 = AtomicU32::new(0);
static I_TRIP_CNT: AtomicU32 = AtomicU32::new(0);
/// ANALOG BLACK BOX (`J` key arms, one-shot): when armed, a single
/// PWM-cycle shunt sample above [`WAX_TRIG_RAW`] freezes the WAXWING
/// ring IN THE ISR — preserving ~42 ms of pre-trigger ground truth
/// (both BEMF phase voltages, current, raw comparator bit, every
/// PWM cycle) — and main auto-dumps it. Built because every
/// firmware-side "lock" indicator is self-referential (gate and
/// polarity positioned by our own schedule): only the wire can
/// arbitrate what the current-spike events actually are.
static WAX_TRIG_ARMED: AtomicBool = AtomicBool::new(false);
static WAX_TRIGGERED: AtomicBool = AtomicBool::new(false);
/// ~4 A instantaneous (raw 90 ≈ 2.4 A → ~37.5 raw/A). Raised from 90
/// for the MONSTER-only autopsy (2026-07-13): small events peak
/// 2.3-3 A, monsters ≥4 A, so this fires only on a monster and the
/// ~42 ms pre-trigger ring holds the INITIATOR window (where the ZC
/// vanished) before the current ran away.
const WAX_TRIG_RAW: u16 = 150;

/// Set by the ISR after a trip; main prints the report, clears the
/// flag, and drops its local `output_enabled` mirror so `r`/`q`
/// re-arm works afterwards.
static OC_TRIPPED: AtomicBool = AtomicBool::new(false);

/// Live vbat raw counts, refreshed at 6 kHz by the TIM7 injected
/// pump (scale: raw 1080 ≈ 8.11 V → 7.5 mV/count).
static VBAT_RAW_LIVE: AtomicU16 = AtomicU16::new(0);
/// Set by the TIM7 SAG KILL; main prints and clears.
static VBAT_SAGGED: AtomicBool = AtomicBool::new(false);
/// Consecutive sub-threshold samples (debounce counter).
static VBAT_SAG_RUN: AtomicU16 = AtomicU16::new(0);
/// Minimum vbat raw since the last `i` readout — makes between-rung
/// transits visible (per-rung snapshots twice hid real sag events
/// from the maps: steady rows read 7.9-8.1 V while the transit
/// between them collapsed to 5.6 V).
static VBAT_MIN_RAW: AtomicU16 = AtomicU16::new(u16::MAX);
/// Minimum vbat raw within the CURRENT float window — harvested
/// into every v4 window record (firmware-side aggregation of the
/// 48 kHz pump: the stream carries worst-case supply voltage at
/// window granularity, so sag transients are directly plottable
/// from any capture).
static WINDOW_VBAT_MIN: AtomicU16 = AtomicU16::new(u16::MAX);
/// The raw value that actually tripped (for the report — the live
/// value may have recovered by print time).
static VBAT_TRIP_RAW_SEEN: AtomicU16 = AtomicU16::new(0);
/// Unloaded vbat raw captured at each arm (`r`/`q`) — the sag kill
/// is RELATIVE: −10 % from this baseline. From 8.0 V that kills at
/// 7.2 V — right as the supply limit starts to engage, with volts of
/// margin — and re-arming after a bench-dial change re-baselines
/// automatically. (A fixed near-brownout threshold would only fire
/// after the bus already collapsed.)
static VBAT_BASELINE_RAW: AtomicU16 = AtomicU16::new(0);
// Sag thresholds/debounce live in minz_core::guards (host-tested).

/// Raw COMP edges in the current window (every EXTI fire).
static WINDOW_RAW: AtomicU32 = AtomicU32::new(0);
/// Edges that survived every gate (blanking, mode-5, half-window).
static WINDOW_VALID: AtomicU32 = AtomicU32::new(0);
/// `ticks_1us()` of the first gate-surviving edge; `u32::MAX` = none yet.
static WINDOW_FIRST_ZC_US: AtomicU32 = AtomicU32::new(u32::MAX);
/// OWL: `ticks_1us()` of the first PERSISTENCE-QUALIFIED edge (held
/// the expected post-ZC level for 5 reads); `u32::MAX` = none yet.
static WINDOW_QZC_US: AtomicU32 = AtomicU32::new(u32::MAX);
/// OWL estimator state — touched only inside TIM7's window-close
/// `free` block. Last qualified ZC (abs µs; MAX = chain broken) and
/// the ¾-smoothed ZC-to-ZC interval (µs; 0 = no estimate yet).
static OWL_LAST_QZC_US: AtomicU32 = AtomicU32::new(u32::MAX);
static OWL_INTERVAL_US: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------
// FALCON — closed-loop commutation.
//
// `y` arms; the first qualified ZC with a valid OWL interval engages:
// from then on the COMP ISR schedules LPTIM2 one-shots at
// interval·(30°−advance)/60° past each qualified ZC, and the LPTIM2
// ISR commutates (sector step, duty, mux, EXTI re-arm) — TIM7's
// crystal-driven angle accumulator is frozen. COMP and LPTIM2 share
// NVIC priority 1, so their state handoffs serialize (no nesting).
// Desync (no qualified ZC for 3 intervals, checked by TIM7) kills
// the output — coasting beats a jolting fallback ramp on this bench.
// ---------------------------------------------------------------

/// `y` pressed while running: engage at the next qualified ZC that
/// has an interval estimate behind it.
static CL_ARMED: AtomicBool = AtomicBool::new(false);
/// Closed loop is driving commutation (TIM7 stepper frozen).
static CL_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Desync-kill happened in ISR context; main prints + syncs state.
static CL_DESYNC: AtomicBool = AtomicBool::new(false);
/// `ticks_10us` of the most recent commutation (either engine) —
/// desync watchdog reference.
static LAST_COMM_10US: AtomicU32 = AtomicU32::new(0);
/// `ticks_10us` of the most recent ACCEPTED qZC. Free-run scheduling
/// made commutation recency meaningless as a health signal: a stalled
/// rotor under a self-sustaining rotating field draws little current
/// (no OC trip) and never stops commutating (no chain watchdog) — a
/// zombie the operator's eyes caught before the firmware did. The
/// loop's blindness IS detectable: qualified ZCs stop being accepted.
static LAST_QZC_10US: AtomicU32 = AtomicU32::new(0);
/// Selects the starvation message in the shared desync report path.
static CL_STARVED: AtomicBool = AtomicBool::new(false);

/// SWIFT — AM32-style edge-timestamped fast path (`M` key). When ON
/// and the loop is LOCKED, a comparator edge that passes the gate +
/// 5-read persistence is ACCEPTED immediately in the COMP ISR
/// (µs-precision timestamp, zero wrap latency) instead of arming a
/// candidate for ADC-sign confirmation at TIM1_UP wraps. Rationale:
/// the deferred-confirm path quantizes acceptance to the PWM wrap
/// (21 µs @ 48 kHz) + 1-2 confirm wraps — a per-window latency tax
/// that collapses the schedule margin as windows shrink (~650 Hz
/// ceiling @ 48 kHz). AM32 has no such quantization, which is why it
/// reaches full throttle on this same hardware. At speed the BEMF is
/// large and comp edges are clean — the regime where edge-trust
/// works. Engage and the ADC-confirm path are UNCHANGED (they own
/// low speed, where this board's comparator is noise-limited); all
/// guards (bounds, starvation, runaway floor, desync) apply to both
/// paths.
static CL_FAST_PATH: AtomicBool = AtomicBool::new(false);
/// Closed-loop commutation counter (for the `i` readout).
static CL_COMM_COUNT: AtomicU32 = AtomicU32::new(0);

// ---------------------------------------------------------------
// RE-ACQUISITION (the ~475 Hz wall fix). The lockout spiral: a few
// blind windows accumulate phase lag → the rotor's real ZCs drift
// toward (then past) the estimate-referenced gate → every rejection
// makes the next window blind too → starvation. The rotor is healthy
// throughout (qzc was 98 % moments before every wall death), so the
// answer is to RE-ACQUIRE, not die: after 2 consecutive ZC-less A/B
// windows, drop the gate to ~8 % (just past flyback), demand full
// 2-confirm strictness again, and re-seed the interval directly from
// the first clean qZC-to-qZC delta. The starvation watchdog (12
// intervals) remains the backstop if re-acquisition itself fails.
// ---------------------------------------------------------------
/// Consecutive A/B (measurable) windows that closed without a qZC.
static CL_NOZ_RUN: AtomicU8 = AtomicU8::new(0);
/// Re-acquisition mode active.
static CL_REACQ: AtomicBool = AtomicBool::new(false);

/// FALCON v2: deferred ZC confirmation. The COMP ISR's 0.6 µs
/// persistence check cannot out-wait PWM dwell noise (the comparator
/// sits still for tens of µs between switching edges), which let the
/// first closed-loop attempt lock onto its own schedule (interval
/// frozen regardless of amp). Instead the COMP ISR only records a
/// *candidate* (time + expected post-ZC level); the TIM1_UP ISR —
/// already sampling once per PWM cycle at the quiet wrap point —
/// confirms the level still holds on the next 2 cycles (an ~83 µs
/// persistence horizon, inherently PWM-blanked, zero extra ISR load)
/// before accepting it as the window's qualified ZC and running the
/// estimator/scheduler. Publish order: EXPECTED+CONFIRMS before the
/// ZC-time store (the store is the publish).
static CAND_ZC_US: AtomicU32 = AtomicU32::new(u32::MAX);
static CAND_EXPECTED: AtomicBool = AtomicBool::new(false);
static CAND_CONFIRMS: AtomicU8 = AtomicU8::new(0);
// ---------------------------------------------------------------
// FALCON black box: a 64-event ring recording the loop's decisions,
// frozen at the desync instant and dumped (decoded) right after the
// desync message. Slot race on the shared index is theoretically
// possible across contexts and acceptable for a diagnostic.
// Event types:
//   0 REF  commutation, shot had been re-timed by an accepted ZC
//   1 BLD  commutation, A/B window but shot was the blind free-run
//   2 DRK  commutation, dead-reckoned C window
//   3 ACC  ZC accepted (data = scheduled delay µs)
//   4 NOZ  window closed with no qZC (data = raw edge count)
//   5 DIS  candidate discarded by ADC-sign confirm (data = confirms)
//   6 DSY  desync watchdog fired (data = silence µs/10)
//   7 ENG  loop engaged (data = interval µs)
// ---------------------------------------------------------------
const BB_LEN: usize = 64;
static BB_T: [AtomicU16; BB_LEN] = [const { AtomicU16::new(0) }; BB_LEN];
static BB_TYPE: [AtomicU8; BB_LEN] = [const { AtomicU8::new(0xFF) }; BB_LEN];
static BB_SEC: [AtomicU8; BB_LEN] = [const { AtomicU8::new(0) }; BB_LEN];
static BB_DATA: [AtomicU16; BB_LEN] = [const { AtomicU16::new(0) }; BB_LEN];
static BB_IDX: AtomicU32 = AtomicU32::new(0);
static BB_FROZEN: AtomicBool = AtomicBool::new(false);
/// Set by an accepted ZC re-time; swapped false at each commutation —
/// classifies REF vs BLD.
static SHOT_REFINED: AtomicBool = AtomicBool::new(false);

fn bb_record(ev: u8, sector: u8, data: u16) {
    if BB_FROZEN.load(Ordering::Relaxed) {
        return;
    }
    let i = (BB_IDX.fetch_add(1, Ordering::Relaxed) as usize) % BB_LEN;
    BB_T[i].store(ticks_10us() as u16, Ordering::Relaxed);
    BB_SEC[i].store(sector, Ordering::Relaxed);
    BB_DATA[i].store(data, Ordering::Relaxed);
    BB_TYPE[i].store(ev, Ordering::Relaxed);
}

/// Window generation stamp: bumped at every window close. A candidate
/// carries the generation it was born in; the accept path (TIM1_UP,
/// prio 3) can be preempted by the commutation ISR (prio 1), so
/// without this check a resumed accept would re-time the NEXT
/// window's pending shot using the PREVIOUS window's stale ZC.
static WINDOW_GEN: AtomicU8 = AtomicU8::new(0);
static CAND_GEN: AtomicU8 = AtomicU8::new(0);

/// FALCON v3: the discriminator probe showed the wrap-sampled COMP
/// bit is premature in 25-73 % of windows, while the SIGN of the
/// mid-ON ADC sample (floating phase vs driven-pair neutral) is 0 %
/// premature with ~46 µs latency. Confirmation therefore reads the
/// WAX ring: A/B float windows use the ADC sign; the phase-C float
/// windows (sectors 0/3, no ADC route on PB7) keep the comp-bit rule
/// for observation and under CL are DEAD-RECKONED (commutation
/// scheduled at the estimator interval, no ZC wait).
///
/// Decaying max of the driven-high phase reads ≈ vbus in ADC counts,
/// for the two sectors whose neutral needs the unmeasured high rail.
static VBUS_EST: AtomicU16 = AtomicU16::new(0);
/// Window closes since the last accepted qZC — the span the
/// estimator divides qZC-to-qZC deltas by (a delta across one
/// dead-reckoned C window is 2 intervals). >3 = chain broken.
static WINDOWS_SINCE_QZC: AtomicU8 = AtomicU8::new(0);
/// Stream gate for the `g` key.
static STREAM_ON: AtomicBool = AtomicBool::new(false);

/// One completed float window, packaged by TIM7 at sector exit.
/// Struct AND wire form live in minz-core (round-trip encode/decode
/// + the sync-mislock sanity rules are host-tested there).
use minz_core::wire::WindowRec;

type WrecQueue = Queue<WindowRec, 64>;

/// SPSC producer half of the window-record queue, owned by the TIM7
/// ISR (enqueues inside `free`). Main drains the consumer each
/// microloop and emits frames into the TX ring.
static WREC_PROD: Mutex<RefCell<Option<Producer<'static, WindowRec>>>> =
    Mutex::new(RefCell::new(None));

/// Monotonic frame sequence number (wraps at 256) — lets the host
/// detect dropped records (queue or TX-ring overflow).
static WREC_SEQ: portable_atomic::AtomicU8 = portable_atomic::AtomicU8::new(0);

/// CHAMELEON: when `true` (default), the TIM7 ISR switches COMP2's
/// INM− mux at every sector entry so the *actually floating* phase is
/// observed in all 6 windows per electrical rev — AM32's
/// `changeCompInput` behaviour. When `false`, legacy single-phase
/// observation via the `p` key (2 windows per rev). Toggled by `o`.
static AUTO_MUX: AtomicBool = AtomicBool::new(true);

/// Which phase floats (and is therefore observed under AUTO_MUX) in
/// each six-step sector. Textbook convention — same table as
/// [`float_sector_mask`] read the other way around.
const SECTOR_FLOAT_PHASE: [comp2::ObservedPhase; 6] = [
    comp2::ObservedPhase::C, // sector 0
    comp2::ObservedPhase::B, // sector 1
    comp2::ObservedPhase::A, // sector 2
    comp2::ObservedPhase::C, // sector 3
    comp2::ObservedPhase::B, // sector 4
    comp2::ObservedPhase::A, // sector 5
];

/// COMP2 hysteresis level — cycled by the `h` key through 0 → 1 → 3
/// (skipping the 2 / medium step). Snapshotted only so the print
/// after the key press shows the current state.
static HYST_LEVEL: AtomicU8 = AtomicU8::new(0);

/// COMP2 EXTI edge mode — cycled by the `k` key:
///   0 = both edges          (rising + falling)
///   1 = rising only
///   2 = falling only
///   3 = rising sec 0–2 / falling sec 3–5
///   4 = falling sec 0–2 / rising sec 3–5
///   5 = both edges, COMP ISR only writes EDGE_BUF / SECTOR_EDGE_COUNT
///       when `COMP2.VALUE == 1` at the moment of ISR entry. With
///       hardware blanking removed (see CLAUDE.md note on the
///       TIM15-OC1 experiment), VALUE here is the *raw* comparator
///       dwell state — mode 5 is now a polarity filter rather than a
///       blanking filter. For PWM-edge suppression, set the software
///       `BLANK_TICKS_10US` window via `n`/`N` keys, which is
///       orthogonal to this mode (gates apply independently and
///       additively).
/// Modes 3 / 4 are applied per-sector inside the TIM7 ISR.
static EDGE_MODE: AtomicU8 = AtomicU8::new(0);

/// Commutation advance in degrees — cycled by the `t` key through
/// 0 / 20 / 40 / -40 / -20. Signed: positive advances the
/// commutation pattern (sector boundaries earlier in raw-angle
/// terms), negative retards it (later in raw-angle terms). The
/// sector dispatched for the motor and the COMP-observation float
/// window is computed as
/// `six_step_sector((raw_angle + advance).rem_euclid(360))`.
/// Used to compensate for open-loop rotor lead/lag — at the right
/// value the BEMF ZC should land closer to the float-window midpoint.
static ADVANCE_DEG: AtomicI8 = AtomicI8::new(0);

/// Motor-drive ISR fires at this rate. Should evenly divide
/// `PWM_FREQUENCY_HZ` for predictable phase alignment. 6 kHz = 24/4,
/// giving 4 PWM cycles per duty refresh. At f_elec ≤ 500 Hz that's
/// still ≥ 12 sine samples per electrical period — visually clean.
const MOTOR_DRIVE_HZ: u32 = minz_core::drive::MOTOR_DRIVE_HZ;

// Angle arithmetic: minz_core::drive::{ANGLE_FULL_REV_FP, angle_tick}.

// Drive geometry/arithmetic (angle_inc_fp, float_sector_mask,
// float_sectors, edges_for) lives in minz_core::drive — host-tested,
// phases indexed 0=A/1=B/2=C matching `ObservedPhase as u8`.
use minz_core::drive::{angle_inc_fp, edges_for, open_loop_gate_us};

/// Local shim: core's mask fn takes the phase index.
const fn float_sector_mask(phase: comp2::ObservedPhase) -> u8 {
    minz_core::drive::float_sector_mask(phase as u8)
}

/// SPSC producer half of the RX byte queue, owned by the USART2 ISR.
/// Main owns the consumer half and drains it in the key-dispatch pass.
static RX_PROD: Mutex<RefCell<Option<Producer<'static, u8>>>> = Mutex::new(RefCell::new(None));

// DMA-backed UART TX ring (DMA1_CH4 → USART1_TX): minz::uart_tx.
use minz::uart_tx::{TX_RING_LEN, UartTxWriter};

/// Wrap extender — call at the top of the 24 kHz TIM1_UP ISR (the
/// SOLE writer of `CYC_HIGH`/`CYC_LAST`). Takes the CYCCNT value the
/// ISR already read for its miss detector so it costs one compare +
/// two stores. Bumps `CYC_HIGH` on the ~53.7 s CYCCNT wrap. Ordering:
/// store HIGH (Release) BEFORE LAST so a reader that sees the new
/// LAST also sees the new HIGH (its double-HIGH read then agrees).
#[inline(always)]
fn cyc_extend(now_cyc: u32) {
    if now_cyc < CYC_LAST.load(Ordering::Relaxed) {
        CYC_HIGH.store(
            CYC_HIGH.load(Ordering::Relaxed).wrapping_add(1),
            Ordering::Release,
        );
    }
    CYC_LAST.store(now_cyc, Ordering::Relaxed);
}

/// 64-bit monotonic cycle count from DWT.CYCCNT + the software wrap
/// extension. Read order: HIGH, LAST, CYCCNT, HIGH again. Retry if
/// HIGH changed mid-read (the extender bumped it — happens once per
/// 53.7 s, so this is effectively never). Self-compensate for a wrap
/// the extender hasn't caught yet: CYCCNT is read AFTER LAST, so if
/// it is below LAST a wrap landed between them and HIGH is one behind
/// — add it locally. ~10 cycles, no ISR, no pending-bit.
#[inline(always)]
fn now_cyc64() -> u64 {
    loop {
        let h1 = CYC_HIGH.load(Ordering::Acquire);
        let last = CYC_LAST.load(Ordering::Relaxed);
        let cyc = cortex_m::peripheral::DWT::cycle_count();
        let h2 = CYC_HIGH.load(Ordering::Acquire);
        if h1 != h2 {
            continue;
        }
        let high = if cyc < last { h1.wrapping_add(1) } else { h1 };
        return ((high as u64) << 32) | (cyc as u64);
    }
}

/// 10 µs wall-clock ticks (u32, wraps cleanly at 2³²·10 µs ≈ 12 h;
/// deltas via `wrapping_sub`). Derived from the 64-bit cycle count so
/// the u32 truncation wraps on a power-of-two boundary — the property
/// every `wrapping_sub` consumer and the MAGPIE wire timestamp rely
/// on. Safe from any ISR / masked section (no shared mutable state
/// touched on the read path beyond the lock-free `now_cyc64`).
#[inline]
fn ticks_10us() -> u32 {
    (now_cyc64() / CYC_PER_10US) as u32
}

/// Microsecond wall clock (u32, wraps cleanly at 2³²·1 µs ≈ 71 min;
/// deltas via `wrapping_sub`). Same source as [`ticks_10us`].
#[inline]
fn ticks_1us() -> u32 {
    (now_cyc64() / CYC_PER_US) as u32
}

/// Both tick units from ONE 64-bit snapshot — the hot-ISR form, so a
/// COMP/window pass takes a single `now_cyc64()` instead of several.
/// Both derive from the same u64 so their wrap epochs stay coherent.
#[inline(always)]
fn ticks_both() -> (u32, u32) {
    let c = now_cyc64();
    ((c / CYC_PER_10US) as u32, (c / CYC_PER_US) as u32)
}

/// TRUE 64-bit 10 µs clock for absolute second-scale timing (main-
/// loop epoch pacing / `IdleLoop`). Unlike [`ticks_10us`] this never
/// wraps in practice (2⁶⁴·10 µs ≈ millennia), so `now < deadline`
/// comparisons over multi-second windows can never straddle a wrap
/// and hang — the failure mode a u32 10 µs clock would hit every 12 h.
#[inline]
fn now_10us_64() -> u64 {
    now_cyc64() / CYC_PER_10US
}

#[entry]
fn main() -> ! {
    static mut RX_QUEUE: RxQueue = Queue::new();
    static mut TX_RING: [u8; TX_RING_LEN] = [0; TX_RING_LEN];
    static mut WREC_QUEUE: WrecQueue = Queue::new();
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        mut cp,
        clocks,
        mut ahb2,
        mut apb1r1,
        mut apb2,
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    rprintln!(
        "motor_tester2: clocks sysclk={} pclk1={} baud={}",
        clocks.sysclk().raw(),
        clocks.pclk1().raw(),
        BAUD,
    );

    // SysTick is RETIRED (lever #1b): the wall clock is DWT.CYCCNT,
    // extended to 64 bits by `cyc_extend` in the TIM1_UP ISR. No
    // SysTick reload, no SysTick exception, no periodic tick ISR at
    // all. Enable the DWT cycle counter HERE, before any peripheral
    // setup, so every subsequent `ticks_*` read is valid.
    // `cp.SYST` is simply left unconfigured/idle.
    let _syst = cp.SYST;
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    // Zero CYCCNT at enable. It powers up with a residual value; if
    // that value sits within ~1 s of 2³² the counter would wrap
    // DURING calibration — before TIM1_UP's wrap extender is unmasked
    // — and the clock would jump backward once. Starting from 0 puts
    // the first wrap a guaranteed 53.7 s out, long after the extender
    // is live. (CYCCNT is DWT base + 0x004.)
    unsafe { core::ptr::write_volatile(0xE000_1004 as *mut u32, 0) };

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // Hardware-UART RX on PA2 (J3 `S` pin): AF7 + pull-up so the line
    // idles high when the adapter is unplugged. PA2's AF7 function is
    // USART2_TX — CR2.SWAP (set in the USART2 init below) routes the
    // receiver onto it. Output type is irrelevant for a receive-only
    // pin; open-drain guarantees we can never drive the line even if
    // TE were enabled by mistake.
    let mut rx2_pin = gpioa.pa2.into_alternate_open_drain::<7>(
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
    );
    rx2_pin.internal_pull_up(&mut gpioa.pupdr, true);

    // Bench instrumentation: PA3 = ADC1_IN8 (supply current),
    // PA6 = ADC1_IN11 (battery voltage). Oneshot HAL reads, ~16 µs each.
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let sense_adc = SenseAdc::new(
        dp.ADC1,
        dp.ADC_COMMON,
        pa3,
        pa6,
        &mut ahb2,
        &mut ccipr,
        clocks,
    );

    // Motor PWM pins → TIM1 AF1 (AM32 pin map).
    configure_motor_pwm_pins(
        gpioa.pa7,
        gpioa.pa8,
        gpioa.pa9,
        gpioa.pa10,
        gpiob.pb0,
        gpiob.pb1,
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
        &mut gpioa.afrh,
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );

    // PB3: scope trigger — push-pull output, toggled at electrical freq F.
    let _pb3_trig = gpiob
        .pb3
        .into_push_pull_output(&mut gpiob.moder, &mut gpiob.otyper);

    // USART1 in half-duplex mode: PB6 only, open-drain alternate
    // function. The HAL's `Serial::usart1` accepts a 1-tuple `(tx,)`
    // when `tx` implements `TxHalfDuplexPin` (= AF + OpenDrain), so
    // PB7 stays unclaimed and is available for the COMP2 INM input.
    //
    // Internal pull-up is enabled because half-duplex relies on the
    // line idling high — the L431 only drives it low for start bits.
    // Most USB-TTL adapters also have an internal pull-up on RX, but
    // this keeps it working without that assumption.
    let mut usart_tx = gpiob.pb6.into_alternate_open_drain::<7>(
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );
    usart_tx.internal_pull_up(&mut gpiob.pupdr, true);
    let serial = Serial::usart1(
        dp.USART1,
        (usart_tx,),
        Config::default().baudrate(BAUD.bps()),
        clocks,
        &mut apb2,
    );
    let (mut tx, _) = serial.split();

    // Flip PB6 to push-pull after the fact — the trick that makes
    // ≥921600 baud work (rationale at minz::uart_tx).
    minz::uart_tx::usart1_tx_push_pull_pb6();

    // COMP2 BEMF sense, switchable INM− input. Textbook convention:
    //   INP+ = PB4 (virtual neutral)
    //   INM− = PA4 (phase A BEMF) — initial; `p` key cycles through
    //           PA4 / PA5 (B) / PB7 (C).
    // All four pins go to Analog mode before init.
    let pb4 = gpiob.pb4.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pb7 = gpiob.pb7.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pa5 = gpioa.pa5.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa4 = gpioa.pa4.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    comp2::init(pb4, pb7, pa5, pa4, &mut apb2);
    comp2::configure_exti_both_edges();

    // Empirical readback of COMP2_CSR after init. Hardware blanking
    // removed (see CLAUDE.md note about TIM15-OC1 experiment); expect
    // 0x04000071 = phase A under textbook convention, BLANKING=000.
    let comp2_csr = unsafe { (*minz::hal::stm32::COMP::ptr()).comp2_csr.read().bits() };
    rprintln!(
        "COMP2_CSR = 0x{:08x}  (expect 0x04000071 for phase A, blanking off)",
        comp2_csr,
    );

    // TIM1 motor PWM init. After init, also enable:
    //   - the update-event interrupt (TIM1_UP_TIM16) for the
    //     per-PWM-period COMP2 / sector L-dump sample;
    //   - CC1IE / CC2IE / CC3IE (TIM1_CC) for the software
    //     PWM-edge timestamp used by the COMP-ISR blanking gate.
    tim1_motor_pwm::init(dp.TIM1, &mut apb2);
    tim1_motor_pwm::enable_update_interrupt();
    tim1_motor_pwm::enable_cc_interrupts();

    // GECKO: continuous PWM-synchronous current sampling. OC4REF
    // (falling edge at CNT = SAMPLE_TICKS) → TRGO2 → ADC1 ch8, one
    // conversion per 24 kHz PWM cycle, harvested in TIM1_UP_TIM16.
    // Needs TIM1 running (init above) and the ADC powered/calibrated
    // (SenseAdc::new above). From here on the HAL OneShot reads are
    // off-limits; vbat goes through the injected group instead.
    adc_sync::start(adc_sync::SAMPLE_TICKS);

    // INDEPENDENT WATCHDOG — the guard that survives the MCU. The
    // 2026-07-10 burnt motor: deep bus sag browned out the core,
    // which WEDGED with the bridge frozen in its last state; every
    // software guard died with it and the PSU poured into a stalled
    // winding. IWDG (LSI 32 kHz / 32 → 1 kHz, reload 1000 = ~1 s)
    // resets a wedged chip and releases the bridge no matter what
    // the firmware was doing. Refreshed once per main-loop pass; the
    // longest legitimate main-loop stall is the `u` blast (~0.35 s at
    // 2 Mbaud → ~3× margin, NOT the waxwing dump's ~100 ms/10×). A
    // future baud drop would shrink that margin further — refresh
    // mid-blast before slowing the link.
    minz::iwdg::start_1s();

    // FALCON commutation one-shot (armed only when the loop engages).
    minz::lptim2_oneshot::init();

    // TIM7 motor-drive heartbeat at `MOTOR_DRIVE_HZ` (= 6 kHz). The
    // `TIM7` ISR (below) reads the motor-drive atomics each tick and
    // writes TIM1's CCRs (sine path) or commutates (six-step path).
    // Main loop never touches TIM1 directly anymore.
    tim7_drive::init(dp.TIM7, &mut apb1r1, MOTOR_DRIVE_HZ);

    // USART2 receiver on PA2 via CR2.SWAP. Raw register init — the
    // HAL's `Serial::usart2` insists on PA3 for RX (no swap support),
    // and PA3 is our current-sense ADC input. Per RM0394, BRR and
    // CR2.SWAP are only writable while UE=0 (true out of reset).
    // TE stays 0: receive-only, we never drive the pin. Kernel clock
    // is the CCIPR reset default (PCLK1 = 80 MHz).
    minz::usart2_rx::init_pa2_rx(dp.USART2, clocks.pclk1().raw(), BAUD);

    let (producer, mut consumer) = RX_QUEUE.split();
    let (wrec_producer, mut wrec_consumer) = WREC_QUEUE.split();

    free(|cs| {
        RX_PROD.borrow(cs).replace(Some(producer));
        WREC_PROD.borrow(cs).replace(Some(wrec_producer));
    });

    // Publish initial motor-drive state BEFORE unmasking TIM7 so the
    // first ISR fire sees consistent atomics. Boot lands in the same
    // "off" state as the `w` key: MOE cleared, `MOTOR_ENABLED=false`,
    // displayed `f=0`. Atomic angle/sector defaults are still seeded
    // at `FREQ_START` so that if the user later presses `r`/`q` the
    // re-arm publishes a clean delta from a known baseline.
    SIX_STEP_MODE.store(true, Ordering::Relaxed);
    AMPLITUDE_PCT.store(AMP_START as u8, Ordering::Relaxed);
    AMP_TARGET_PCT.store(AMP_START as u8, Ordering::Relaxed);
    ANGLE_INC.store(angle_inc_fp(FREQ_START), Ordering::Relaxed);
    FLOAT_SECTOR_MASK.store(
        float_sector_mask(comp2::ObservedPhase::A),
        Ordering::Relaxed,
    );
    SECTOR_GATE_US.store(open_loop_gate_us(FREQ_START), Ordering::Relaxed);
    MOTOR_ENABLED.store(false, Ordering::Relaxed);
    // Kill the h-bridges immediately. `tim1_motor_pwm::init` sets MOE=1
    // as part of register parity with AM32, which would otherwise leave
    // us driving whatever stale CCRs happen to be in TIM1 the instant
    // TIM7's ISR is unmasked. Equivalent to pressing `w`.
    tim1_motor_pwm::all_off();
    // Discard any UEV latched during TIM7 init so the first ISR fire
    // doesn't run on stale (zero) state.
    tim7_drive::clear_update_flag();
    // EXTI line 22 stays MASKED while the motor is off. With FETs
    // off both the observed phase pin (PA4 by default) and PB4 (virtual neutral) are floating
    // and the comparator threshold-noise can storm at ~650 k events/s,
    // saturating the CPU and starving LPTIM1 (verified via the `i`
    // key's irq/s readout). EXTI22 is re-armed at every `r` / `q`
    // re-arm and re-masked at every `w` / `L` / `E` kill.
    comp2::set_exti_enabled(false);

    // Boot help — sent via blocking `writeln!` BEFORE the NVIC unmask
    // block so the per-byte stall doesn't matter and so the IdleLoop
    // calibration below runs with no app ISRs eating cycles. USART1
    // hardware is already configured; `writeln!` just polls TXE on
    // TDR, no IRQ involvement.
    writeln!(
        &mut tx,
        "\r\n=== Hello, this is motor tester 2 (RX on PA2/S pin, {} baud) ===\r",
        BAUD,
    )
    .ok();
    // Static key reference — one blocking write; only the boot-state
    // line below interpolates values.
    const HELP: &str = "Frequency (Hz):       d   +1, c -1, f +10, v -10\r\n\
        Amplitude (% of ARR): a +1, z -1, s +10, x -10\r\n\
        Mode toggle:          m   (sine <-> six-step)\r\n\
        Panic reset:          r   (six-step, f=start, amp=start)\r\n\
        Slow reset:           q   (six-step, f=50, amp=start)\r\n\
        Kill output:          w   (clear MOE, all FETs off)\r\n\
        Print sense:          i   (PA3 IN8 isns, PA6 IN11 vbat)\r\n\
        Print BEMF comp:      b   (rate/s + level on observed phase)\r\n\
        Cycle observed phase: p   (A=PA4, B=PA5, C=PB7; manual mode only)\r\n\
        Toggle comp mux:      o   (auto per-sector [default] <-> manual single phase)\r\n\
        Cycle COMP hyst:      h   (0/none -> 1/low -> 3/high -> 0)\r\n\
        Cycle EXTI edges:     k   (both / raw_rise / raw_fall / phys_ZC / phys_anti / val_gated)\r\n\
        Commutation advance:  t +2deg / T -2deg (clamped 0..28)\r\n\
        ZC path toggle:       M   (adc-confirm <-> SWIFT am32-edge)\r\n\
        Adjust SW blanking:   n/N (+/- 1 us)   ./, (+/- 10 us coarse), clamped 0..50 us\r\n\
        Dump PWM samples:     l   (per-60deg chunks: . = 0, # = 1)\r\n\
        Dump edge buffer:     e   (2 revs, 10us bins; f >= 50 Hz)\r\n\
        UART blast test:      u   (64 KiB counting pattern, blocking)\r\n\
        Window record stream: g   (binary 22B frames, one per float window)\r\n\
        Waveform dump:        j   (85 ms burst: A/B volts + current + comp/sector, a85)\r\n\
        Closed loop:          y   (engage at next qualified ZC; y again = kill; desync auto-kills)\r\n\
        Failsafe: auto-kill at >1.5 A avg over 85 ms (stall guard, r/q re-arms)\r\n\
        Freeze/dump edge buf: E   (1st press: freeze + dump; 2nd press: resume)\r\n";
    tx.write_str(HELP).ok();
    writeln!(
        &mut tx,
        "Boot: OUTPUT OFF, f = 0 Hz, amp = {} % (cap {}). Press r/q to arm.\r",
        AMP_START, AMP_MAX,
    )
    .ok();

    let mut tx_writer = UartTxWriter::new(tx, TX_RING);

    // ----------------------------------------------------------------
    // Re-enable interrupts globally: `panic::ensure_rtt` (run by
    // `init`) called `cortex_m::interrupt::disable()` and never
    // re-enabled, so PRIMASK=1 until here. NVIC still masks all app
    // IRQs, so the calibration baseline below runs with NOTHING
    // periodic firing (SysTick is retired) — a clean free-loop
    // reference. The DWT clock needs no interrupt to advance.
    // ----------------------------------------------------------------
    unsafe { cortex_m::interrupt::enable() };

    let mut idle_loop = IdleLoop::new();
    // TRUE 64-bit (not u32-widened): the epoch pacing compares
    // `now < next_epoch` over multi-second windows, which a 12 h-
    // wrapping u32 clock would eventually straddle → a one-epoch hang.
    let now_u64 = || now_10us_64();
    let cal = idle_loop.calibrate(SECOND_TICKS, &now_u64);
    rprintln!(
        "idle_loop: calibration = {} counts / s (SysTick-only baseline)",
        cal,
    );
    // Mirror to UART so host logs capture the baseline — needed to
    // interpret cpu% across builds (the busy math is relative to it).
    write!(&mut tx_writer, "idle cal = {} counts/s\r\n", cal).ok();

    // Program NVIC + SCB priorities BEFORE unmasking the app IRQs so
    // every IRQ comes up at its intended level. PRIGROUP=3 (4 preempt
    // / 0 sub bits). Levels: SysTick=0, COMP=1, TIM1=2, TIM7=3,
    // USART2 RX=4 (TIM1↔TIM7 swapped 2026-07-13 — see priority.rs:
    // TIM7 above TIM1 coalesced TIM1_UP's UIF, 12 % of PWM cycles
    // lost at amp 44-48 CL). USART2 isn't in the canonical `set_irq_prios` list
    // (it replaces the LPTIM1/EXTI0 soft-UART) — set it explicitly at
    // the soft-UART's old level so host RX stays the lowest-priority
    // tier.
    unsafe {
        priority::set_irq_prios();
        priority::set_irq_prio(Interrupt::USART2, priority::PRIO_LPTIM1);
        // FALCON: commutation one-shot at COMP's level — same
        // priority means COMP and LPTIM2 serialize (tail-chain, never
        // nest), so ZC-accept and commutate can't interleave state.
        priority::set_irq_prio(Interrupt::LPTIM2, priority::PRIO_COMP);
        // TIM1_CC stays at PRIO_TIM1 = 3 (set by set_irq_prios above).
        // We do NOT promote it to 0 to run before COMP (priority 1):
        // that caused a lockup because priority-0 ISRs block SysTick
        // (also priority 0) from running, and `ticks_1us()` in TIM1_CC
        // ISR spins in its CVR-wrap retry loop waiting for a SysTick
        // that can never preempt it → 100% CPU.
        //
        // In practice the race is benign: COMP propagation delay through
        // the gate driver and EXTI path is 0.5–2 µs ≈ 40–160 cycles,
        // while TIM1_CC ISR completes in ~40 cycles. TIM1_CC almost
        // always updates LAST_PWM_EDGE_US before the analog transition
        // reaches the comparator. The blanking sweep confirmed it works.
    }

    unsafe {
        NVIC::unmask(Interrupt::USART2);
        NVIC::unmask(Interrupt::LPTIM2);
        NVIC::unmask(Interrupt::COMP);
        NVIC::unmask(Interrupt::TIM1_UP_TIM16);
        NVIC::unmask(Interrupt::TIM7);
        NVIC::unmask(Interrupt::TIM1_CC);
    }

    // Emit the ACTUAL priority configuration (hardware read-back, not
    // the constants) over UART once at init — every automated-test
    // session log then permanently records what the run executed
    // under. Mirrors to RTT via the existing dump helpers.
    priority::dump_to(&mut tx_writer);
    priority::dump_prigroup();
    priority::dump_irq_prios();

    // Main-loop mirrors of motor state held in atomics. We keep
    // locals only so the prev-vs-now diff in the key handlers can
    // print `f=` / `amp=` / `mode=` messages cleanly. The actual
    // commutation reads from atomics in the TIM7 ISR.
    let mut waveform = Waveform::SixStep;
    let mut amplitude_pct: u16 = AMP_START;
    // `0` is a display-only sentinel meaning "not driving"; FREQ_MIN
    // clamps any subsequent key press back into the valid 1..=FREQ_MAX
    // range. Paired with `output_enabled = false` (= `w`-key state).
    let mut electrical_hz: u32 = 0;
    let mut output_enabled = false;

    // Per-second snapshot windows for the COMP / VALID rates
    // (host-tested wrapping-delta in minz_core::rates). Latched at
    // every 1 s boundary; restarted by the `o`/`p` mux keys.
    let mut comp_rate_win = minz_core::rates::RateWindow::new(COMP_COUNT.load(Ordering::Relaxed));
    let mut valid_rate_win =
        minz_core::rates::RateWindow::new(VALID_COMP_COUNT.load(Ordering::Relaxed));

    // CPU busy% snapshot updated at every 1 s boundary. The `i` key
    // reads this so the figure printed alongside vbat / isns reflects
    // the most recent fully-elapsed second — same window as the
    // `IdleLoop` calibration.
    let mut last_busy_pct: u8 = 0;

    // Per-ISR rate diagnostics for the `i` key. Snapshot every
    // counter + the wall clock at the previous press; on the next
    // press, compute (delta count × 100 000 / delta ticks) to get
    // events/sec since the last press. Initialised at boot so the
    // first `i` reports the cumulative rate since calibration ended.
    let mut last_i_tick: u32 = ticks_10us();
    let mut last_i_comp: u32 = COMP_COUNT.load(Ordering::Relaxed);
    let mut last_i_usart2: u32 = USART2_COUNT.load(Ordering::Relaxed);
    let mut last_i_tim7: u32 = TIM7_COUNT.load(Ordering::Relaxed);
    let mut last_i_tim1: u32 = TIM1_UP_COUNT.load(Ordering::Relaxed);
    let mut last_i_tim1_cc: u32 = TIM1_CC_COUNT.load(Ordering::Relaxed);
    let mut last_i_miss: u32 = TIM1_UP_MISSED.load(Ordering::Relaxed);

    // Which phase we're routing through COMP2 INM− right now. The
    // float-sector mapping below depends on this. `p` cycles A→B→C.
    let mut observed_phase = comp2::ObservedPhase::A;

    // Outer / inner microloop — mirrors
    // `ref/usb-servo-rs/.../examples/loop_timing.rs`:
    //   - Outer iteration = 1 s. At its boundary we latch the idle
    //     counter and snapshot the per-second COMP rates.
    //   - Inner microloop fires every `MICROLOOP_TICKS` (= 1 ms).
    //     Each microloop spins `IdleLoop::run_until` (the slack) until
    //     its boundary, then runs one active-work pass: RX dequeue +
    //     key dispatch + TX service. ISR-stolen cycles and main's
    //     active work both reduce the counter relative to the
    //     calibrated baseline, so both show up as "busy" %.
    let mut epoch_start = now_u64();
    // Clock monotonicity tripwire: the 64-bit DWT clock must never go
    // backward. If it ever does (a wrap-extension bug), bump a counter
    // shown in `i` so the failure is LOUD, not a silent time glitch.
    let mut last_mono: u64 = epoch_start;
    loop {
        let next_epoch = epoch_start + SECOND_TICKS;
        let mut next_microloop = epoch_start + MICROLOOP_TICKS;

        while now_u64() < next_epoch {
            // IWDG refresh — once per microloop (~10 ms cadence,
            // 100× inside the 1 s window). If the core wedges, this
            // stops and the watchdog resets the chip, releasing the
            // bridge.
            minz::iwdg::refresh();

            // Monotonic check (main-loop rate tripwire).
            let mono = now_10us_64();
            if mono < last_mono {
                CLOCK_BACK.fetch_add(1, Ordering::Relaxed);
            }
            last_mono = mono;

            // Slack: spin idle counter until next microloop boundary.
            // ISRs preempt this naturally and steal counter increments;
            // that's exactly how "busy" gets measured.
            idle_loop.run_until(next_microloop, &now_u64);

            // Active-phase duration of this microloop pass (to the
            // end of the loop body) - closes the busy% accounting.
            let _dur_main = DurGuard::new(&DUR_MAIN);

            // Analog black-box trigger fired? The ISR froze the
            // free-run current ring (adc_sync::CUR_RING) the instant a
            // >4 A cycle sample was seen; it holds the ~0.55 ms of
            // oversampled (~150/cycle) current LEADING UP TO the
            // trigger — the spike ONSET at ~0.27 µs resolution, which
            // is what distinguishes a smooth winding-limited BEMF-aided
            // ramp from a sub-µs shoot-through transient at a switching
            // edge. Dump it flat (4 samples/frame), resume the ring,
            // then inject a `j` to append the per-cycle analog context
            // (A/B/comp/sector) for correlation.
            let mut injected: Option<u8> = None;
            if WAX_TRIGGERED.swap(false, Ordering::Relaxed) {
                write!(
                    &mut tx_writer,
                    "!! WAX TRIGGER: >4 A cycle sample - free-run current ring frozen\r\n"
                )
                .ok();
                let oldest = adc_sync::freeze_current();
                write!(
                    &mut tx_writer,
                    "gecko: {} samples b85 isns oversampled 12-bit, adc_hz~3700000\r\n",
                    adc_sync::CUR_FRAMES,
                )
                .ok();
                minz_core::dump::wax_cdump(
                    adc_sync::CUR_FRAMES / 4,
                    |k| {
                        let base = oldest + k * 4;
                        [
                            adc_sync::cur_word(base),
                            adc_sync::cur_word(base + 1),
                            adc_sync::cur_word(base + 2),
                            adc_sync::cur_word(base + 3),
                        ]
                    },
                    |bb| tx_writer.write_blocking(bb),
                );
                // The trigger disarmed itself (one-shot) — stop the
                // oversample, back to inject-only. Re-arm with `J`.
                adc_sync::oversample_stop();
                // Dump the frozen black box: the ZC/commutation event
                // sequence into the spike (EV_NOZ miss, EV_RAQ reacq,
                // EV_ACC accept w/ qzc offset), then thaw it.
                write!(&mut tx_writer, "bb (spike onset):\r\n").ok();
                let bidx = BB_IDX.load(Ordering::Relaxed) as usize;
                minz_core::blackbox::format_dump(
                    (0..BB_LEN).map(|k| {
                        let i = (bidx + k) % BB_LEN;
                        minz_core::blackbox::Event {
                            t: BB_T[i].load(Ordering::Relaxed),
                            ty: BB_TYPE[i].load(Ordering::Relaxed),
                            sector: BB_SEC[i].load(Ordering::Relaxed),
                            data: BB_DATA[i].load(Ordering::Relaxed),
                        }
                    }),
                    |b| tx_writer.write_blocking(b),
                );
                BB_FROZEN.store(false, Ordering::Relaxed);
                injected = Some(b'j');
            }

            // ISR trip-flag sync — BEFORE key dispatch (roadmap F2):
            // when an ISR kill and an `r`/`q` land in the same pass
            // (guaranteed when main was stalled in a `u` blast or
            // waxwing dump with keys queued), the arm used to see a
            // stale `output_enabled == true` and silently no-op while
            // printing success. Draining the flags first makes the
            // mirror honest before any key reads it.
            //
            // FALCON desync report: the TIM7 watchdog already killed
            // the output; sync main's mirror and tell the operator.
            if CL_DESYNC.load(Ordering::Relaxed) {
                CL_DESYNC.store(false, Ordering::Relaxed);
                output_enabled = false;
                if CL_STARVED.load(Ordering::Relaxed) {
                    CL_STARVED.store(false, Ordering::Relaxed);
                    write!(
                        &mut tx_writer,
                        "!! CL ZC-STARVED - no accepted ZC for 12 intervals (stall/blind) - output killed\r\n",
                    )
                    .ok();
                } else {
                    write!(
                        &mut tx_writer,
                        "!! CL DESYNC - commutation silence > 3 intervals - output killed (r/q re-arms)\r\n",
                    )
                    .ok();
                }
                // Black-box dump: the 64 events leading to the kill,
                // dt in µs since the previous recorded event. Line
                // rendering is host-tested in minz_core::blackbox.
                let idx = BB_IDX.load(Ordering::Relaxed) as usize;
                minz_core::blackbox::format_dump(
                    (0..BB_LEN).map(|k| {
                        let i = (idx + k) % BB_LEN;
                        minz_core::blackbox::Event {
                            t: BB_T[i].load(Ordering::Relaxed),
                            ty: BB_TYPE[i].load(Ordering::Relaxed),
                            sector: BB_SEC[i].load(Ordering::Relaxed),
                            data: BB_DATA[i].load(Ordering::Relaxed),
                        }
                    }),
                    |b| tx_writer.write_blocking(b),
                );
                for e in BB_TYPE.iter() {
                    e.store(0xFF, Ordering::Relaxed);
                }
                BB_FROZEN.store(false, Ordering::Relaxed);
            }
            // Sag trip report: the ISR already killed the output;
            // sync main's mirror so `r`/`q` re-arm works. mV via the
            // calibrated adc_to_mv + the ONE divider constant
            // (minz_core::sense) — the old hardcoded ×7507/1000 was a
            // second calibration that could silently diverge.
            if VBAT_SAGGED.load(Ordering::Relaxed) {
                VBAT_SAGGED.store(false, Ordering::Relaxed);
                output_enabled = false;
                let raw = VBAT_TRIP_RAW_SEEN.load(Ordering::Relaxed);
                let base = VBAT_BASELINE_RAW.load(Ordering::Relaxed);
                write!(
                    &mut tx_writer,
                    "!! VBAT SAG KILL: bus {} mV (1.3 ms sustained) < 90% of arm baseline {} mV - output killed (r/q re-arms)\r\n",
                    minz_core::sense::vbat_mv(sense_adc.adc_to_mv(raw) as u32),
                    minz_core::sense::vbat_mv(sense_adc.adc_to_mv(base) as u32),
                )
                .ok();
                tx_writer.write_blocking(&[]);
            }
            if OC_TRIPPED.load(Ordering::Relaxed) {
                OC_TRIPPED.store(false, Ordering::Relaxed);
                output_enabled = false;
                write!(
                    &mut tx_writer,
                    "!! OVERCURRENT TRIP: current above throttle envelope for 85 ms - output killed (r/q re-arms)\r\n",
                )
                .ok();
            }

            // Active phase: drain RX queue + dispatch keys.
            while let Some(b) = injected.take().or_else(|| consumer.dequeue()) {
                let mut do_edge_dump = false;
                let prev_amp = amplitude_pct;
                let prev_hz = electrical_hz;
                let prev_mode = waveform;
                match b {
                    b'a' => amplitude_pct = clamp_amp(amplitude_pct as i32 + 1),
                    b'z' => amplitude_pct = clamp_amp(amplitude_pct as i32 - 1),
                    b's' => amplitude_pct = clamp_amp(amplitude_pct as i32 + 10),
                    b'x' => amplitude_pct = clamp_amp(amplitude_pct as i32 - 10),
                    b'd' => electrical_hz = clamp_hz(electrical_hz as i32 + 1),
                    b'c' => electrical_hz = clamp_hz(electrical_hz as i32 - 1),
                    b'f' => electrical_hz = clamp_hz(electrical_hz as i32 + 10),
                    b'v' => electrical_hz = clamp_hz(electrical_hz as i32 - 10),
                    b'J' => {
                        let on = !WAX_TRIG_ARMED.load(Ordering::Relaxed);
                        WAX_TRIG_ARMED.store(on, Ordering::Relaxed);
                        // Gate the free-run current oversample on the
                        // arm: it runs ONLY while hunting, so it fills
                        // the pre-trigger ring, then stops — normal
                        // lock keeps the inject-only (pre-hybrid-
                        // equivalent) ADC activity.
                        if on {
                            adc_sync::oversample_start();
                        } else {
                            adc_sync::oversample_stop();
                        }
                        write!(
                            &mut tx_writer,
                            "wax trigger {}\r\n",
                            if on {
                                "ARMED (one-shot >4A, oversample on)"
                            } else {
                                "off"
                            }
                        )
                        .ok();
                        tx_writer.write_blocking(&[]);
                    }
                    b'M' => {
                        let on = !CL_FAST_PATH.load(Ordering::Relaxed);
                        CL_FAST_PATH.store(on, Ordering::Relaxed);
                        write!(
                            &mut tx_writer,
                            "zc path = {}\r\n",
                            if on {
                                "SWIFT (am32 edge)"
                            } else {
                                "adc-confirm"
                            }
                        )
                        .ok();
                        tx_writer.write_blocking(&[]);
                    }
                    // Arm/kill/CL/mode transitions all go through the
                    // host-tested state machine (minz_core::mode):
                    // estimator-reset-on-arm, snap-on-arm, EXTI mask
                    // rules, no-zombie-CL, and the B4 stale-arm gate
                    // are its regression suite. `r` = panic reset to
                    // FREQ_START; `q` = slow bench reset at 50 Hz
                    // (floor of clean no-cog tracking); `w` = hard
                    // kill (CCR/CCER preserved so `r` resumes).
                    b'm' => mode_cmd(
                        minz_core::mode::Cmd::ModeToggle,
                        &mut output_enabled,
                        &mut waveform,
                        &mut electrical_hz,
                        &mut amplitude_pct,
                        &mut tx_writer,
                    ),
                    b'r' => mode_cmd(
                        minz_core::mode::Cmd::Arm {
                            hz: FREQ_START,
                            amp_pct: AMP_START,
                        },
                        &mut output_enabled,
                        &mut waveform,
                        &mut electrical_hz,
                        &mut amplitude_pct,
                        &mut tx_writer,
                    ),
                    b'q' => mode_cmd(
                        minz_core::mode::Cmd::Arm {
                            hz: 50,
                            amp_pct: AMP_START,
                        },
                        &mut output_enabled,
                        &mut waveform,
                        &mut electrical_hz,
                        &mut amplitude_pct,
                        &mut tx_writer,
                    ),
                    b'w' => mode_cmd(
                        minz_core::mode::Cmd::Kill,
                        &mut output_enabled,
                        &mut waveform,
                        &mut electrical_hz,
                        &mut amplitude_pct,
                        &mut tx_writer,
                    ),
                    b'b' => {
                        // COMP2 edge rates: `raw` = every EXTI fire, `valid` =
                        // edges that survive the time-window check (only events
                        // fired in the second half of the current float sector
                        // are counted). `level` = current COMP_VALUE bit.
                        write!(
                            &mut tx_writer,
                            "comp2 phase={} raw={}/s valid={}/s level={}\r\n",
                            if AUTO_MUX.load(Ordering::Relaxed) {
                                "auto"
                            } else {
                                observed_phase.name()
                            },
                            COMP_RATE.load(Ordering::Relaxed),
                            VALID_COMP_RATE.load(Ordering::Relaxed),
                            comp2::value() as u8,
                        )
                        .ok();
                    }
                    b'o' => {
                        // CHAMELEON toggle: auto per-sector mux vs
                        // legacy single-phase observation. Turning
                        // auto OFF restores the `p`-selected phase's
                        // mux routing (in `free` — TIM7 modifies the
                        // same CSR when auto is on).
                        let was_auto = AUTO_MUX.load(Ordering::Relaxed);
                        if was_auto {
                            free(|_| {
                                AUTO_MUX.store(false, Ordering::Relaxed);
                                comp2::set_observed_phase(observed_phase);
                            });
                            WAS_IN_FLOAT_SECTOR.store(false, Ordering::Relaxed);
                            write!(
                                &mut tx_writer,
                                "mux=manual (observing {} only; 'p' cycles)\r\n",
                                observed_phase.name(),
                            )
                            .ok();
                        } else {
                            AUTO_MUX.store(true, Ordering::Relaxed);
                            write!(
                                &mut tx_writer,
                                "mux=auto (per-sector floating phase, 6 windows/rev)\r\n",
                            )
                            .ok();
                        }
                        comp_rate_win.latch(COMP_COUNT.load(Ordering::Relaxed));
                        valid_rate_win.latch(VALID_COMP_COUNT.load(Ordering::Relaxed));
                        COMP_RATE.store(0, Ordering::Relaxed);
                        VALID_COMP_RATE.store(0, Ordering::Relaxed);
                    }
                    b'p' if AUTO_MUX.load(Ordering::Relaxed) => {
                        write!(
                            &mut tx_writer,
                            "mux=auto - 'p' is manual-mode only ('o' toggles)\r\n",
                        )
                        .ok();
                    }
                    b'p' => {
                        // Cycle the observed phase: A → B → C → A.
                        // Mask EXTI during the switch so a transient
                        // edge from the input mux doesn't get counted,
                        // and reset the rate window so the next 1-s
                        // sample reports the new phase's count cleanly.
                        // Briefly mask EXTI across the INMSEL change
                        // so the mux transient doesn't latch a fake
                        // edge, then immediately re-enable (we collect
                        // edges across the whole rev now).
                        comp2::set_exti_enabled(false);
                        WAS_IN_FLOAT_SECTOR.store(false, Ordering::Relaxed);
                        observed_phase = match observed_phase {
                            comp2::ObservedPhase::A => comp2::ObservedPhase::B,
                            comp2::ObservedPhase::B => comp2::ObservedPhase::C,
                            comp2::ObservedPhase::C => comp2::ObservedPhase::A,
                        };
                        free(|_| comp2::set_observed_phase(observed_phase));
                        // Only re-arm EXTI22 if the FETs are actually
                        // driving. With the motor off the new INMSEL
                        // pin is just as floating as the old one, and
                        // re-enabling here would restart the storm.
                        if output_enabled {
                            comp2::set_exti_enabled(true);
                        }
                        FLOAT_SECTOR_MASK
                            .store(float_sector_mask(observed_phase), Ordering::Relaxed);
                        comp_rate_win.latch(COMP_COUNT.load(Ordering::Relaxed));
                        valid_rate_win.latch(VALID_COMP_COUNT.load(Ordering::Relaxed));
                        COMP_RATE.store(0, Ordering::Relaxed);
                        VALID_COMP_RATE.store(0, Ordering::Relaxed);
                        write!(
                            &mut tx_writer,
                            "observe phase {}\r\n",
                            observed_phase.name(),
                        )
                        .ok();
                    }
                    b'l' => {
                        // Dump PWM-sampled COMP2 over the last 2
                        // electrical revs, aligned to sector 0. One
                        // line per sector chunk (`.` = COMP=0, `#` =
                        // COMP=1). Float sectors of the observed
                        // phase get a textbook-ZC midpoint marker:
                        // `*` if mid char was `#`, `o` if `.`.
                        //
                        // Motor keeps running. We snapshot the entire
                        // PWM ring into stack memory under a brief
                        // TIM1_UP_TIM16 mask (~125 µs for 2048 atomic
                        // loads), then unmask and print from the
                        // snapshot. TIM1 PWM and TIM7 commutation run
                        // on their own — the snapshot is the only
                        // thing that needs a consistent view.
                        let (float_s0, float_s1) =
                            minz_core::drive::float_sectors(observed_phase as u8);
                        let mut snap = [0u8; PWM_SAMPLE_LEN];
                        NVIC::mask(Interrupt::TIM1_UP_TIM16);
                        let head =
                            (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
                        for i in 0..PWM_SAMPLE_LEN {
                            snap[i] = PWM_SAMPLE_BUF[i].load(Ordering::Relaxed);
                        }
                        unsafe { NVIC::unmask(Interrupt::TIM1_UP_TIM16) };
                        // Rev alignment + chunk/ZC-marker rendering are
                        // host-tested in minz_core::dump.
                        match minz_core::dump::pwm_rev_span(&snap, head) {
                            Ok((r_start, length)) => {
                                write!(
                                    &mut tx_writer,
                                    "pwm_samples last 2 revs aligned to sec 0 \
                                     ({} samples, hyst={}, obs={}, advance={}deg, blank={}us):\r\n",
                                    length,
                                    HYST_LEVEL.load(Ordering::Relaxed),
                                    observed_phase.name(),
                                    ADVANCE_DEG.load(Ordering::Relaxed),
                                    BLANK_US.load(Ordering::Relaxed),
                                )
                                .ok();
                                minz_core::dump::pwm_sector_dump(
                                    &snap,
                                    r_start,
                                    length,
                                    float_s0,
                                    float_s1,
                                    |b| tx_writer.write_blocking(b),
                                );
                            }
                            Err(found) => {
                                write!(
                                    &mut tx_writer,
                                    "pwm_samples last 2 revs: only {} rev starts in buffer (need 3)\r\n",
                                    found,
                                )
                                .ok();
                            }
                        }
                    }
                    // FALCON engage/kill: engaging waits for the next
                    // qualified ZC with an interval estimate behind it;
                    // pressing again while active = kill + coast. The
                    // estimator-reset-on-arm lives in the core state
                    // machine.
                    b'y' => mode_cmd(
                        minz_core::mode::Cmd::ClToggle,
                        &mut output_enabled,
                        &mut waveform,
                        &mut electrical_hz,
                        &mut amplitude_pct,
                        &mut tx_writer,
                    ),
                    b'g' => {
                        // MAGPIE stream toggle. Binary 16-byte frames
                        // interleave with ASCII key echoes; the host
                        // parser scans for the 5A A5 sync pair.
                        let on = !STREAM_ON.load(Ordering::Relaxed);
                        STREAM_ON.store(on, Ordering::Relaxed);
                        write!(
                            &mut tx_writer,
                            "stream={}\r\n",
                            if on { "on" } else { "off" },
                        )
                        .ok();
                    }
                    b'u' => {
                        // UART throughput / integrity test. Blast 64 KiB
                        // of a 0..=255 counting pattern between two marker
                        // lines via blocking TXE polling. The host script
                        // verifies every byte and measures the sustained
                        // rate. Main loop stalls for the duration (5.9 s
                        // at 115200, 0.34 s at 2 M); the motor is
                        // unaffected — commutation lives in TIM7/TIM1
                        // ISRs which preempt this loop freely.
                        write!(&mut tx_writer, "uart_test start len=65536\r\n").ok();
                        let mut chunk = [0u8; 256];
                        for (i, c) in chunk.iter_mut().enumerate() {
                            *c = i as u8;
                        }
                        for _ in 0..256 {
                            tx_writer.write_blocking(&chunk);
                        }
                        write!(&mut tx_writer, "\r\nuart_test done\r\n").ok();
                        tx_writer.write_blocking(&[]);
                    }
                    b'j' => {
                        // WAXWING: dump ~85 ms of per-PWM-cycle frames
                        // in the rinz cdump/Ascii85 format. Channels
                        // per frame: ch9=A (PA4), ch10=B (PA5), ch8=
                        // current (all mid-ON from the injected burst),
                        // ch99=status byte (bit0 COMP value, bits1-3
                        // sector). Under the GECKO-scope hybrid these
                        // come from the CPU-written CTX rings (the DMA
                        // ring now carries the oversampled free-run
                        // current, dumped by `J`) — all four rings
                        // advance in lockstep at the same TIM1_UP index.
                        // Snapshot under mask so a wrap can't tear the
                        // A/B/I/status alignment; motor keeps running.
                        let mut a = [0u16; PWM_SAMPLE_LEN];
                        let mut b = [0u16; PWM_SAMPLE_LEN];
                        let mut ci = [0u16; PWM_SAMPLE_LEN];
                        let mut status = [0u8; PWM_SAMPLE_LEN];
                        NVIC::mask(Interrupt::TIM1_UP_TIM16);
                        let pwm_head =
                            (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
                        for i in 0..PWM_SAMPLE_LEN {
                            a[i] = CTX_A[i].load(Ordering::Relaxed);
                            b[i] = CTX_B[i].load(Ordering::Relaxed);
                            ci[i] = CTX_I[i].load(Ordering::Relaxed);
                            status[i] = PWM_SAMPLE_BUF[i].load(Ordering::Relaxed);
                        }
                        unsafe { NVIC::unmask(Interrupt::TIM1_UP_TIM16) };

                        write!(
                            &mut tx_writer,
                            "\r\ncdump: {} frames b85 4 channels (ch9 ch10 ch8 ch99) \
                             12-bit, sample_hz={} Hz\r\n",
                            PWM_SAMPLE_LEN, PWM_FREQUENCY_HZ,
                        )
                        .ok();
                        // Framing (LE word packing, 2 a85 groups /
                        // frame, 80-char lines, "end") is host-tested
                        // in minz_core::dump; this site only supplies
                        // the ring accessors.
                        minz_core::dump::wax_cdump(
                            PWM_SAMPLE_LEN,
                            |k| {
                                let f = (pwm_head + k) & (PWM_SAMPLE_LEN - 1);
                                [a[f], b[f], ci[f], status[f] as u16]
                            },
                            |bb| tx_writer.write_blocking(bb),
                        );
                    }
                    b'G' => {
                        // GECKO-scope: on-demand dump of the free-run
                        // oversampled current ring (~150 samples/PWM
                        // cycle, ~0.55 ms). Same payload as the >4 A
                        // auto-trigger, but manual — for baseline shape
                        // and format checks. The oversample is normally
                        // OFF (inject-only lock), so start it, let it
                        // fill the ring (>0.55 ms), freeze → flat cdump
                        // (4 samples/frame, oldest-first) → stop.
                        adc_sync::oversample_start();
                        cortex_m::asm::delay(80_000); // ~1 ms at 80 MHz
                        let oldest = adc_sync::freeze_current();
                        write!(
                            &mut tx_writer,
                            "\r\ngecko: {} samples b85 isns oversampled 12-bit, adc_hz~3700000\r\n",
                            adc_sync::CUR_FRAMES,
                        )
                        .ok();
                        minz_core::dump::wax_cdump(
                            adc_sync::CUR_FRAMES / 4,
                            |k| {
                                let base = oldest + k * 4;
                                [
                                    adc_sync::cur_word(base),
                                    adc_sync::cur_word(base + 1),
                                    adc_sync::cur_word(base + 2),
                                    adc_sync::cur_word(base + 3),
                                ]
                            },
                            |bb| tx_writer.write_blocking(bb),
                        );
                        adc_sync::oversample_stop();
                    }
                    b'B' => {
                        // On-demand black-box dump (freeze → dump →
                        // thaw): read the live ACC commutation-delay
                        // values at speed to confirm the floor rework —
                        // ACC `d` should track the real delay, not sit
                        // pinned at the old 24 µs floor.
                        BB_FROZEN.store(true, Ordering::Relaxed);
                        write!(&mut tx_writer, "bb (on demand):\r\n").ok();
                        let bidx = BB_IDX.load(Ordering::Relaxed) as usize;
                        minz_core::blackbox::format_dump(
                            (0..BB_LEN).map(|k| {
                                let i = (bidx + k) % BB_LEN;
                                minz_core::blackbox::Event {
                                    t: BB_T[i].load(Ordering::Relaxed),
                                    ty: BB_TYPE[i].load(Ordering::Relaxed),
                                    sector: BB_SEC[i].load(Ordering::Relaxed),
                                    data: BB_DATA[i].load(Ordering::Relaxed),
                                }
                            }),
                            |b| tx_writer.write_blocking(b),
                        );
                        BB_FROZEN.store(false, Ordering::Relaxed);
                    }
                    b'e' => {
                        do_edge_dump = true;
                    }
                    b'E' => {
                        let was_frozen = EDGE_DUMP_FREEZE.load(Ordering::Relaxed);
                        if was_frozen {
                            EDGE_DUMP_FREEZE.store(false, Ordering::Relaxed);
                            write!(&mut tx_writer, "[edge buf RESUMED]\r\n").ok();
                        } else {
                            EDGE_DUMP_FREEZE.store(true, Ordering::Relaxed);
                            write!(&mut tx_writer, "[edge buf FROZEN]\r\n").ok();
                            do_edge_dump = true;
                        }
                    }
                    b'i' => {
                        // Current comes from the continuous PWM-
                        // synchronous sampler (GECKO); vbat from an
                        // on-demand injected conversion. Neither
                        // touches the HAL OneShot path (which would
                        // fight the trigger-armed regular channel).
                        let i_raw = LAST_I_RAW.load(Ordering::Relaxed);
                        let i_ma = minz_core::sense::isns_ma(sense_adc.adc_to_mv(i_raw) as u32);
                        // From the 6 kHz TIM7 pump — a blocking
                        // injected read here would race the pump's
                        // JADSTART/JEOS handling.
                        let v_raw = VBAT_RAW_LIVE.load(Ordering::Relaxed);
                        let v_supply_mv =
                            minz_core::sense::vbat_mv(sense_adc.adc_to_mv(v_raw) as u32);
                        // Worst vbat since the previous readout —
                        // makes between-rung transits visible (twice
                        // a real sag event was invisible to per-rung
                        // snapshots). Reset on read.
                        let v_min_raw = VBAT_MIN_RAW.swap(u16::MAX, Ordering::Relaxed);
                        let v_min_mv = if v_min_raw == u16::MAX {
                            0
                        } else {
                            minz_core::sense::vbat_mv(sense_adc.adc_to_mv(v_min_raw) as u32)
                        };
                        write!(
                            &mut tx_writer,
                            "vbat={}.{:03}V min={}.{:03}V isns={}.{:03}A cpu={}% (raw v={} i={})\r\n",
                            v_supply_mv / 1000,
                            v_supply_mv % 1000,
                            v_min_mv / 1000,
                            v_min_mv % 1000,
                            i_ma / 1000,
                            i_ma % 1000,
                            last_busy_pct,
                            v_raw,
                            i_raw,
                        )
                        .ok();
                        // Per-ISR rates since last `i` press — the
                        // wrapping-delta + ticks→per-second math is
                        // host-tested in minz_core::rates.
                        let now_tick = ticks_10us();
                        let now_comp = COMP_COUNT.load(Ordering::Relaxed);
                        let now_usart2 = USART2_COUNT.load(Ordering::Relaxed);
                        let now_tim7 = TIM7_COUNT.load(Ordering::Relaxed);
                        let now_tim1 = TIM1_UP_COUNT.load(Ordering::Relaxed);
                        let now_tim1_cc = TIM1_CC_COUNT.load(Ordering::Relaxed);
                        let now_miss = TIM1_UP_MISSED.load(Ordering::Relaxed);
                        let maxgap_cyc = TIM1_UP_MAXGAP_CYC.swap(0, Ordering::Relaxed);
                        let dtick = now_tick.wrapping_sub(last_i_tick);
                        if dtick > 0 {
                            let rate = |dc: u32| -> u32 {
                                minz_core::rates::rate_per_s(dc, dtick).unwrap_or(0)
                            };
                            write!(
                                &mut tx_writer,
                                "irq/s: comp={} tim1_up={} tim1_cc={} tim7={} \
                                 usart2={} (dt={}.{:02}s)  blank={}us\r\n",
                                rate(now_comp.wrapping_sub(last_i_comp)),
                                rate(now_tim1.wrapping_sub(last_i_tim1)),
                                rate(now_tim1_cc.wrapping_sub(last_i_tim1_cc)),
                                rate(now_tim7.wrapping_sub(last_i_tim7)),
                                rate(now_usart2.wrapping_sub(last_i_usart2)),
                                dtick / 100_000,
                                (dtick % 100_000) / 1000,
                                BLANK_US.load(Ordering::Relaxed),
                            )
                            .ok();
                            // TIM1_UP scheduling health over the same
                            // window: missed update events (DWT-gap
                            // detector at ISR entry) + worst gap.
                            write!(
                                &mut tx_writer,
                                "t1u: miss/s={} missed={} maxgap={}us clkback={}\r\n",
                                rate(now_miss.wrapping_sub(last_i_miss)),
                                now_miss,
                                maxgap_cyc / (minz::SYSCLK.raw() / 1_000_000),
                                CLOCK_BACK.load(Ordering::Relaxed),
                            )
                            .ok();
                            write!(
                                &mut tx_writer,
                                "dur cyc: t1u={} cc={} comp={} t7={} lp2={} main={}\r\n",
                                DUR_T1U.load(Ordering::Relaxed),
                                DUR_T1CC.load(Ordering::Relaxed),
                                DUR_COMP.load(Ordering::Relaxed),
                                DUR_TIM7.load(Ordering::Relaxed),
                                DUR_LPTIM2.load(Ordering::Relaxed),
                                DUR_MAIN.load(Ordering::Relaxed),
                            )
                            .ok();
                            // LPTIM2 ARR-sync safety tripwire (lever
                            // #2): must stay 0 — non-zero means the ARR
                            // write failed to sync into the kernel
                            // domain (the light re-arm's premise).
                            write!(
                                &mut tx_writer,
                                "arrok_guard_hits={}\r\n",
                                minz::lptim2_oneshot::ARROK_GUARD_HITS.load(Ordering::Relaxed),
                            )
                            .ok();
                        }
                        last_i_miss = now_miss;
                        last_i_tick = now_tick;
                        last_i_comp = now_comp;
                        last_i_usart2 = now_usart2;
                        if CL_ACTIVE.load(Ordering::Relaxed) {
                            let iv = OWL_INTERVAL_US.load(Ordering::Relaxed);
                            if iv != 0 {
                                write!(
                                    &mut tx_writer,
                                    "cl: ACTIVE f_e={}Hz interval={}us comms={}\r\n",
                                    1_000_000 / (6 * iv),
                                    iv,
                                    CL_COMM_COUNT.load(Ordering::Relaxed),
                                )
                                .ok();
                            }
                        }
                        last_i_tim7 = now_tim7;
                        last_i_tim1 = now_tim1;
                        last_i_tim1_cc = now_tim1_cc;
                    }
                    b'h' => {
                        // Cycle COMP2 hysteresis: 0 → 1 → 3 → 0 (skip
                        // the 2/medium step since the user only wants
                        // none / low / high). Wrapped in `free` so the
                        // .modify() race with TIM7's per-fire polarity
                        // write on the same CSR is closed.
                        let next = minz_core::ui::next_hyst(HYST_LEVEL.load(Ordering::Relaxed));
                        free(|_| {
                            HYST_LEVEL.store(next, Ordering::Relaxed);
                            comp2::set_hysteresis(next);
                        });
                        write!(
                            &mut tx_writer,
                            "hyst = {}\r\n",
                            minz_core::ui::hyst_name(next)
                        )
                        .ok();
                        tx_writer.write_blocking(&[]);
                    }
                    b'n' | b'N' | b'.' | b',' => {
                        // Software COMP-blanking window in µs. Keys:
                        //   `n` = +1 µs fine
                        //   `N` = -1 µs fine  (`m` is taken by mode toggle)
                        //   `.` = +10 µs coarse
                        //   `,` = -10 µs coarse
                        // Clamped to [0, 50] µs — one PWM cycle at
                        // 24 kHz is 41.67 µs, so 50 µs is the upper
                        // useful bound (above that we're gating
                        // entire cycles, including legitimate BEMF
                        // edges).
                        let next = minz_core::ui::blank_adjust(BLANK_US.load(Ordering::Relaxed), b);
                        BLANK_US.store(next, Ordering::Relaxed);
                        write!(
                            &mut tx_writer,
                            "blank window = {} µs (post-PWM-edge suppression)\r\n",
                            next,
                        )
                        .ok();
                        tx_writer.write_blocking(&[]);
                    }
                    b't' | b'T' => {
                        // Fine-grained commutation advance: t = +2°,
                        // T = −2°, clamped 0..=28. (Was a coarse
                        // 0/20/40/−40/−20 cycle from the open-loop
                        // observation days — useless for CL tuning,
                        // and negative advance is proven destructive:
                        // it collapses the rotor to a crawl.)
                        let next = minz_core::ui::advance_adjust(
                            ADVANCE_DEG.load(Ordering::Relaxed),
                            b == b't',
                        );
                        ADVANCE_DEG.store(next, Ordering::Relaxed);
                        write!(&mut tx_writer, "advance = {}°\r\n", next).ok();
                        tx_writer.write_blocking(&[]);
                    }
                    b'k' => {
                        // Cycle COMP2 EXTI edge mode 0..=5. Read
                        // current sector and apply for that sector
                        // immediately so the bits are correct even
                        // with motor off. Wrapped in `free` so the
                        // .modify() race with TIM7's per-fire
                        // `set_exti_edges` is closed.
                        let next = minz_core::ui::next_edge_mode(EDGE_MODE.load(Ordering::Relaxed));
                        free(|_| {
                            EDGE_MODE.store(next, Ordering::Relaxed);
                            let sector = CURRENT_SECTOR.load(Ordering::Relaxed);
                            let (re, fe) = edges_for(next, sector);
                            comp2::set_exti_edges(re, fe);
                        });
                        write!(
                            &mut tx_writer,
                            "edges = {}\r\n",
                            minz_core::ui::edge_mode_name(next)
                        )
                        .ok();
                        tx_writer.write_blocking(&[]);
                    }
                    _ => {}
                }
                // Edge-buffer dump (triggered by `e` or `E`-freeze).
                // Runs after the match so both keys share the same path.
                // Snapshot the frozen half under a brief TIM7+COMP mask
                // then format to UART with motor still running.
                if do_edge_dump {
                    let mut snap = [0u8; HALF_TICKS];
                    let mut sec_starts = [0u32; 12];
                    let mut sec_counts = [0u32; 12];
                    NVIC::mask(Interrupt::TIM7);
                    NVIC::mask(Interrupt::COMP);
                    let frozen = ((ACTIVE_HALF.load(Ordering::Relaxed) ^ 1) & 1) as usize;
                    for i in 0..HALF_TICKS {
                        snap[i] = EDGE_BUF[frozen][i].load(Ordering::Relaxed);
                    }
                    for i in 0..12 {
                        sec_starts[i] = SECTOR_BOUNDARIES[frozen][i].load(Ordering::Relaxed);
                        sec_counts[i] = SECTOR_EDGE_COUNT[frozen][i].load(Ordering::Relaxed);
                    }
                    let window_end_tick = HALF_START_TICK.load(Ordering::Relaxed);
                    unsafe { NVIC::unmask(Interrupt::COMP) };
                    unsafe { NVIC::unmask(Interrupt::TIM7) };
                    let buf = &snap;
                    let is_frozen = EDGE_DUMP_FREEZE.load(Ordering::Relaxed);

                    let edge_name =
                        minz_core::ui::edge_mode_short(EDGE_MODE.load(Ordering::Relaxed));

                    // Validity + header math host-tested in
                    // minz_core::dump::edge_dump_status.
                    match minz_core::dump::edge_dump_status(
                        &sec_starts,
                        window_end_tick,
                        electrical_hz,
                    ) {
                        minz_core::dump::EdgeDumpStatus::Invalid => {
                            write!(
                                &mut tx_writer,
                                "edges last 2 revs: no complete rev-pair captured yet \
                                 (let motor spin a few revs after arm/re-arm)\r\n",
                            )
                            .ok();
                        }
                        minz_core::dump::EdgeDumpStatus::MotorOff => {
                            write!(&mut tx_writer, "edges last 2 revs: motor off (f=0)\r\n",).ok();
                        }
                        minz_core::dump::EdgeDumpStatus::Ok { window_us } => {
                            write!(
                                &mut tx_writer,
                                "edges last 2 revs aligned to sec 0 \
                             (window={}us, hyst={}, edges={}, advance={}deg, blank={}us{}):\r\n",
                                window_us,
                                HYST_LEVEL.load(Ordering::Relaxed),
                                edge_name,
                                ADVANCE_DEG.load(Ordering::Relaxed),
                                BLANK_US.load(Ordering::Relaxed),
                                if is_frozen { ", FROZEN" } else { "" },
                            )
                            .ok();
                            // Sector chunking + glyph rendering are
                            // host-tested in minz_core::dump.
                            minz_core::dump::edge_dump_body(
                                buf,
                                &sec_starts,
                                &sec_counts,
                                window_end_tick,
                                |b| tx_writer.write_blocking(b),
                            );
                        }
                    }
                }
                // Publish any changes to the motor-drive atomics and
                // print a one-line confirmation. The TIM7 ISR picks
                // these up on its next tick.
                if amplitude_pct != prev_amp {
                    // Keys set the TARGET; TIM7 slews the applied
                    // duty toward it at 1 %/50 ms (roadmap item 5).
                    // Step transients at speed were tripping the
                    // current envelope (a +3 step at 1,285 Hz pulls
                    // an 85 ms surge past even 3.8× the healthy
                    // curve); with the slew the loop never sees a
                    // step at all.
                    AMP_TARGET_PCT.store(amplitude_pct as u8, Ordering::Relaxed);
                    write!(&mut tx_writer, "amp={}\r\n", amplitude_pct).ok();
                }
                if electrical_hz != prev_hz {
                    ANGLE_INC.store(angle_inc_fp(electrical_hz), Ordering::Relaxed);
                    // Under CL the gate belongs to the loop (30 % of
                    // the MEASURED interval) — an open-loop half-
                    // window value here would yank one window's gate
                    // to a nonsense position mid-lock (roadmap B5).
                    if !CL_ACTIVE.load(Ordering::Relaxed) {
                        SECTOR_GATE_US.store(open_loop_gate_us(electrical_hz), Ordering::Relaxed);
                    }
                    write!(&mut tx_writer, "f={}\r\n", electrical_hz).ok();
                }
                if waveform != prev_mode {
                    // Switching to sine kills the BEMF observation
                    // window (no float phase), so zero the mask so
                    // the ISR doesn't try to gate EXTI. Switching to
                    // six-step re-installs the observed phase's mask.
                    let new_six_step = matches!(waveform, Waveform::SixStep);
                    SIX_STEP_MODE.store(new_six_step, Ordering::Relaxed);
                    FLOAT_SECTOR_MASK.store(
                        if new_six_step {
                            float_sector_mask(observed_phase)
                        } else {
                            0
                        },
                        Ordering::Relaxed,
                    );
                    write!(&mut tx_writer, "mode={}\r\n", mode_name(waveform)).ok();
                }
            }
            // MAGPIE drain: emit completed window records as 16-byte
            // frames. Emit all-or-nothing per frame — a partial frame
            // would desync the host parser; a dropped one just shows
            // as a seq gap.
            while let Some(rec) = wrec_consumer.dequeue() {
                if TX_RING_LEN - 1 - tx_writer.pending() >= minz_core::wire::FRAME_LEN_V4 {
                    for b in rec.encode() {
                        let _ = tx_writer.push(b);
                    }
                }
            }
            tx_writer.service();

            next_microloop += MICROLOOP_TICKS;
        }

        // 1-second boundary: latch idle counter, snapshot COMP rates.
        // Both windows are sized to `SECOND_TICKS` so they match the
        // `IdleLoop` calibration window exactly.
        last_busy_pct = idle_loop.busy_percentage(idle_loop.latch());

        let now_count = COMP_COUNT.load(Ordering::Relaxed);
        let now_valid = VALID_COMP_COUNT.load(Ordering::Relaxed);
        COMP_RATE.store(comp_rate_win.latch(now_count), Ordering::Relaxed);
        VALID_COMP_RATE.store(valid_rate_win.latch(now_valid), Ordering::Relaxed);

        epoch_start = next_epoch;
    }
}

#[inline]
fn mode_name(w: Waveform) -> &'static str {
    match w {
        Waveform::Sine => "sine",
        Waveform::SixStep => "six-step",
    }
}

#[inline]
fn clamp_amp(v: i32) -> u16 {
    minz_core::ui::clamp_pct(v, AMP_MIN, AMP_MAX)
}

#[inline]
fn clamp_hz(v: i32) -> u32 {
    minz_core::ui::clamp_hz(v, FREQ_MIN, FREQ_MAX)
}

// SysTick handler retired (lever #1b): the wall clock is DWT.CYCCNT,
// extended in `cyc_extend` from the TIM1_UP ISR. No periodic tick ISR.

#[interrupt]
fn USART2() {
    // Host-key receiver. Reading RDR clears RXNE. ORE must be cleared
    // explicitly or the IRQ storms (overrun shares the RXNEIE enable).
    // Framing/noise flags can't raise this IRQ (EIE is off) but are
    // cleared alongside so they never sit stale in ISR.
    USART2_COUNT.fetch_add(1, Ordering::Relaxed);
    let usart = unsafe { &*stm32::USART2::ptr() };
    let isr = usart.isr.read();
    if isr.ore().bit_is_set() || isr.fe().bit_is_set() || isr.nf().bit_is_set() {
        usart
            .icr
            .write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
    }
    if isr.rxne().bit_is_set() {
        let b = usart.rdr.read().bits() as u8;
        free(|cs| {
            let mut prod = RX_PROD.borrow(cs).borrow_mut();
            if let Some(p) = prod.as_mut() {
                // Drop on overflow — keystroke channel, not critical.
                let _ = p.enqueue(b);
            }
        });
    }
}

#[interrupt]
fn TIM7() {
    let _dur = DurGuard::new(&DUR_TIM7);
    // Motor-drive heartbeat (6 kHz). When armed: advance the electrical
    // angle accumulator, commutate (six-step or sine), update polarity
    // / EXTI-edge config for the new sector, publish CURRENT_SECTOR.
    // When disarmed: still update polarity / edges for the **last**
    // sector (so `y` / `k` mode changes take effect within one TIM7
    // tick even with the motor off).
    //
    // Ordering matters once NVIC priorities are programmed (COMP=1 can
    // now preempt TIM7=2): the polarity + edge-mask writes happen
    // BEFORE the `CURRENT_SECTOR` store, with a `Release` fence between
    // them, so a COMP edge that fires immediately after the sector
    // store observes the matching polarity + edges (not stale ones).
    tim7_drive::clear_update_flag();
    let tick = TIM7_COUNT.fetch_add(1, Ordering::Relaxed);

    // Throttle slew limiter (roadmap item 5): the APPLIED duty walks
    // toward the key-set target at 1 % per 50 ms (every 300th tick
    // at 6 kHz). The loop never sees a throttle STEP — step
    // transients at speed pulled 85 ms current surges past even
    // 3.8× the healthy envelope. Kills (`w`, guards) bypass this
    // entirely via MOTOR_ENABLED/all_off.
    if tick.is_multiple_of(minz_core::throttle::STEP_TICKS) {
        // Host-tested: minz_core::throttle (incl. the snap-on-arm
        // regression that cost 4/4 engages).
        AMPLITUDE_PCT.store(
            minz_core::throttle::step(
                AMPLITUDE_PCT.load(Ordering::Relaxed),
                AMP_TARGET_PCT.load(Ordering::Relaxed),
            ),
            Ordering::Relaxed,
        );
    }

    // FALCON: while the closed loop drives, the crystal stepper is
    // frozen — LPTIM2 owns sector changes, mux, and window close.
    // TIM7 keeps two jobs: duty refresh (live amp keys) and the
    // desync watchdog (no qualified ZC for 3 intervals → kill and
    // coast; a fallback ramp jolt has cost this bench a motor before).
    if CL_ACTIVE.load(Ordering::Relaxed) {
        if MOTOR_ENABLED.load(Ordering::Relaxed) {
            let duty =
                open_loop::six_step_duty(max_duty(), AMPLITUDE_PCT.load(Ordering::Relaxed) as u16);
            tim1_motor_pwm::set_six_step(CURRENT_SECTOR.load(Ordering::Relaxed), duty);
            let interval_us = OWL_INTERVAL_US.load(Ordering::Relaxed);
            // Read LAST_COMM *before* now: a commutation preempting us
            // between the two reads then only makes `since` slightly
            // stale-large (≤ one TIM7 tick), never underflowed. The
            // black box caught the original order killing a healthy
            // 309 Hz lock with since = u32-underflow garbage 20 µs
            // after a refined commutation. Top-bit clamp for belt and
            // suspenders.
            // Host-tested: minz_core::guards::{since_us, cl_watchdog}
            // — read-order rule, starvation, runaway floor, desync
            // (each constant traces to an incident; see core tests).
            // One time read for both checks — taken AFTER both
            // reference loads so the reference-before-now rule holds
            // for each (hoisting `now` above the references would
            // reintroduce the underflow race this block documents).
            let last = LAST_COMM_10US.load(Ordering::Relaxed);
            let last_qzc = LAST_QZC_10US.load(Ordering::Relaxed);
            let now_10 = ticks_10us();
            let since_us = minz_core::guards::since_us(now_10, last);
            let starve_us = minz_core::guards::since_us(now_10, last_qzc);
            let kill =
                minz_core::guards::cl_watchdog(interval_us, starve_us, since_us, last_qzc != 0);
            if let Some(kind) = kill {
                let starved = kind == minz_core::guards::Kill::ZcStarved;
                bb_record(
                    if starved {
                        minz_core::blackbox::EV_STV
                    } else {
                        minz_core::blackbox::EV_DSY
                    },
                    CURRENT_SECTOR.load(Ordering::Relaxed),
                    ((if starved { starve_us } else { since_us }) / 10).min(0xFFFF) as u16,
                );
                // Flag matrix host-pinned in minz_core::guards (this
                // site used to miss the CL_ARMED clear).
                minz_core::guards::apply_isr_kill(
                    &KILL_FLAGS,
                    if starved {
                        minz_core::guards::IsrKillKind::Starved
                    } else {
                        minz_core::guards::IsrKillKind::Desync
                    },
                );
                tim1_motor_pwm::all_off();
                comp2::set_exti_enabled(false);
            }
        }
        return;
    }

    let motor_enabled = MOTOR_ENABLED.load(Ordering::Relaxed);
    let prev_sector = CURRENT_SECTOR.load(Ordering::Relaxed);

    // Determine which sector this tick's polarity / edge config applies
    // to. Armed → compute new sector from advancing angle, with the
    // commutation-advance offset folded in. Disarmed → reuse the last
    // sector so static polarity / edges for modes 2/3 stay sensible.
    let (sector, angle, sector_changed) = if motor_enabled {
        // Accumulator advance + signed-advance folding host-tested in
        // minz_core::drive::angle_tick (#[inline] — verified to
        // inline into this ISR; see the piecewise-rewire notes).
        let (accum, commutation_angle) = minz_core::drive::angle_tick(
            ANGLE_ACCUM.load(Ordering::Relaxed),
            ANGLE_INC.load(Ordering::Relaxed),
            ADVANCE_DEG.load(Ordering::Relaxed) as i32,
        );
        ANGLE_ACCUM.store(accum, Ordering::Relaxed);
        let new_sector = open_loop::six_step_sector(commutation_angle);
        (new_sector, commutation_angle, new_sector != prev_sector)
    } else {
        (prev_sector, 0, false)
    };
    let rev_wrapped = sector_changed && minz_core::drive::is_rev_wrap(prev_sector, sector);

    // Apply COMP2 EXTI edges for this sector + edge-mode. COMP2
    // polarity is locked to non-inverted (POLARITY=0); only the EXTI
    // edge selection changes per sector for modes 3/4.
    let edge_mode = EDGE_MODE.load(Ordering::Relaxed);
    let (rising_en, falling_en) = edges_for(edge_mode, sector);
    comp2::set_exti_edges(rising_en, falling_en);

    // Release fence so COMP ISR (which can now preempt this ISR) reads
    // the new polarity / edge config when it samples CURRENT_SECTOR.
    core::sync::atomic::compiler_fence(Ordering::Release);
    CURRENT_SECTOR.store(sector, Ordering::Relaxed);

    if !motor_enabled {
        return;
    }

    // Scope trigger: toggle PB3 once per electrical revolution so the
    // output square wave runs at exactly the commanded frequency F.
    if rev_wrapped {
        let was_high = PB3_LEVEL.fetch_not(Ordering::Relaxed);
        minz::pb3::set(!was_high);
    }

    // EDGE_BUF rev-pair flip + sector-boundary recording, both inside
    // a critical section so COMP ISR (higher priority) can't snapshot
    // a torn (ACTIVE_HALF, HALF_START_TICK, REV_PHASE) tuple. The
    // flip state machine (pair cadence, freeze semantics, slot
    // placement) is host-tested in minz_core::edgebuf (#[inline]);
    // the buffer clears + boundary stores run here from its plan.
    if rev_wrapped || sector_changed {
        free(|_| {
            if rev_wrapped
                && let Some(plan) = minz_core::edgebuf::on_rev_wrap(&EDGE_HALVES, ticks_10us())
            {
                // Clear the new active half; the previous pair's
                // data stays in the soon-to-be-frozen half.
                unsafe {
                    core::ptr::write_bytes(
                        EDGE_BUF[plan.new_active].as_ptr() as *mut u8,
                        0,
                        HALF_TICKS,
                    );
                }
                for slot in 0..12 {
                    SECTOR_EDGE_COUNT[plan.new_active][slot].store(0, Ordering::Relaxed);
                }
                // Slot-0 validity invariant (see edgebuf docs).
                SECTOR_BOUNDARIES[plan.old_active][0].store(plan.old_half_start, Ordering::Relaxed);
                SECTOR_BOUNDARIES[plan.new_active][0]
                    .store(HALF_START_TICK.load(Ordering::Relaxed), Ordering::Relaxed);
            }
            // Record the sector-boundary tick into the (now possibly
            // rotated) half/phase slot.
            if sector_changed
                && let Some(slot) =
                    minz_core::edgebuf::boundary_slot(REV_PHASE.load(Ordering::Relaxed), sector)
            {
                let active = ACTIVE_HALF.load(Ordering::Relaxed) as usize;
                SECTOR_BOUNDARIES[active][slot].store(ticks_10us(), Ordering::Relaxed);
            }
        });
    }

    let amp = AMPLITUDE_PCT.load(Ordering::Relaxed) as u16;
    let arr = max_duty();

    if SIX_STEP_MODE.load(Ordering::Relaxed) {
        let duty = open_loop::six_step_duty(arr, amp);
        tim1_motor_pwm::set_six_step(sector, duty);

        if AUTO_MUX.load(Ordering::Relaxed) {
            // CHAMELEON: every sector is a float window for *some*
            // phase — route that phase to COMP2 at each sector entry,
            // discard whatever edge the switchover latched, and
            // restart the window clock. The mux settles in <5 µs;
            // the half-sector time gate (≥139 µs even at f=600)
            // discards that transient region with orders of margin,
            // so no blocking delay in this ISR.
            if sector_changed {
                comp2::set_inm(SECTOR_FLOAT_PHASE[sector as usize]);
                comp2::clear_pending();
                // MAGPIE: package the just-ended window, then reset
                // the accumulators for the new one. `free` closes the
                // race with the higher-priority COMP ISR, which would
                // otherwise smear one edge across the old/new window
                // during the snapshot-and-reset.
                free(|cs| close_float_window(cs, prev_sector));
                WAS_IN_FLOAT_SECTOR.store(true, Ordering::Relaxed);
            }
        } else {
            // Legacy single-phase observation: track entry into the
            // `p`-selected phase's float window so the COMP-ISR's
            // time-window gate (`valid` rate counter) has a meaningful
            // `SECTOR_START_TICK` reference. EXTI is not gated here —
            // edges are recorded into `EDGE_BUF` across the whole rev.
            // Edge-detect host-tested: minz_core::drive::float_entry.
            let (entered, in_float) = minz_core::drive::float_entry(
                FLOAT_SECTOR_MASK.load(Ordering::Relaxed),
                sector,
                WAS_IN_FLOAT_SECTOR.load(Ordering::Relaxed),
            );
            if entered {
                SECTOR_START_US.store(ticks_1us(), Ordering::Relaxed);
            }
            WAS_IN_FLOAT_SECTOR.store(in_float, Ordering::Relaxed);
        }
    } else {
        // Sine: continuous 3-phase, no float window.
        let (c1, c2, c3) = open_loop::sine_duties(angle, arr, amp);
        tim1_motor_pwm::set_duties(c1, c2, c3);
        WAS_IN_FLOAT_SECTOR.store(false, Ordering::Relaxed);
    }
}

/// The core close's view of this file's statics — wired once here;
/// host tests build the same struct over locals. See
/// `minz_core::window` for the logic (reacq trigger, pred_err,
/// decimation, accumulator reset list — all host-tested).
static WINDOW_STATE: minz_core::window::WindowState<'static> = minz_core::window::WindowState {
    sector_start_us: &SECTOR_START_US,
    qzc_us: &WINDOW_QZC_US,
    first_zc_us: &WINDOW_FIRST_ZC_US,
    raw: &WINDOW_RAW,
    valid: &WINDOW_VALID,
    i_sum: &WINDOW_I_SUM,
    i_n: &WINDOW_I_N,
    i_min: &WINDOW_I_MIN,
    i_max: &WINDOW_I_MAX,
    cand_zc_us: &CAND_ZC_US,
    window_gen: &WINDOW_GEN,
    interval_us: &OWL_INTERVAL_US,
    last_qzc_us: &OWL_LAST_QZC_US,
    windows_since_qzc: &WINDOWS_SINCE_QZC,
    cl_active: &CL_ACTIVE,
    cl_noz_run: &CL_NOZ_RUN,
    cl_reacq: &CL_REACQ,
    stream_on: &STREAM_ON,
    wrec_decim: &WREC_DECIM,
    wrec_seq: &WREC_SEQ,
    vbat_min: &WINDOW_VBAT_MIN,
    vbat_live: &VBAT_RAW_LIVE,
    last_comm_10us: &LAST_COMM_10US,
};

/// MAGPIE/OWL float-window close: called at every commutation, from
/// whichever engine performed it — TIM7 (open loop) or the LPTIM2
/// one-shot (closed loop) — always inside a critical section so the
/// higher/equal-priority COMP ISR can't smear an edge across the
/// old/new window during snapshot-and-reset. This site supplies the
/// clocks and performs the two returned side effects.
/// The EDGE_BUF half-flip state machine's view of this file's
/// statics (pair cadence, freeze, slot placement host-tested in
/// minz_core::edgebuf; used inside TIM7's `free` block only).
static EDGE_HALVES: minz_core::edgebuf::EdgeHalves<'static> = minz_core::edgebuf::EdgeHalves {
    rev_phase: &REV_PHASE,
    active_half: &ACTIVE_HALF,
    half_start_tick: &HALF_START_TICK,
    freeze: &EDGE_DUMP_FREEZE,
};

/// The mode state machine's view of this file's statics (arm/kill/
/// CL transitions host-tested in minz_core::mode).
static MODE_STATE: minz_core::mode::ModeState<'static> = minz_core::mode::ModeState {
    motor_enabled: &MOTOR_ENABLED,
    cl_active: &CL_ACTIVE,
    cl_armed: &CL_ARMED,
    cl_reacq: &CL_REACQ,
    cl_noz_run: &CL_NOZ_RUN,
    interval_us: &OWL_INTERVAL_US,
    last_qzc_us: &OWL_LAST_QZC_US,
    windows_since_qzc: &WINDOWS_SINCE_QZC,
    amplitude_pct: &AMPLITUDE_PCT,
    amp_target_pct: &AMP_TARGET_PCT,
    vbat_baseline_raw: &VBAT_BASELINE_RAW,
    vbat_live: &VBAT_RAW_LIVE,
};

/// The ZC candidate/confirm/accept state machine's view of this
/// file's statics (minz_core::zc — incl. the B1 TOCTOU regression).
static ZC_STATE: minz_core::zc::ZcState<'static> = minz_core::zc::ZcState {
    cand_zc_us: &CAND_ZC_US,
    cand_expected: &CAND_EXPECTED,
    cand_confirms: &CAND_CONFIRMS,
    cand_gen: &CAND_GEN,
    window_gen: &WINDOW_GEN,
    window_qzc_us: &WINDOW_QZC_US,
    interval_us: &OWL_INTERVAL_US,
    last_qzc_us: &OWL_LAST_QZC_US,
    windows_since_qzc: &WINDOWS_SINCE_QZC,
    last_qzc_10us: &LAST_QZC_10US,
    cl_active: &CL_ACTIVE,
    cl_armed: &CL_ARMED,
    cl_reacq: &CL_REACQ,
    cl_noz_run: &CL_NOZ_RUN,
    cl_fast_path: &CL_FAST_PATH,
};

/// ISR-kill flag matrix (minz_core::guards::apply_isr_kill) — the
/// zombie-flag class is host-pinned there.
static KILL_FLAGS: minz_core::guards::KillFlags<'static> = minz_core::guards::KillFlags {
    motor_enabled: &MOTOR_ENABLED,
    cl_active: &CL_ACTIVE,
    cl_armed: &CL_ARMED,
    cl_desync: &CL_DESYNC,
    cl_starved: &CL_STARVED,
    oc_tripped: &OC_TRIPPED,
    vbat_sagged: &VBAT_SAGGED,
    bb_frozen: &BB_FROZEN,
};

/// Shuttle a mode command through the core state machine and apply
/// its actions (peripheral calls + prints) in the contract order:
/// arm_output before EXTI-enable, all_off before EXTI-mask.
fn mode_cmd(
    cmd: minz_core::mode::Cmd,
    output_enabled: &mut bool,
    waveform: &mut Waveform,
    electrical_hz: &mut u32,
    amplitude_pct: &mut u16,
    tx_writer: &mut UartTxWriter,
) {
    use minz_core::mode::{Mirror, Msg, step};
    let mut mir = Mirror {
        output_enabled: *output_enabled,
        six_step: matches!(*waveform, Waveform::SixStep),
        hz: *electrical_hz,
        amp_pct: *amplitude_pct,
    };
    let a = step(&MODE_STATE, &mut mir, cmd);
    *output_enabled = mir.output_enabled;
    *waveform = if mir.six_step {
        Waveform::SixStep
    } else {
        Waveform::Sine
    };
    *electrical_hz = mir.hz;
    *amplitude_pct = mir.amp_pct;
    if a.arm_output {
        tim1_motor_pwm::arm_output();
    }
    if a.all_off {
        tim1_motor_pwm::all_off();
    }
    if let Some(on) = a.exti {
        comp2::set_exti_enabled(on);
    }
    let msg: &str = match a.msg {
        // hz/amp/mode changes print via the delta-publish block.
        Msg::None | Msg::ModeToggled => "",
        Msg::Armed => "armed\r\n",
        Msg::Off => "off\r\n",
        Msg::ClBlocked => "CL armed/active - 'y' first\r\n",
        Msg::ClOffKilled => "CL off - output killed (r/q re-arms open loop)\r\n",
        Msg::ClArmed => "CL ARMED - engaging at next qualified ZC\r\n",
        Msg::ClNeedsDrive => "CL needs a running six-step drive first (r/q)\r\n",
    };
    if !msg.is_empty() {
        tx_writer.write_blocking(msg.as_bytes());
    }
}

fn close_float_window(cs: &cortex_m::interrupt::CriticalSection, prev_sector: u8) {
    let (now_10, now_us) = ticks_both();
    let out = minz_core::window::close_float_window(&WINDOW_STATE, prev_sector, now_10, now_us);
    for (ev, data) in out.bb.iter().flatten() {
        bb_record(*ev, prev_sector, *data);
    }
    if let Some(rec) = out.rec {
        let mut prod = WREC_PROD.borrow(cs).borrow_mut();
        if let Some(p) = prod.as_mut() {
            // Drop on overflow — host sees the seq gap.
            let _ = p.enqueue(rec);
        }
    }
}

/// Round-robin counter for the high-speed telemetry decimation.
static WREC_DECIM: AtomicU32 = AtomicU32::new(0);

/// FALCON commutation ISR — fires `interval·(30°−adv)/60°` after each
/// qualified ZC (scheduled by the COMP ISR). Shares priority 1 with
/// COMP so the two never nest.
#[interrupt]
fn LPTIM2() {
    let _dur = DurGuard::new(&DUR_LPTIM2);
    minz::lptim2_oneshot::clear_flag();
    if !CL_ACTIVE.load(Ordering::Relaxed) || !MOTOR_ENABLED.load(Ordering::Relaxed) {
        return; // stale one-shot after disengage/kill
    }
    CL_COMM_COUNT.fetch_add(1, Ordering::Relaxed);
    let prev = CURRENT_SECTOR.load(Ordering::Relaxed);
    {
        // Black box: classify the commutation by the window it ends
        // (REF/BLD/DRK table host-tested in minz_core::drive).
        let refined = SHOT_REFINED.swap(false, Ordering::Relaxed);
        bb_record(
            minz_core::drive::commutation_class(prev, refined),
            prev,
            OWL_INTERVAL_US.load(Ordering::Relaxed).min(0xFFFF) as u16,
        );
    }
    let sector = minz_core::drive::next_sector(prev);
    // FIX #2: clamp the commanded amplitude when flying blind (a
    // sustained ZC-miss cascade) so the monster current ramp can't
    // sag-kill. Isolated misses ride through at full drive. SPEED-
    // gated to the high-speed regime where monsters exist (interval <
    // HIGH_SPEED_US): at engage / low speed, misses are normal and
    // clamping the amp starves the torque needed to lock (bisected:
    // the ungated clamp took engage to 1/6 on a healthy bench). Full
    // drive below the threshold.
    let base_amp = AMPLITUDE_PCT.load(Ordering::Relaxed) as u16;
    let iv = OWL_INTERVAL_US.load(Ordering::Relaxed);
    let amp = if iv > 0 && iv < minz_core::window::HIGH_SPEED_US {
        minz_core::guards::blind_amp_clamp(base_amp, CL_NOZ_RUN.load(Ordering::Relaxed))
    } else {
        base_amp
    };
    let duty = open_loop::six_step_duty(max_duty(), amp);
    tim1_motor_pwm::set_six_step(sector, duty);
    comp2::set_inm(SECTOR_FLOAT_PHASE[sector as usize]);
    let (re, fe) = edges_for(EDGE_MODE.load(Ordering::Relaxed), sector);
    comp2::set_exti_edges(re, fe);
    core::sync::atomic::compiler_fence(Ordering::Release);
    CURRENT_SECTOR.store(sector, Ordering::Relaxed);
    free(|cs| close_float_window(cs, prev));
    // Adaptive time gate: earliest acceptable ZC at 40 % of the
    // MEASURED interval (not the stale commanded-f half-window) so an
    // accelerating rotor's earlier-arriving ZC stays in bounds while
    // early-window transients stay out.
    let interval = OWL_INTERVAL_US.load(Ordering::Relaxed);
    if interval != 0 {
        // Adaptive gate: 20 % of the measured interval normally; in
        // re-acquisition drop to ~8 % (just past the commutation
        // flyback) so ZCs that drifted early — the lockout-spiral
        // signature — become acceptable again.
        // Host-tested: minz_core::timing::gate_us (30 % normal, 8 %
        // re-acquisition; history in the core docs + tests).
        SECTOR_GATE_US.store(
            minz_core::timing::gate_us(interval, CL_REACQ.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }
    // Free-run schedule: exactly 1.0× the estimator interval (AM32
    // semantics — an accepted ZC merely RE-TIMES the pending shot;
    // the 1.5×T-fallback compounded-lag incident is a named
    // regression on minz_core::drive::freerun_reschedule_us).
    // Lever #2 (validated 2026-07-13): this runs at the ARR match
    // (counter STOPPED), so the LIGHT re-arm is valid — no pending
    // count to cancel, kernel warm. Saves ~310 cyc + a 2.5 µs
    // busy-wait per commutation vs the full disable/enable path.
    if let Some(t) = minz_core::drive::freerun_reschedule_us(interval) {
        minz::lptim2_oneshot::reschedule_light(t);
    }
    // Scope trigger on each electrical rev, same as the open loop.
    if sector == 0 {
        let was_high = PB3_LEVEL.fetch_not(Ordering::Relaxed);
        minz::pb3::set(!was_high);
    }
    // Re-open the ear for the new window (clears any pending edge).
    comp2::set_exti_enabled(true);
}

#[interrupt]
fn TIM1_CC() {
    let _dur = DurGuard::new(&DUR_T1CC);
    // Fires whenever TIM1.CNT matches CCR1 / CCR2 / CCR3 (whichever
    // CCxIE we enabled — see `tim1_motor_pwm::enable_cc_interrupts`).
    // CCR4 is intentionally NOT enabled — it's the fixed ADC-TRGO
    // value `TIM1_CCR4_TRGO`, not a PWM transition.
    //
    // In PWM mode 1 with edge-aligned counting, the compare match is
    // exactly the high-to-low transition of OCxREF (= falling edge
    // of the channel's output). We latch the current 10 µs tick into
    // `LAST_PWM_EDGE` so the COMP ISR can suppress events that fall
    // within `BLANK_TICKS_10US` of any PWM transition.
    //
    // Effective fire rate at our 24 kHz PWM is up to 3 × 24 kHz =
    // 72 kHz, since each PWM cycle hits CC1 + CC2 + CC3 once. That's
    // the cost paid for being duty- and sector-agnostic: we don't
    // need to know which channel is the active high-side one,
    // because we latch on every transition regardless.
    tim1_motor_pwm::clear_cc_flags();
    TIM1_CC_COUNT.fetch_add(1, Ordering::Relaxed);
    LAST_PWM_EDGE_US.store(ticks_1us(), Ordering::Relaxed);
}

#[interrupt]
fn TIM1_UP_TIM16() {
    let _dur = DurGuard::new(&DUR_T1U);
    // One-byte COMP2 + sector sample per PWM period. UIF must be
    // cleared first or the IRQ re-fires immediately on return.
    // Byte layout: bit 0 = COMP2 value, bits 1..=3 = sector (0..=5).
    TIM1_UP_COUNT.fetch_add(1, Ordering::Relaxed);
    tim1_motor_pwm::clear_update_flag();
    // Miss detector: entry-to-entry gap in DWT cycles, rounded to PWM
    // periods. ≥2 periods = coalesced UIF = missed cycle(s). Sole
    // writer of LAST_CYC is this ISR, so plain load/store is fine.
    {
        let now_cyc = cortex_m::peripheral::DWT::cycle_count();
        // Wall-clock 64-bit wrap extension (lever #1b): this 24 kHz
        // ISR is the sole writer of the CYCCNT wrap counter and runs
        // every ~41 µs, so it never misses a 53.7 s CYCCNT wrap.
        // Shares the CYCCNT read with the miss detector.
        cyc_extend(now_cyc);
        let last = TIM1_UP_LAST_CYC.load(Ordering::Relaxed);
        TIM1_UP_LAST_CYC.store(now_cyc, Ordering::Relaxed);
        let gap = now_cyc.wrapping_sub(last);
        const PERIOD_CYC: u32 = minz::TIM1_AUTORELOAD as u32 + 1;
        // Ignore the boot-first sample and anything absurd (>50 ms:
        // a halt/dump artifact, not a scheduling miss).
        if last != 0 && gap < 4_000_000 {
            if gap > TIM1_UP_MAXGAP_CYC.load(Ordering::Relaxed) {
                TIM1_UP_MAXGAP_CYC.store(gap, Ordering::Relaxed);
            }
            let periods = (gap + PERIOD_CYC / 2) / PERIOD_CYC;
            if periods >= 2 {
                TIM1_UP_MISSED.fetch_add(periods - 1, Ordering::Relaxed);
            }
        }
    }
    // Harvest the PWM-synchronous current sample converted earlier in
    // this cycle (OC4REF falling edge at CNT=SAMPLE_TICKS triggered
    // it; by the update event it finished long ago). Sole writer of
    // the window accumulators between TIM7's `free`-wrapped resets,
    // and TIM7 (lower priority since the 2026-07-13 swap — this
    // invariant was FALSE while TIM7 sat at level 2) can't interrupt
    // us — plain load/store min/max is race-free.
    let (pa_a, pa_b, i_raw, vbat_raw) = adc_sync::inj_read();

    // FIRMWARE SAG KILL — vbat is now the 4th injected channel, so it
    // arrives mid-ON alongside A/B/current every cycle (was a separate
    // 8.2 µs software pump that collided with the phase sequence ~40 %
    // of the time and corrupted sector confirms on two motors). Same
    // one-per-cycle rate as the old wrap-slot pump, so the debounce
    // timing is unchanged. Kill at −10 % from the arm baseline (or the
    // brownout backstop), debounced 64 consecutive samples = 1.3 ms.
    {
        let raw = vbat_raw;
        VBAT_RAW_LIVE.store(raw, Ordering::Relaxed);
        if raw < VBAT_MIN_RAW.load(Ordering::Relaxed) {
            VBAT_MIN_RAW.store(raw, Ordering::Relaxed);
        }
        if raw < WINDOW_VBAT_MIN.load(Ordering::Relaxed) {
            WINDOW_VBAT_MIN.store(raw, Ordering::Relaxed);
        }
        // Threshold + debounce host-tested through SagGuard's suite
        // via the load-run-store form (minz_core::guards::sag_step);
        // flag matrix via apply_isr_kill.
        if MOTOR_ENABLED.load(Ordering::Relaxed) {
            let (run, trip) = minz_core::guards::sag_step(
                VBAT_BASELINE_RAW.load(Ordering::Relaxed),
                VBAT_SAG_RUN.load(Ordering::Relaxed),
                raw,
            );
            VBAT_SAG_RUN.store(run, Ordering::Relaxed);
            if trip {
                VBAT_TRIP_RAW_SEEN.store(raw, Ordering::Relaxed);
                minz_core::guards::apply_isr_kill(&KILL_FLAGS, minz_core::guards::IsrKillKind::Sag);
                tim1_motor_pwm::all_off();
                comp2::set_exti_enabled(false);
            }
        } else {
            VBAT_SAG_RUN.store(0, Ordering::Relaxed);
        }
    }
    // Decaying-max vbus estimate — host-tested in minz_core::zc
    // (timescale + no-rail-sector ride-through pinned there).
    VBUS_EST.store(
        minz_core::zc::vbus_decay_step(VBUS_EST.load(Ordering::Relaxed), pa_a, pa_b),
        Ordering::Relaxed,
    );
    LAST_I_RAW.store(i_raw, Ordering::Relaxed);
    // Analog black box trigger — freeze the wire the moment a spike
    // is seen (one-shot; the ADSTP wait is <1 µs at these sample
    // times). Frozen means LAST_I_RAW goes stale until the dump
    // resumes the ring — acceptable for a ~100 ms dump.
    //
    // Trigger on the INTRA-CYCLE PEAK, not the mid-ON `i_raw`. When
    // armed the free-run oversample is on (~150 samples added per PWM
    // cycle); the single mid-ON point misses a spike between samples.
    // Scan the new free-run samples (stride 4 — a real amp-draw event
    // is sustained across a whole window, i_avg≈i_max in the sweep
    // data, so decimating can't miss it) for the max and trigger on
    // that. This is the fix that lets the microscope catch what the
    // mid-ON telemetry can't.
    let armed = WAX_TRIG_ARMED.load(Ordering::Relaxed);
    let mut trig_raw = i_raw;
    if armed {
        let head = adc_sync::cur_head();
        let last = LAST_CUR_HEAD.load(Ordering::Relaxed) as usize;
        let n = (head + adc_sync::CUR_FRAMES - last) % adc_sync::CUR_FRAMES;
        let mut peak = 0u16;
        let mut k = 0usize;
        while k < n {
            let v = adc_sync::cur_word((last + k) % adc_sync::CUR_FRAMES);
            if v > peak {
                peak = v;
            }
            k += 4;
        }
        LAST_CUR_HEAD.store(head as u32, Ordering::Relaxed);
        if peak > trig_raw {
            trig_raw = peak;
        }
    }
    if trig_raw > WAX_TRIG_RAW && armed && !WAX_TRIGGERED.load(Ordering::Relaxed) {
        WAX_TRIG_ARMED.store(false, Ordering::Relaxed);
        minz::adc_sync::freeze_current();
        // Freeze the black box too — the 64 commutation/ZC events
        // leading into the spike show WHY the amp draw started (the
        // NOZ → reacq-gate-widen → premature-accept divergence).
        BB_FROZEN.store(true, Ordering::Relaxed);
        WAX_TRIGGERED.store(true, Ordering::Relaxed);
    }
    WINDOW_I_SUM.fetch_add(i_raw as u32, Ordering::Relaxed);
    WINDOW_I_N.fetch_add(1, Ordering::Relaxed);
    if i_raw < WINDOW_I_MIN.load(Ordering::Relaxed) {
        WINDOW_I_MIN.store(i_raw, Ordering::Relaxed);
    }
    if i_raw > WINDOW_I_MAX.load(Ordering::Relaxed) {
        WINDOW_I_MAX.store(i_raw, Ordering::Relaxed);
    }
    // FALCON v3: confirm or discard the pending ZC candidate using
    // the mid-ON ADC sign of the floating phase vs the driven-pair
    // neutral (2× the float value vs 2× the neutral avoids halving).
    // `true` = phase below neutral = COMP VALUE=1 convention, so
    // CAND_EXPECTED applies unchanged. Sectors 0/3 float phase C
    // (no ADC route) — comp-bit fallback, observation only.
    if CAND_ZC_US.load(Ordering::Relaxed) != u32::MAX {
        // Per-sector float-vs-neutral arithmetic host-tested in
        // minz_core::zc (incl. the sector-2 ZC-boundary regression);
        // the confirm state machine (depth via timing::confirm_need —
        // 2 until CL_ACTIVE, 1 after, 2 in re-acq; the 1-confirm
        // engage-runaway and confirmation-latency history live in the
        // core docs) and the consume + gen-snapshot guard likewise.
        // The `free` envelope excludes the commutation ISR for the
        // ~10 µs of estimator + re-schedule; accept_gen_current
        // inside it validates the LOAD-TIME generation snapshot —
        // the B1 TOCTOU fix (a close + fresh re-arm between our load
        // and the CS used to defeat the gen-only re-check).
        let cur_sec = CURRENT_SECTOR.load(Ordering::Relaxed) & 7;
        let observed = minz_core::zc::adc_sign_observed(
            cur_sec,
            pa_a,
            pa_b,
            VBUS_EST.load(Ordering::Relaxed),
            comp2::value(),
        );
        match minz_core::zc::confirm_step(&ZC_STATE, observed) {
            minz_core::zc::ConfirmAction::Idle | minz_core::zc::ConfirmAction::Progress => {}
            minz_core::zc::ConfirmAction::AcceptPending { zc_us, gen_snap } => {
                free(|_| {
                    if minz_core::zc::accept_gen_current(&ZC_STATE, gen_snap) {
                        accept_qualified_zc(zc_us);
                    }
                });
            }
            minz_core::zc::ConfirmAction::Discarded {
                confirms,
                cl_active,
            } => {
                if cl_active {
                    bb_record(minz_core::blackbox::EV_DIS, cur_sec, confirms as u16);
                }
            }
        }
    }

    // Overcurrent failsafe: 85 ms average vs the 1.5 A trip level.
    // This ISR is the accumulators' sole writer — plain load/store.
    // Kill directly from here (don't wait for main): all six gate
    // inputs to OUTPUT-LOW, TIM7 CCR updates off, COMP EXTI masked —
    // identical to the `w` key path.
    // Windowed average (minz_core::guards::trip_accum_step) +
    // decision (::overcurrent — phase-vs-battery semantics and the
    // AM32-rides-the-knee history are regression-tested there) +
    // flag matrix (::apply_isr_kill — the OC zombie-status incident).
    let (acc, cnt, avg) = minz_core::guards::trip_accum_step(
        I_TRIP_ACC.load(Ordering::Relaxed),
        I_TRIP_CNT.load(Ordering::Relaxed),
        i_raw,
    );
    I_TRIP_ACC.store(acc, Ordering::Relaxed);
    I_TRIP_CNT.store(cnt, Ordering::Relaxed);
    if let Some(avg_raw) = avg {
        let tripped = minz_core::guards::overcurrent(avg_raw, CL_ACTIVE.load(Ordering::Relaxed));
        if tripped && MOTOR_ENABLED.load(Ordering::Relaxed) {
            minz_core::guards::apply_isr_kill(
                &KILL_FLAGS,
                minz_core::guards::IsrKillKind::Overcurrent,
            );
            tim1_motor_pwm::all_off();
            comp2::set_exti_enabled(false);
        }
    }
    let idx = (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
    let value = comp2::value() as u8;
    let sector = CURRENT_SECTOR.load(Ordering::Relaxed) & 0x07;
    // Per-cycle context (GECKO-scope hybrid): mid-ON A/B/current from
    // this cycle's injected burst + the free-run current-ring head, so
    // the `j` analog dump and the oversampled `J` autopsy can align
    // cycles onto the two streams. Same index as the status byte below.
    CTX_A[idx].store(pa_a, Ordering::Relaxed);
    CTX_B[idx].store(pa_b, Ordering::Relaxed);
    CTX_I[idx].store(i_raw, Ordering::Relaxed);
    CTX_MARK[idx].store(adc_sync::cur_head() as u16, Ordering::Relaxed);
    PWM_SAMPLE_BUF[idx].store(value | (sector << 1), Ordering::Relaxed);
    PWM_SAMPLE_IDX.store(
        (idx as u32).wrapping_add(1) & PWM_SAMPLE_MASK,
        Ordering::Relaxed,
    );
}

#[interrupt]
fn COMP() {
    let _dur = DurGuard::new(&DUR_COMP);
    // EXTI line 22 is COMP2's output (COMP1 is line 21, unused here).
    // Ack the pending bit first so the IRQ doesn't immediately re-fire.
    comp2::clear_pending();
    COMP_COUNT.fetch_add(1, Ordering::Relaxed);
    WINDOW_RAW.fetch_add(1, Ordering::Relaxed);

    // ONE time snapshot per entry (see `ticks_both`). Entry time is
    // the honest edge timestamp anyway — the EXTI fired microseconds
    // before any later in-body read would run.
    let (now_10, now_us) = ticks_both();

    // Software PWM-edge blanking. The `TIM1_CC` ISR latches
    // `ticks_1us()` into `LAST_PWM_EDGE_US` at every PWM channel
    // transition. If we're inside the `BLANK_US` window after any
    // such transition, this edge is almost certainly PWM-coupled
    // ringing rather than a real BEMF event — drop it from the
    // EDGE_BUF / SECTOR_EDGE_COUNT recording.
    //
    // 1 µs resolution comes from `ticks_1us()` = DWT.CYCCNT / 80.
    // PWM cycle at 24 kHz = 41.67 µs, so a sensible blank window is
    // 1-30 µs.
    //
    // This is the duty-independent replacement for the TIM15-OC1
    // hardware blanking we used to do (which was tied to a fixed
    // offset from CNT=0 and therefore stopped covering the actual
    // PWM falling edge as duty grew). We keep `COMP_COUNT` /
    // `VALID_COMP_COUNT` unfiltered so the raw EXTI rate stays
    // visible via the `i` key.
    // SPEED-ADAPTIVE blanking (cribbed from AM32's actual L431
    // strategy). AM32 has NO time-since-PWM-edge blank on L431 — its
    // noise defense is persistence depth scaled with speed
    // (`filter_level = map(average_interval, ...)`, main.c:2112).
    // Our fixed blank was strangling high speed: at 8 µs and 48 kHz
    // PWM (~2 CC edges per 20.8 µs cycle) we were blind ~75 % of
    // every window, and shrinking windows made the visible slivers
    // rarer still (the amp-28 SWIFT break). Fix: the effective blank
    // shrinks with the measured interval (interval/75 → 3 µs at
    // 740 Hz, 2 µs at 1.1 kHz) and the persistence check deepens to
    // AM32's 12 reads once the blank falls below 5 µs (AM32 uses 12
    // for our entire interval range). At low speed the arithmetic
    // yields exactly the proven 8 µs + 5-read combo — unchanged.
    let interval_us = OWL_INTERVAL_US.load(Ordering::Relaxed);
    // Host-tested: minz_core::timing::{blank_us, persistence_reads}.
    let blank_us =
        minz_core::timing::blank_us(BLANK_US.load(Ordering::Relaxed) as u32, interval_us);
    if blank_us > 0 {
        let since_edge = now_us.wrapping_sub(LAST_PWM_EDGE_US.load(Ordering::Relaxed));
        if since_edge < blank_us {
            let elapsed = now_us.wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
            if elapsed >= SECTOR_GATE_US.load(Ordering::Relaxed) {
                VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
    }
    // Persistence depth for the qZC filter below: AM32-style
    // deepening as the time-blank fades.
    let persist_reads: u32 = minz_core::timing::persistence_reads(blank_us);

    // Mode 5: value-gated recording. EXTI line 22 fires off the *raw*
    // comparator output (not gated by any internal blanking), so an
    // edge during PWM ringing still reaches us here. Reading
    // `COMP2.VALUE` tells us the comparator's current dwell state at
    // ISR entry — if the user wants only the rising-into-VALUE=1
    // events, mode 5 drops the rest.
    // COMP_COUNT and VALID_COMP_COUNT stay unfiltered.
    if EDGE_MODE.load(Ordering::Relaxed) == 5 && !comp2::value() {
        // Also still run the time-window gate so VALID_COMP_COUNT
        // tracks the same denominator across modes.
        let elapsed = now_us.wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
        if elapsed >= SECTOR_GATE_US.load(Ordering::Relaxed) {
            VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        return;
    }

    // Edge-buffer write: mark "an edge happened in this 10 µs bin"
    // at the current offset from the start of the current 2-rev
    // window. No direction label: L4 has no per-direction EXTI
    // pending bits and `comp2::value()` only tells us the dwell
    // state, not the edge direction (see `EDGE_BUF` docs).
    // At f_elec < 50 Hz 2 revs exceeds 4096 ticks and the masked
    // index wraps, overwriting earlier cells — see `HALF_TICKS` docs.
    let edge_idx =
        (now_10.wrapping_sub(HALF_START_TICK.load(Ordering::Relaxed)) & HALF_TICK_MASK) as usize;
    let edge_half = (ACTIVE_HALF.load(Ordering::Relaxed) & 1) as usize;
    // Saturating-increment per cell. COMP ISR is the only writer (TIM7
    // memset / E-dump snapshot run with COMP masked or in `free`), so a
    // plain load + store is race-safe. `u8::saturating_add(1)` caps at
    // 255 so we never wrap back to 0. The dump still maps non-zero →
    // `#`, zero → `.`, so the visual output is unchanged; the per-cell
    // count is available for a future "per-cell density" view.
    let cur = EDGE_BUF[edge_half][edge_idx].load(Ordering::Relaxed);
    EDGE_BUF[edge_half][edge_idx].store(cur.saturating_add(1), Ordering::Relaxed);

    // Per-sector total-edge counter (unlike EDGE_BUF, sees every edge
    // not just every 10 µs window with at least one edge). Slot
    // indexing matches SECTOR_BOUNDARIES: `rev_phase * 6 + sector`.
    // TIM7 ISR is the sole writer of REV_PHASE / CURRENT_SECTOR /
    // ACTIVE_HALF and does so inside a critical section, so the three
    // atomics read together here are consistent.
    let rev_phase = REV_PHASE.load(Ordering::Relaxed) as usize;
    let cur_sector = CURRENT_SECTOR.load(Ordering::Relaxed) as usize;
    let slot = rev_phase * 6 + cur_sector;
    if slot < 12 {
        SECTOR_EDGE_COUNT[edge_half][slot].fetch_add(1, Ordering::Relaxed);
    }

    // Time-window gate (raw-only intermediate counter). Only edges in
    // the second half of the float sector pass this. Useful as a
    // first-pass noise gate; the upstream raw `COMP_COUNT` and the
    // per-sector / per-cell EDGE_BUF + counters are the unfiltered
    // diagnostic surface.
    let elapsed = now_us.wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
    if elapsed < SECTOR_GATE_US.load(Ordering::Relaxed) {
        return;
    }
    VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
    // MAGPIE: per-window valid count + first-survivor timestamp.
    // This ISR is the sole writer between TIM7's `free`-wrapped
    // resets, so plain check-then-store is race-free.
    WINDOW_VALID.fetch_add(1, Ordering::Relaxed);
    if WINDOW_FIRST_ZC_US.load(Ordering::Relaxed) == u32::MAX {
        WINDOW_FIRST_ZC_US.store(now_us, Ordering::Relaxed);
    }

    // OWL: persistence qualification — AM32's layer-2 filter. A real
    // post-ZC comparator level DWELLS; PWM-coupled ringing bounces
    // back within a few hundred ns. Accept the edge as *the* ZC only
    // if VALUE holds the expected post-ZC level for 5 spaced reads
    // (~0.6 µs total — this ISR is priority 1 and stays short).
    // Expected level under textbook polarity (POLARITY=0, INP=neutral,
    // INM=floating phase): even sectors ride a falling-BEMF window →
    // post-ZC the phase is BELOW neutral → VALUE=1; odd sectors the
    // inverse. Matches `edges_for` mode 3 (rising even / falling odd).
    if WINDOW_QZC_US.load(Ordering::Relaxed) == u32::MAX {
        let expected = (CURRENT_SECTOR.load(Ordering::Relaxed) & 1) == 0;
        let mut held = true;
        for _ in 0..persist_reads {
            cortex_m::asm::delay(8);
            if comp2::value() != expected {
                held = false;
                break;
            }
        }
        if held {
            // Dispatch host-tested in minz_core::zc::on_held_edge:
            // SWIFT (fast path + active) accepts right here — edge-
            // timestamped, AM32-style, zero wrap latency. Safe from
            // this ISR: LPTIM2 (commutation) shares priority 1 with
            // COMP so they never preempt each other, and TIM1_UP
            // (prio 3) is preempted — no `free` needed; WINDOW_QZC
            // caps it at one accept per window. Otherwise a candidate
            // is armed for TIM1_UP to confirm. Timestamp = ISR entry
            // (`now_us` hoisted above): the physical edge fired just
            // before entry, so entry time is the more honest ZC stamp
            // than a fresh read taken after the ~1 µs persistence
            // loop; interval math is differential so a uniform shift
            // cancels, and the schedule path re-reads elapsed time
            // fresh for its latency compensation.
            if minz_core::zc::on_held_edge(&ZC_STATE, expected, now_us)
                == minz_core::zc::HeldEdge::AcceptNow
            {
                accept_qualified_zc(now_us);
            }
        }
    }
}

/// FALCON acceptance path — publish + estimator + engage decision
/// are host-tested in minz_core::zc::accept_publish (mask-after-
/// accept, C-window exclusion, engage transition, the estimator's
/// full incident suite); this site records the bb events and runs
/// the scheduling tail, keeping the elapsed-time read in its
/// original position (after the estimator work) so the confirmation-
/// latency compensation is unchanged.
fn accept_qualified_zc(zc_us: u32) {
    let sec = CURRENT_SECTOR.load(Ordering::Relaxed) & 7;
    let Some(plan) = minz_core::zc::accept_publish(&ZC_STATE, sec, zc_us, ticks_10us()) else {
        return; // window already has its ZC
    };
    if plan.engaged {
        bb_record(
            minz_core::blackbox::EV_ENG,
            sec,
            plan.interval_us.min(0xFFFF) as u16,
        );
    }
    if plan.schedule {
        // Host-tested: auto-advance ramp + scheduling delay
        // (minz_core::timing).
        let adv = minz_core::timing::auto_advance_deg(
            plan.interval_us,
            ADVANCE_DEG.load(Ordering::Relaxed) as i32,
        );
        let elapsed = ticks_1us().wrapping_sub(zc_us) as i32;
        let delay = minz_core::timing::commutation_delay_us(plan.interval_us, adv, elapsed);
        minz::lptim2_oneshot::schedule_us(delay);
        SHOT_REFINED.store(true, Ordering::Relaxed);
        bb_record(minz_core::blackbox::EV_ACC, sec, delay.min(0xFFFF) as u16);
        // Deaf until the commutation (mask-after-accept).
        comp2::set_exti_enabled(false);
    }
}
