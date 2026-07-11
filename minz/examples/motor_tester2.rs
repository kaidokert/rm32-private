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
use cortex_m_rt::{entry, exception};
use heapless::spsc::{Producer, Queue};
use minz::adc_sync;
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2;
use minz::current_adc::SenseAdc;
use minz::hal::pac::{USART1, interrupt};
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial, Tx};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::idle_loop::IdleLoop;
use minz::open_loop::{self, Waveform};
use minz::priority;
use minz::tim1_motor_pwm::{self, max_duty};
use minz::{PWM_FREQUENCY_HZ, SYSTICK, a85, tim7_drive};
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

/// Amplitude clamps as % of full ARR swing. **Hard-capped at 20 %** because
/// open-loop sine drive has no rotor sync and no current limit — at low
/// electrical frequency the applied voltage divides across the milliohm
/// winding resistance and turns straight into copper losses (already cost
/// us one motor on this bench). 20 % at 5 V bench supply ≈ 1 V × 1/0.05 Ω
/// ≈ 20 A peak per phase, which is already plenty. Drop this further if
/// the bench supply goes back above 5 V.
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
const AMP_MAX: u16 = 60;
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

/// PA6 (ADC1_IN11) → bench-supply voltage divider, scaled ×100.
/// Vimdrones L431 schematic: 30 kΩ from VBat to PA6, 3.6 kΩ from PA6
/// to GND. `V_bat = V_pa6 × (30 + 3.6) / 3.6 = V_pa6 × 9.333`.
///
/// (Bench cal under-read the schematic ratio by ~8 % at 5.39 V —
/// resistor tolerance; trust the schematic for now and add a fixed
/// trim if you actually need ±1 % accuracy.)
const VBAT_DIVIDER_X100: u32 = 933;

/// PA3 (ADC1_IN8) current-sense gain in mV per amp.
/// Vimdrones L431 uses an INA180B1 (gain **20 V/V**) on a **1.5 mΩ**
/// shunt: `V_pa3 = I × 1.5 mΩ × 20 = I × 30 mV/A`.
///
/// (Bench cal at 0.222 A read 5 mV ≈ 22.5 mV/A — 25 % lower than the
/// schematic predicts. PSU readout error / INA offset / shunt tol;
/// schematic is the honest answer until measured otherwise.)
const ISNS_MV_PER_AMP: u32 = 30;

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

/// SysTick-backed 64-bit timer used as the bench's monotonic clock.
///
/// - `tick_hz = 100_000` → output ticks are 10 µs each (matching the
///   `TICKS_10US_FAST` atomic scale below).
/// - `reload_value = 799` → SysTick fires every 800 cycles = every
///   10 µs at 80 MHz.
/// - `systick_freq = 80_000_000` → matches `SYSCLK`.
///
/// **Hot-path callers should NOT use `SYSTICK_TIMER.now()`** — each
/// call costs ~300 cycles because of the stable-snapshot retry loop
/// (multiple `SeqCst` atomic loads + SYST.CVR reads + PendST check +
/// u64 multiply with u128 overflow fallback). That's fine for slow
/// polling but is too heavy for ISRs that fire at 24 kHz+. The
/// `TICKS_10US_FAST` atomic below is the cheap hot-path counter
/// (~5 cycles per read), maintained in lockstep from the SysTick
/// handler. Use `ticks_10us()` to read it.
///
/// The crate's `now()` stays available for future code that needs
/// 64-bit width or sub-tick resolution (e.g. when we bump `tick_hz`
/// to `1_000_000` to get 1 µs resolution from `SYST.CVR` reads).
static SYSTICK_TIMER: systick_timer::Timer = systick_timer::Timer::new(100_000, 799, 80_000_000);

/// Cheap 10 µs tick counter, bumped from the SysTick handler in
/// lockstep with `SYSTICK_TIMER`'s internal state. Reading it is a
/// single relaxed atomic load (~5 cycles) — safe to call from any
/// ISR. Wraps every ~12 h at 10 µs ticks; all callers use
/// `wrapping_sub` for delta computations so the wrap is benign.
static TICKS_10US_FAST: AtomicU32 = AtomicU32::new(0);

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

/// Half-sector duration in µs at a given electrical frequency.
/// 6 sectors per electrical rev → full sector = `1e6 / (6 × f)` µs;
/// half is `1e6 / (12 × f)`. At f=60 → 1.39 ms. At f=600 → 139 µs.
const fn sector_gate_us(electrical_hz: u32) -> u32 {
    1_000_000 / (12 * electrical_hz)
}

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
/// 2.0 A × 30 mV/A = 60 mV → 60 / (3300/4095) ≈ 75 counts.
/// (Raised from 1.5 A for the amp-60 envelope: the prop's ω³ draw is
/// a legitimate ~1.2 A average at ~1.5 kHz elec. Stall protection is
/// not weakened — a stalled winding at these duties hits the PSU's
/// 2.5 A limit and the ZC-starvation guard within milliseconds,
/// long before an 85 ms average matters.)
const I_TRIP_RAW: u32 = 75;
const I_TRIP_SHIFT: u32 = 11; // 2048 cycles = 85 ms @ 24 kHz
static I_TRIP_ACC: AtomicU32 = AtomicU32::new(0);
static I_TRIP_CNT: AtomicU32 = AtomicU32::new(0);
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
/// Absolute backstop ≈ 5.95 V: below this the MCU's 3.3 V rail is
/// one transient from brownout — which WEDGES the chip with the
/// bridge frozen and every software guard dead (the 2026-07-10
/// burnt motor). Applies even if the baseline was low.
const VBAT_KILL_RAW: u16 = 793;

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
#[derive(Copy, Clone)]
struct WindowRec {
    start_10us: u32,
    len_10us: u16,
    /// First valid edge, µs from window start; `0xFFFF` = no valid edge.
    zc_off_us: u16,
    raw: u16,
    valid: u16,
    /// PWM-synchronous current over the window, raw 12-bit counts.
    i_min: u16,
    i_max: u16,
    i_avg: u16,
    /// OWL: first persistence-qualified ZC, µs from window start;
    /// `0xFFFF` = none.
    qzc_off_us: u16,
    /// OWL: (actual boundary − predicted boundary) µs, where predicted
    /// = qualified ZC + smoothed_interval/2. `i16::MIN` = no
    /// prediction this window (no qZC or no interval estimate).
    pred_err_us: i16,
    /// Live vbat raw at window close (frame v4) — puts the supply
    /// voltage INTO the high-rate stream so between-rung transits
    /// (sag events) are directly visible in captures at window rate.
    vbat_raw: u16,
    sector: u8,
    seq: u8,
}

const WREC_SYNC0: u8 = 0x5A;
/// v4 sync (26-byte v3 used 0xA5; the parser accepts both).
const WREC_SYNC1: u8 = 0xA6;
const WREC_FRAME_LEN: usize = 28;

impl WindowRec {
    /// Little-endian wire frame v4: sync(2) seq(1) sector|flags(1)
    /// start(4) len(2) zc_off(2) raw(2) valid(2) i_min(2) i_max(2)
    /// i_avg(2) qzc_off(2) pred_err(2,i16) vbat_raw(2). Bit 7 of
    /// byte 3 = "zc found".
    fn encode(&self) -> [u8; WREC_FRAME_LEN] {
        let mut f = [0u8; WREC_FRAME_LEN];
        f[0] = WREC_SYNC0;
        f[1] = WREC_SYNC1;
        f[2] = self.seq;
        f[3] = (self.sector & 0x0F) | if self.zc_off_us != 0xFFFF { 0x80 } else { 0 };
        f[4..8].copy_from_slice(&self.start_10us.to_le_bytes());
        f[8..10].copy_from_slice(&self.len_10us.to_le_bytes());
        f[10..12].copy_from_slice(&self.zc_off_us.to_le_bytes());
        f[12..14].copy_from_slice(&self.raw.to_le_bytes());
        f[14..16].copy_from_slice(&self.valid.to_le_bytes());
        f[16..18].copy_from_slice(&self.i_min.to_le_bytes());
        f[18..20].copy_from_slice(&self.i_max.to_le_bytes());
        f[20..22].copy_from_slice(&self.i_avg.to_le_bytes());
        f[22..24].copy_from_slice(&self.qzc_off_us.to_le_bytes());
        f[24..26].copy_from_slice(&self.pred_err_us.to_le_bytes());
        f[26..28].copy_from_slice(&self.vbat_raw.to_le_bytes());
        f
    }
}

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
const MOTOR_DRIVE_HZ: u32 = 6_000;

/// One full electrical revolution in 16.16 fixed point.
const ANGLE_FULL_REV_FP: u32 = 360u32 << 16;

/// Compute the per-tick `ANGLE_INC` for a given electrical f. Done
/// in u64 to avoid overflow at high f. Returns 16.16 fixed point.
const fn angle_inc_fp(electrical_hz: u32) -> u32 {
    ((360u64 << 16) * electrical_hz as u64 / MOTOR_DRIVE_HZ as u64) as u32
}

/// Bitfield of float-window sectors for a given observed phase.
/// Textbook 6-step BLDC convention.
const fn float_sector_mask(phase: comp2::ObservedPhase) -> u8 {
    match phase {
        comp2::ObservedPhase::A => (1 << 2) | (1 << 5),
        comp2::ObservedPhase::B => (1 << 1) | (1 << 4),
        comp2::ObservedPhase::C => (1 << 0) | (1 << 3),
    }
}

/// Which COMP2 EXTI edges to enable for the given sector + edge-mode.
/// COMP2 polarity is locked to non-inverted (POLARITY=0); modes 3/4
/// (phys_ZC / phys_anti) split on sector parity so EXTI fires on the
/// expected physical V+/V- crossing direction for each sector.
///
///   0 (both): rising + falling — every COMP_VALUE transition
///   1 (raw_rise): rising only
///   2 (raw_fall): falling only
///   3 (phys_ZC): rising in even sectors (falling-BEMF ZC),
///                falling in odd sectors (rising-BEMF ZC)
///   4 (phys_anti): the opposite of mode 3 (diagnostic)
#[inline]
fn edges_for(mode: u8, sector: u8) -> (bool, bool) {
    match mode {
        0 => (true, true),
        1 => (true, false),
        2 => (false, true),
        3 => {
            if (sector & 1) == 0 {
                (true, false)
            } else {
                (false, true)
            }
        }
        4 => {
            if (sector & 1) == 0 {
                (false, true)
            } else {
                (true, false)
            }
        }
        // Mode 5: both edges enabled at EXTI; the COMP ISR filters in
        // software based on `COMP2.VALUE`.
        _ => (true, true),
    }
}

/// SPSC producer half of the RX byte queue, owned by the USART2 ISR.
/// Main owns the consumer half and drains it in the key-dispatch pass.
static RX_PROD: Mutex<RefCell<Option<Producer<'static, u8>>>> = Mutex::new(RefCell::new(None));

/// DMA-backed UART TX ring. `write!` enqueues into a 4 KiB static ring
/// (drop-on-overflow, same semantics as the old deque writer); the
/// motor loop calls [`service`] once per microloop, which hands the
/// longest contiguous unsent run to **DMA1_CH4** (USART1_TX request,
/// CSELR C4S = 0b0010) and lets hardware drain it at wire rate. No DMA
/// interrupt — transfer-complete is polled from `service`, so the whole
/// TX path stays main-context-only.
///
/// Why: the old writer shifted ONE byte per 1 ms service pass
/// (~1 kB/s effective) — fine for key echoes at 9600, useless for
/// streaming at 2 Mbaud (200 kB/s). With 4 KiB chunks kicked per pass
/// the wire stays >95 % utilised while main spends ~0 CPU on TX.
///
/// Ownership note: `Tx<USART1>` is held only to keep the HAL from
/// handing the peripheral to anyone else — after `CR3.DMAT` is set,
/// data moves ring → TDR entirely by DMA. Never write TDR from the
/// CPU while a chunk is in flight (interleaved garbage); all output
/// must go through this ring, including "blocking" dumps.
struct UartTxWriter {
    _tx: Tx<USART1>,
    ring: &'static mut [u8; TX_RING_LEN],
    /// Next byte to write (main only). Ring is full when advancing
    /// head would collide with tail (one slot wasted, classic ring).
    head: usize,
    /// Oldest unsent byte. Advances only on DMA transfer-complete.
    tail: usize,
    /// Bytes handed to the in-flight DMA chunk (0 = DMA idle).
    inflight: usize,
}

const TX_RING_LEN: usize = 4096; // power of two

impl UartTxWriter {
    fn new(tx: Tx<USART1>, ring: &'static mut [u8; TX_RING_LEN]) -> Self {
        // One-time plumbing: DMA1 clock, route channel 4 to USART1_TX,
        // point CPAR at TDR, and let USART1 raise DMA requests.
        unsafe {
            (*stm32::RCC::ptr())
                .ahb1enr
                .modify(|_, w| w.dma1en().set_bit());
            let dma = &*stm32::DMA1::ptr();
            dma.cselr.modify(|_, w| w.c4s().bits(0b0010));
            dma.cpar4
                .write(|w| w.bits(&(*stm32::USART1::ptr()).tdr as *const _ as u32));
            (*stm32::USART1::ptr())
                .cr3
                .modify(|_, w| w.dmat().set_bit());
        }
        Self {
            _tx: tx,
            ring,
            head: 0,
            tail: 0,
            inflight: 0,
        }
    }

    fn pending(&self) -> usize {
        self.head.wrapping_sub(self.tail) & (TX_RING_LEN - 1)
    }

    /// Push one byte; `false` (byte dropped) if the ring is full.
    fn push(&mut self, b: u8) -> bool {
        let next = (self.head + 1) & (TX_RING_LEN - 1);
        if next == self.tail {
            return false;
        }
        self.ring[self.head] = b;
        self.head = next;
        true
    }

    /// Reap a completed DMA chunk (if any) and kick the next one.
    /// Called once per microloop; also spun directly by the blocking
    /// paths. Worst-case gap between chunk-complete and next kick is
    /// one microloop (1 ms) — with 4 KiB chunks (20 ms of wire time
    /// at 2 M) that keeps the line >95 % utilised.
    fn service(&mut self) {
        let dma = unsafe { &*stm32::DMA1::ptr() };
        if self.inflight != 0 {
            if dma.isr.read().tcif4().bit_is_set() {
                dma.ccr4.modify(|_, w| w.en().clear_bit());
                dma.ifcr.write(|w| w.cgif4().set_bit());
                self.tail = (self.tail + self.inflight) & (TX_RING_LEN - 1);
                self.inflight = 0;
            } else {
                return; // chunk still on the wire
            }
        }
        let pending = self.pending();
        if pending == 0 {
            return;
        }
        // Longest contiguous run from tail (a wrap becomes two chunks).
        let contig = pending.min(TX_RING_LEN - self.tail);
        unsafe {
            dma.cmar4
                .write(|w| w.bits(self.ring.as_ptr().add(self.tail) as u32));
            dma.cndtr4.write(|w| w.bits(contig as u32));
            // Ring bytes must be visible to DMA before EN.
            core::sync::atomic::compiler_fence(Ordering::Release);
            dma.ccr4
                .write(|w| w.minc().set_bit().dir().set_bit().en().set_bit());
        }
        self.inflight = contig;
    }

    /// Enqueue a byte slice without dropping: spin `service` whenever
    /// the ring is full. Ordering vs earlier `write!` output is free
    /// (single ring). Returns once everything is *enqueued* — the tail
    /// of the data may still be draining by DMA afterwards, which is
    /// fine because all output goes through the same ring.
    fn write_blocking(&mut self, bytes: &[u8]) {
        for &b in bytes {
            while !self.push(b) {
                self.service();
            }
        }
        self.service();
    }
}

impl core::fmt::Write for UartTxWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // Drop bytes on overflow — status messages aren't critical.
            let _ = self.push(b);
        }
        Ok(())
    }
}

fn ticks_10us() -> u32 {
    // Cheap atomic read — ~5 cycles, no stable-snapshot loop. The
    // `TICKS_10US_FAST` atomic is bumped from the SysTick handler in
    // lockstep with the systick-timer crate's 64-bit counter, so all
    // existing 10 µs-scale call sites work unchanged. u32 wraps every
    // ~12 h; downstream code uses `wrapping_sub` for deltas.
    TICKS_10US_FAST.load(Ordering::Relaxed)
}

/// Microsecond-resolution wall clock for the software-blanking math.
/// Used **only** by the TIM1_CC ISR (latch `LAST_PWM_EDGE_US`) and
/// the COMP ISR (compute `now - LAST_PWM_EDGE_US`). Everything else
/// stays on the cheaper `ticks_10us()`.
///
/// Implementation: combine the 10 µs base from `TICKS_10US_FAST` with
/// the sub-tick remainder from `SYST.CVR`. `SYST.CVR` counts DOWN
/// from `RELOAD = 799` to 0 each 10 µs (at 80 MHz core clock), so
/// `(799 - CVR)` is "cycles elapsed in current 10 µs tick" and
/// `/ 80` converts to µs.
///
/// Race window: SysTick fires every 800 cycles at the highest NVIC
/// priority. If it preempts us between the two reads, the wrap
/// counter and `CVR` go out of sync. We sandwich the atomic read
/// between two `CVR` reads: if `cvr_post > cvr_pre`, a wrap
/// happened (CVR jumped back up to 799), so we retry. In the
/// common case the retry never fires, so cost is ~15 cycles total.
///
/// Wraps every ~71 min (u32 / 1 µs); deltas via `wrapping_sub` so
/// the wrap is benign for the blanking window.
#[inline]
fn ticks_1us() -> u32 {
    loop {
        let cvr_pre = cortex_m::peripheral::SYST::get_current();
        let coarse = TICKS_10US_FAST.load(Ordering::Relaxed);
        let cvr_post = cortex_m::peripheral::SYST::get_current();
        if cvr_post <= cvr_pre {
            return coarse
                .wrapping_mul(10)
                .wrapping_add((799u32.wrapping_sub(cvr_post)) / 80);
        }
        // SysTick wrapped between the reads; retry for a consistent
        // snapshot. The next iteration sees the post-wrap state.
    }
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

    // SysTick now runs through the systick-timer crate so we can grow
    // it to 1 µs resolution later without touching the ISR. The crate
    // owns the SYST peripheral: set clock source, reload, clear CVR,
    // enable interrupt + counter — same wiring `configure_systick`
    // did, but with the crate's 64-bit cycle accounting bolted on.
    let mut syst = cp.SYST;
    SYSTICK_TIMER.start(&mut syst);

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

    // Flip PB6 to push-pull after the fact — the HAL's half-duplex pin
    // trait insists on open-drain, but this line is TX-only (nothing
    // else ever drives it), and the open-drain rise through the 40 kΩ
    // internal pull-up is ~2-3 µs, which caps the usable baud at about
    // 115200. Actively driving both levels is what makes ≥921600 work.
    unsafe {
        (*stm32::GPIOB::ptr())
            .otyper
            .modify(|r, w| w.bits(r.bits() & !(1 << 6)));
    }

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
    // longest legitimate main-loop stall (waxwing dump ~100 ms) has
    // 10× margin.
    unsafe {
        let iwdg = &*stm32::IWDG::ptr();
        iwdg.kr.write(|w| w.key().bits(0x5555));
        iwdg.pr.write(|w| w.pr().bits(0b011));
        iwdg.rlr.write(|w| w.rl().bits(1000));
        iwdg.kr.write(|w| w.key().bits(0xCCCC));
    }

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
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.usart2en().set_bit());
    }
    let usart2 = dp.USART2;
    usart2.cr2.write(|w| w.swap().set_bit());
    usart2
        .brr
        .write(|w| unsafe { w.bits((clocks.pclk1().raw() + BAUD / 2) / BAUD) });
    usart2
        .cr1
        .write(|w| w.re().set_bit().rxneie().set_bit().ue().set_bit());

    let (producer, mut consumer) = RX_QUEUE.split();
    let (wrec_producer, mut wrec_consumer) = WREC_QUEUE.split();

    free(|cs| {
        RX_PROD.borrow(cs).replace(Some(producer));
        WREC_PROD.borrow(cs).replace(Some(wrec_producer));
    });

    // Enable DWT cycle counter — used by the rolling 1-second
    // window below for sub-second-precise rate calc.
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();

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
    SECTOR_GATE_US.store(sector_gate_us(FREQ_START), Ordering::Relaxed);
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
    writeln!(
        &mut tx,
        "Frequency (Hz):       d   +1, c -1, f +10, v -10\r"
    )
    .ok();
    writeln!(&mut tx, "Amplitude (% of ARR): a +1, z -1, s +10, x -10\r").ok();
    writeln!(&mut tx, "Mode toggle:          m   (sine <-> six-step)\r").ok();
    writeln!(
        &mut tx,
        "Panic reset:          r   (six-step, f=start, amp=start)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Slow reset:           q   (six-step, f=50, amp=start)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Kill output:          w   (clear MOE, all FETs off)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Print sense:          i   (PA3 IN8 isns, PA6 IN11 vbat)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Print BEMF comp:      b   (rate/s + level on observed phase)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Cycle observed phase: p   (A=PA4, B=PA5, C=PB7; manual mode only)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Toggle comp mux:      o   (auto per-sector [default] <-> manual single phase)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Cycle COMP hyst:      h   (0/none -> 1/low -> 3/high -> 0)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Cycle EXTI edges:     k   (both / raw_rise / raw_fall / phys_ZC / phys_anti / val_gated)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Commutation advance:  t +2deg / T -2deg (clamped 0..28)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "ZC path toggle:       M   (adc-confirm <-> SWIFT am32-edge)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Adjust SW blanking:   n/N (+/- 1 µs)   ./, (+/- 10 µs coarse), clamped 0..50 µs\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Dump PWM samples:     l   (per-60deg chunks: . = 0, # = 1)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Dump edge buffer:     e   (2 revs, 10us bins; f >= 50 Hz)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "UART blast test:      u   (64 KiB counting pattern, blocking)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Window record stream: g   (binary 22B frames, one per float window)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Waveform dump:        j   (85 ms burst: A/B volts + current + comp/sector, a85)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Closed loop:          y   (engage at next qualified ZC; y again = kill; desync auto-kills)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Failsafe: auto-kill at >1.5 A avg over 85 ms (stall guard, r/q re-arms)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Freeze/dump edge buf: E   (1st press: freeze + dump; 2nd press: resume)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Boot: OUTPUT OFF, f = 0 Hz, amp = {} % (cap {}). Press r/q to arm.\r",
        AMP_START, AMP_MAX,
    )
    .ok();

    let mut tx_writer = UartTxWriter::new(tx, TX_RING);

    // ----------------------------------------------------------------
    // Flip PRIMASK now so SysTick can actually deliver its exception
    // and increment `TICKS_10US`. `configure_systick` set TICKINT+ENABLE
    // in SYST_CSR way up top, but `panic::ensure_rtt` (run by `init`)
    // calls `cortex_m::interrupt::disable()` and never re-enables — so
    // PRIMASK=1 has been masking SysTick the whole time. NVIC stays
    // masked for app IRQs so the calibration baseline below sees only
    // SysTick eating cycles (~1 % at 100 kHz / ~10 µs ISR cost).
    // ----------------------------------------------------------------
    unsafe { cortex_m::interrupt::enable() };

    let mut idle_loop = IdleLoop::new();
    let now_u64 = || ticks_10us() as u64;
    let cal = idle_loop.calibrate(SECOND_TICKS, &now_u64);
    rprintln!(
        "idle_loop: calibration = {} counts / s (SysTick-only baseline)",
        cal,
    );

    // Program NVIC + SCB priorities BEFORE unmasking the app IRQs so
    // every IRQ comes up at its intended level. PRIGROUP=3 (4 preempt
    // / 0 sub bits). Levels: SysTick=0, COMP=1, TIM7=2, TIM1=3,
    // USART2 RX=4. USART2 isn't in the canonical `set_irq_prios` list
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

    // Per-second snapshot state for the COMP / VALID rates.
    // Sampled at every 1 s boundary below.
    let mut rate_start_count: u32 = COMP_COUNT.load(Ordering::Relaxed);
    let mut rate_start_valid: u32 = VALID_COMP_COUNT.load(Ordering::Relaxed);

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
    loop {
        let next_epoch = epoch_start + SECOND_TICKS;
        let mut next_microloop = epoch_start + MICROLOOP_TICKS;

        while now_u64() < next_epoch {
            // IWDG refresh — once per microloop (~10 ms cadence,
            // 100× inside the 1 s window). If the core wedges, this
            // stops and the watchdog resets the chip, releasing the
            // bridge.
            unsafe { (*stm32::IWDG::ptr()).kr.write(|w| w.key().bits(0xAAAA)) };

            // Slack: spin idle counter until next microloop boundary.
            // ISRs preempt this naturally and steal counter increments;
            // that's exactly how "busy" gets measured.
            idle_loop.run_until(next_microloop, &now_u64);

            // Active phase: drain RX queue + dispatch keys.
            while let Some(b) = consumer.dequeue() {
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
                    b'm' if CL_ACTIVE.load(Ordering::Relaxed) => {
                        write!(&mut tx_writer, "CL active - 'y' first\r\n").ok();
                    }
                    b'm' => {
                        waveform = match waveform {
                            Waveform::Sine => Waveform::SixStep,
                            Waveform::SixStep => Waveform::Sine,
                        };
                    }
                    b'r' | b'q' if CL_ACTIVE.load(Ordering::Relaxed) => {
                        write!(&mut tx_writer, "CL active - 'y' first\r\n").ok();
                    }
                    b'r' => {
                        // Panic reset: back to a known-good idle config.
                        // The existing prev_amp / prev_hz / prev_mode delta
                        // checks below pick this up and print the changes.
                        // Arm-time vbat baseline for the −10 % sag kill
                        // (captured unloaded, before the drive engages).
                        VBAT_BASELINE_RAW
                            .store(VBAT_RAW_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
                        waveform = Waveform::SixStep;
                        amplitude_pct = AMP_START;
                        electrical_hz = FREQ_START;
                        // SNAP applied = target on arm: the slew is
                        // for changes while RUNNING. An abort at amp
                        // 50 left the applied duty slewing down for
                        // 2 s while the next engage's spin-up ran at
                        // ~45 % — junk estimator seeds, 4/4 failed
                        // engages.
                        AMPLITUDE_PCT.store(AMP_START as u8, Ordering::Relaxed);
                        AMP_TARGET_PCT.store(AMP_START as u8, Ordering::Relaxed);
                        if !output_enabled {
                            output_enabled = true;
                            MOTOR_ENABLED.store(true, Ordering::Relaxed);
                            tim1_motor_pwm::arm_output();
                            // Re-arm BEMF EXTI now that the FETs are
                            // driving — see boot-time comment for why.
                            comp2::set_exti_enabled(true);
                            write!(&mut tx_writer, "armed\r\n").ok();
                        }
                    }
                    b'q' => {
                        // Slow-bench reset: same as `r` but lands at
                        // 50 Hz instead of `FREQ_START` (60 Hz). 50 Hz
                        // is around the floor of where the rotor still
                        // tracks cleanly without cogging — a useful
                        // "watch the waveforms" speed.
                        VBAT_BASELINE_RAW
                            .store(VBAT_RAW_LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
                        waveform = Waveform::SixStep;
                        amplitude_pct = AMP_START;
                        electrical_hz = 50;
                        AMPLITUDE_PCT.store(AMP_START as u8, Ordering::Relaxed);
                        AMP_TARGET_PCT.store(AMP_START as u8, Ordering::Relaxed);
                        if !output_enabled {
                            output_enabled = true;
                            MOTOR_ENABLED.store(true, Ordering::Relaxed);
                            tim1_motor_pwm::arm_output();
                            comp2::set_exti_enabled(true);
                            write!(&mut tx_writer, "armed\r\n").ok();
                        }
                    }
                    b'w' => {
                        // Hard kill: clear MOE → all FETs off immediately.
                        // CCRs / CCER are preserved so `r` resumes from
                        // the same waveform state. Also disable the
                        // TIM7 ISR's CCR updates so a stale arm doesn't
                        // resume the previous duty mid-decision, and
                        // mask EXTI22 so the now-floating phases don't
                        // storm the COMP ISR (see boot-time comment).
                        CL_ACTIVE.store(false, Ordering::Relaxed);
                        CL_ARMED.store(false, Ordering::Relaxed);
                        if output_enabled {
                            output_enabled = false;
                            MOTOR_ENABLED.store(false, Ordering::Relaxed);
                            tim1_motor_pwm::all_off();
                            comp2::set_exti_enabled(false);
                            write!(&mut tx_writer, "off\r\n").ok();
                        }
                    }
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
                        rate_start_count = COMP_COUNT.load(Ordering::Relaxed);
                        rate_start_valid = VALID_COMP_COUNT.load(Ordering::Relaxed);
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
                        rate_start_count = COMP_COUNT.load(Ordering::Relaxed);
                        rate_start_valid = VALID_COMP_COUNT.load(Ordering::Relaxed);
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
                        let (float_s0, float_s1) = match observed_phase {
                            comp2::ObservedPhase::A => (2u8, 5u8),
                            comp2::ObservedPhase::B => (1, 4),
                            comp2::ObservedPhase::C => (0, 3),
                        };
                        let mut snap = [0u8; PWM_SAMPLE_LEN];
                        NVIC::mask(Interrupt::TIM1_UP_TIM16);
                        let head =
                            (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
                        for i in 0..PWM_SAMPLE_LEN {
                            snap[i] = PWM_SAMPLE_BUF[i].load(Ordering::Relaxed);
                        }
                        unsafe { NVIC::unmask(Interrupt::TIM1_UP_TIM16) };
                        // 2-revs-aligned dump. Walk backwards from
                        // `head` looking for three 5→0 sector transitions.
                        // Three transitions enclose exactly two complete
                        // electrical revs; the oldest is the dump start,
                        // the newest is one past the end. Always exactly
                        // 12 chunks (6 sectors × 2 revs), each entered at
                        // its sector start. Float sectors of the observed
                        // phase get a `*`/`o` textbook-ZC marker at the
                        // chunk midpoint.
                        let mut line = [0u8; 200];
                        let mut rev_starts: [usize; 3] = [0; 3];
                        let mut found = 0usize;
                        let mut newer_sec: u8 =
                            (snap[(head + PWM_SAMPLE_LEN - 1) & (PWM_SAMPLE_LEN - 1)] >> 1) & 0x07;
                        for i in 2..=PWM_SAMPLE_LEN {
                            let cur_idx = (head + PWM_SAMPLE_LEN - i) & (PWM_SAMPLE_LEN - 1);
                            let cur_sec = (snap[cur_idx] >> 1) & 0x07;
                            if cur_sec == 5 && newer_sec == 0 {
                                rev_starts[found] = (cur_idx + 1) & (PWM_SAMPLE_LEN - 1);
                                found += 1;
                                if found == 3 {
                                    break;
                                }
                            }
                            newer_sec = cur_sec;
                        }
                        if found >= 3 {
                            // rev_starts[0] = newest 5→0 transition,
                            // rev_starts[2] = oldest. Span [rev_starts[2],
                            // rev_starts[0]) is exactly 2 complete revs.
                            let r_start = rev_starts[2];
                            let r_end = rev_starts[0];
                            let length = (r_end + PWM_SAMPLE_LEN - r_start) & (PWM_SAMPLE_LEN - 1);
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
                            let mut rev_label = 0u8;
                            let mut chunks_seen = 0u8;
                            let mut chunk_sec2: u8 = 0xFF;
                            let mut line_len = 0usize;
                            for i in 0..length {
                                let pos = (r_start + i) & (PWM_SAMPLE_LEN - 1);
                                let byte = snap[pos];
                                let sec = (byte >> 1) & 0x07;
                                let val = byte & 1;
                                if sec != chunk_sec2 {
                                    if line_len > 0 {
                                        if (chunk_sec2 == float_s0 || chunk_sec2 == float_s1)
                                            && line_len >= 2
                                        {
                                            let mid = line_len / 2;
                                            line[mid] = if line[mid] == b'#' { b'*' } else { b'o' };
                                        }
                                        line[line_len] = b'\r';
                                        line[line_len + 1] = b'\n';
                                        tx_writer.write_blocking(&line[..line_len + 2]);
                                        line_len = 0;
                                    }
                                    // After the first chunk, a new
                                    // sector-0 entry means we're in
                                    // rev 1.
                                    if sec == 0 && chunks_seen > 0 {
                                        rev_label = 1;
                                    }
                                    write!(&mut tx_writer, "[rev {} sec {}]: ", rev_label, sec,)
                                        .ok();
                                    tx_writer.write_blocking(&[]);
                                    chunk_sec2 = sec;
                                    chunks_seen += 1;
                                }
                                if line_len >= line.len() {
                                    tx_writer.write_blocking(&line);
                                    line_len = 0;
                                }
                                line[line_len] = if val != 0 { b'#' } else { b'.' };
                                line_len += 1;
                            }
                            if line_len > 0 {
                                if (chunk_sec2 == float_s0 || chunk_sec2 == float_s1)
                                    && line_len >= 2
                                {
                                    let mid = line_len / 2;
                                    line[mid] = if line[mid] == b'#' { b'*' } else { b'o' };
                                }
                                line[line_len] = b'\r';
                                line[line_len + 1] = b'\n';
                                tx_writer.write_blocking(&line[..line_len + 2]);
                            }
                        } else {
                            write!(
                                &mut tx_writer,
                                "pwm_samples last 2 revs: only {} rev starts in buffer (need 3)\r\n",
                                found,
                            )
                            .ok();
                        }
                    }
                    b'y' => {
                        // FALCON engage/kill. Engaging waits for the
                        // next qualified ZC that has an interval
                        // estimate behind it (OWL runs from arm, so
                        // that's typically the very next window).
                        // Pressing again while active = kill + coast
                        // (no jolty open-loop resume in v1).
                        if CL_ACTIVE.load(Ordering::Relaxed) {
                            CL_ACTIVE.store(false, Ordering::Relaxed);
                            CL_ARMED.store(false, Ordering::Relaxed);
                            output_enabled = false;
                            MOTOR_ENABLED.store(false, Ordering::Relaxed);
                            tim1_motor_pwm::all_off();
                            comp2::set_exti_enabled(false);
                            write!(
                                &mut tx_writer,
                                "CL off - output killed (r/q re-arms open loop)\r\n"
                            )
                            .ok();
                        } else if output_enabled && matches!(waveform, Waveform::SixStep) {
                            // Reset the estimator before arming: a
                            // stale interval (e.g. 144 µs left by a
                            // runaway) is otherwise UNRECOVERABLE —
                            // the runaway floor kills every engage
                            // instantly while the symmetric bound
                            // rejects every honest open-loop sample
                            // (1667 µs ≫ 1.8×144 µs). Engagement
                            // waits for interval != 0, so this
                            // re-seeds fresh from open-loop qZCs
                            // within a few windows.
                            OWL_INTERVAL_US.store(0, Ordering::Relaxed);
                            OWL_LAST_QZC_US.store(u32::MAX, Ordering::Relaxed);
                            WINDOWS_SINCE_QZC.store(0, Ordering::Relaxed);
                            CL_REACQ.store(false, Ordering::Relaxed);
                            CL_NOZ_RUN.store(0, Ordering::Relaxed);
                            CL_ARMED.store(true, Ordering::Relaxed);
                            write!(
                                &mut tx_writer,
                                "CL ARMED - engaging at next qualified ZC\r\n"
                            )
                            .ok();
                        } else {
                            write!(
                                &mut tx_writer,
                                "CL needs a running six-step drive first (r/q)\r\n"
                            )
                            .ok();
                        }
                    }
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
                        // WAXWING: freeze the DMA waveform ring and
                        // dump ~85 ms of per-PWM-cycle frames in the
                        // rinz cdump/Ascii85 format. Channels per
                        // frame: ch9=A (PA4), ch10=B (PA5), ch8=
                        // current, ch99=status byte (bit0 COMP value,
                        // bits1-3 sector) from PWM_SAMPLE_BUF — the
                        // two rings advance in lockstep at 24 kHz,
                        // aligned here by their write heads (≤2-frame
                        // skew). Motor keeps running; the current
                        // failsafe is blind for the ~110 ms dump.
                        let adc_idx = adc_sync::freeze_waveform();
                        let mut status = [0u8; PWM_SAMPLE_LEN];
                        NVIC::mask(Interrupt::TIM1_UP_TIM16);
                        let pwm_head =
                            (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
                        for (i, s) in status.iter_mut().enumerate() {
                            *s = PWM_SAMPLE_BUF[i].load(Ordering::Relaxed);
                        }
                        unsafe { NVIC::unmask(Interrupt::TIM1_UP_TIM16) };

                        write!(
                            &mut tx_writer,
                            "\r\ncdump: {} frames b85 4 channels (ch9 ch10 ch8 ch99) \
                             12-bit, sample_hz={} Hz\r\n",
                            adc_sync::WAX_FRAMES,
                            PWM_FREQUENCY_HZ,
                        )
                        .ok();
                        // 8 payload bytes per frame = 2 Ascii85 groups
                        // = 10 chars; 8 frames per output line.
                        let mut line = [0u8; 82];
                        let mut pos = 0usize;
                        for k in 0..adc_sync::WAX_FRAMES {
                            let f = ((adc_idx + k) % adc_sync::WAX_FRAMES) * adc_sync::WAX_CHANS;
                            let a = adc_sync::wax_word(f);
                            let b_ph = adc_sync::wax_word(f + 1);
                            let cur = adc_sync::wax_word(f + 2);
                            let st = status[(pwm_head + k) & (PWM_SAMPLE_LEN - 1)] as u16;
                            let w = [a, b_ph, cur, st];
                            let mut bytes = [0u8; 8];
                            for (n, v) in w.iter().enumerate() {
                                bytes[2 * n..2 * n + 2].copy_from_slice(&v.to_le_bytes());
                            }
                            line[pos..pos + 5].copy_from_slice(&a85::encode_group(
                                bytes[0..4].try_into().unwrap(),
                            ));
                            line[pos + 5..pos + 10].copy_from_slice(&a85::encode_group(
                                bytes[4..8].try_into().unwrap(),
                            ));
                            pos += 10;
                            if pos >= 80 {
                                line[pos] = b'\r';
                                line[pos + 1] = b'\n';
                                tx_writer.write_blocking(&line[..pos + 2]);
                                pos = 0;
                            }
                        }
                        if pos > 0 {
                            line[pos] = b'\r';
                            line[pos + 1] = b'\n';
                            tx_writer.write_blocking(&line[..pos + 2]);
                        }
                        tx_writer.write_blocking(b"end\r\n");
                        adc_sync::resume_waveform();
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
                        let i_mv = sense_adc.adc_to_mv(i_raw);
                        let i_ma = i_mv as u32 * 1000 / ISNS_MV_PER_AMP;
                        // From the 6 kHz TIM7 pump — a blocking
                        // injected read here would race the pump's
                        // JADSTART/JEOS handling.
                        let v_raw = VBAT_RAW_LIVE.load(Ordering::Relaxed);
                        let v_mv = sense_adc.adc_to_mv(v_raw);
                        let v_supply_mv = v_mv as u32 * VBAT_DIVIDER_X100 / 100;
                        // Worst vbat since the previous readout —
                        // makes between-rung transits visible (twice
                        // a real sag event was invisible to per-rung
                        // snapshots). Reset on read.
                        let v_min_raw = VBAT_MIN_RAW.swap(u16::MAX, Ordering::Relaxed);
                        let v_min_mv = if v_min_raw == u16::MAX {
                            0
                        } else {
                            sense_adc.adc_to_mv(v_min_raw) as u32 * VBAT_DIVIDER_X100 / 100
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
                        // Per-ISR rates since last `i` press. 10 µs
                        // tick units → multiply by 100 000 / Δticks
                        // to get events/sec. Skip rate calc if Δticks
                        // is zero (back-to-back presses inside one
                        // microloop) to avoid divide-by-zero.
                        let now_tick = ticks_10us();
                        let now_comp = COMP_COUNT.load(Ordering::Relaxed);
                        let now_usart2 = USART2_COUNT.load(Ordering::Relaxed);
                        let now_tim7 = TIM7_COUNT.load(Ordering::Relaxed);
                        let now_tim1 = TIM1_UP_COUNT.load(Ordering::Relaxed);
                        let now_tim1_cc = TIM1_CC_COUNT.load(Ordering::Relaxed);
                        let dtick = now_tick.wrapping_sub(last_i_tick);
                        if dtick > 0 {
                            let rate =
                                |dc: u32| -> u32 { ((dc as u64) * 100_000 / dtick as u64) as u32 };
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
                        }
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
                        let cur = HYST_LEVEL.load(Ordering::Relaxed);
                        let next = match cur {
                            0 => 1,
                            1 => 3,
                            _ => 0,
                        };
                        free(|_| {
                            HYST_LEVEL.store(next, Ordering::Relaxed);
                            comp2::set_hysteresis(next);
                        });
                        let name = match next {
                            0 => "0/none",
                            1 => "1/low",
                            _ => "3/high",
                        };
                        write!(&mut tx_writer, "hyst = {}\r\n", name).ok();
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
                        let cur = BLANK_US.load(Ordering::Relaxed) as i32;
                        let delta: i32 = match b {
                            b'n' => 1,
                            b'N' => -1,
                            b'.' => 10,
                            _ => -10,
                        };
                        let next = (cur + delta).clamp(0, 50) as u16;
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
                        let cur = ADVANCE_DEG.load(Ordering::Relaxed);
                        let next = (cur + if b == b't' { 2 } else { -2 }).clamp(0, 28);
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
                        let cur = EDGE_MODE.load(Ordering::Relaxed);
                        let next = if cur >= 5 { 0 } else { cur + 1 };
                        free(|_| {
                            EDGE_MODE.store(next, Ordering::Relaxed);
                            let sector = CURRENT_SECTOR.load(Ordering::Relaxed);
                            let (re, fe) = edges_for(next, sector);
                            comp2::set_exti_edges(re, fe);
                        });
                        let name = match next {
                            0 => "both",
                            1 => "raw rise",
                            2 => "raw fall",
                            3 => "phys ZC (rise even / fall odd)",
                            4 => "phys anti-ZC (fall even / rise odd)",
                            _ => "value-gated (both edges, VALUE=1 only)",
                        };
                        write!(&mut tx_writer, "edges = {}\r\n", name).ok();
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

                    let edge_mode = EDGE_MODE.load(Ordering::Relaxed);
                    let edge_name = match edge_mode {
                        0 => "both",
                        1 => "raw_rise",
                        2 => "raw_fall",
                        3 => "phys_ZC",
                        4 => "phys_anti",
                        _ => "val_gated",
                    };

                    let valid = window_end_tick != 0 && sec_starts.iter().all(|&t| t != 0);
                    if !valid {
                        write!(
                            &mut tx_writer,
                            "edges last 2 revs: no complete rev-pair captured yet \
                             (let motor spin a few revs after arm/re-arm)\r\n",
                        )
                        .ok();
                    } else if electrical_hz == 0 {
                        write!(&mut tx_writer, "edges last 2 revs: motor off (f=0)\r\n",).ok();
                    } else {
                        let window_start_tick = sec_starts[0];
                        let window_ticks = window_end_tick.wrapping_sub(window_start_tick);
                        write!(
                            &mut tx_writer,
                            "edges last 2 revs aligned to sec 0 \
                             (window={}us, hyst={}, edges={}, advance={}deg, blank={}us{}):\r\n",
                            window_ticks.wrapping_mul(10),
                            HYST_LEVEL.load(Ordering::Relaxed),
                            edge_name,
                            ADVANCE_DEG.load(Ordering::Relaxed),
                            BLANK_US.load(Ordering::Relaxed),
                            if is_frozen { ", FROZEN" } else { "" },
                        )
                        .ok();
                        let mut line2 = [0u8; 256];
                        for s in 0..12u32 {
                            let rev = s / 6;
                            let sec = s % 6;
                            let start_tick = sec_starts[s as usize];
                            let end_tick = if s + 1 < 12 {
                                sec_starts[(s + 1) as usize]
                            } else {
                                window_end_tick
                            };
                            let start_off = (start_tick.wrapping_sub(window_start_tick)
                                & HALF_TICK_MASK)
                                as usize;
                            let end_off = (end_tick.wrapping_sub(window_start_tick)
                                & HALF_TICK_MASK)
                                as usize;
                            write!(&mut tx_writer, "[rev {} sec {}]: ", rev, sec,).ok();
                            tx_writer.write_blocking(&[]);
                            if end_off > HALF_TICKS || end_off < start_off {
                                tx_writer.write_blocking(b"(range invalid)\r\n");
                                continue;
                            }
                            let mut line_len = 0usize;
                            for k in start_off..end_off {
                                if line_len >= line2.len() {
                                    tx_writer.write_blocking(&line2);
                                    line_len = 0;
                                }
                                line2[line_len] = match buf[k] {
                                    0 => b'.',
                                    1 => b'o',
                                    2 => b'O',
                                    3 => b'*',
                                    _ => b'#',
                                };
                                line_len += 1;
                            }
                            tx_writer.write_blocking(&line2[..line_len]);
                            write!(&mut tx_writer, " [{}]\r\n", sec_counts[s as usize],).ok();
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
                    SECTOR_GATE_US.store(sector_gate_us(electrical_hz), Ordering::Relaxed);
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
                // Black-box dump: the 64 events leading to the kill.
                // dt is µs since the previous recorded event.
                const BB_NAMES: [&str; 10] = [
                    "REF", "BLD", "DRK", "ACC", "NOZ", "DIS", "DSY", "ENG", "STV", "RAQ",
                ];
                let idx = BB_IDX.load(Ordering::Relaxed) as usize;
                let mut prev_t: Option<u16> = None;
                for k in 0..BB_LEN {
                    let i = (idx + k) % BB_LEN;
                    let ty = BB_TYPE[i].load(Ordering::Relaxed);
                    if ty == 0xFF {
                        continue;
                    }
                    let t = BB_T[i].load(Ordering::Relaxed);
                    let dt = prev_t.map(|p| t.wrapping_sub(p) as u32 * 10).unwrap_or(0);
                    prev_t = Some(t);
                    write!(
                        &mut tx_writer,
                        "bb +{:6}us {} s{} d={}\r\n",
                        dt,
                        BB_NAMES.get(ty as usize).copied().unwrap_or("???"),
                        BB_SEC[i].load(Ordering::Relaxed),
                        BB_DATA[i].load(Ordering::Relaxed),
                    )
                    .ok();
                    tx_writer.service();
                }
                for e in BB_TYPE.iter() {
                    e.store(0xFF, Ordering::Relaxed);
                }
                BB_FROZEN.store(false, Ordering::Relaxed);
            }
            // Overcurrent trip report: the ISR already killed the
            // output; sync main's mirror so `r`/`q` re-arm works.
            if VBAT_SAGGED.load(Ordering::Relaxed) {
                VBAT_SAGGED.store(false, Ordering::Relaxed);
                output_enabled = false;
                let raw = VBAT_TRIP_RAW_SEEN.load(Ordering::Relaxed) as u32;
                let base = VBAT_BASELINE_RAW.load(Ordering::Relaxed) as u32;
                write!(
                    &mut tx_writer,
                    "!! VBAT SAG KILL: bus {} mV (1.3 ms sustained) < 90% of arm baseline {} mV - output killed (r/q re-arms)\r\n",
                    raw * 7507 / 1000,
                    base * 7507 / 1000,
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
            // MAGPIE drain: emit completed window records as 16-byte
            // frames. Emit all-or-nothing per frame — a partial frame
            // would desync the host parser; a dropped one just shows
            // as a seq gap.
            while let Some(rec) = wrec_consumer.dequeue() {
                if TX_RING_LEN - 1 - tx_writer.pending() >= WREC_FRAME_LEN {
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
        COMP_RATE.store(now_count.wrapping_sub(rate_start_count), Ordering::Relaxed);
        VALID_COMP_RATE.store(now_valid.wrapping_sub(rate_start_valid), Ordering::Relaxed);
        rate_start_count = now_count;
        rate_start_valid = now_valid;

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
    v.clamp(AMP_MIN as i32, AMP_MAX as i32) as u16
}

#[inline]
fn clamp_hz(v: i32) -> u32 {
    v.clamp(FREQ_MIN as i32, FREQ_MAX as i32) as u32
}

#[exception]
fn SysTick() {
    // Two tick stores per fire:
    // 1. The cheap `TICKS_10US_FAST` atomic — read by every hot-path
    //    `ticks_10us()` caller in the ISRs.
    // 2. The systick-timer crate's wrap counter — used by code that
    //    needs the 64-bit `SYSTICK_TIMER.now()` (none currently, but
    //    the wiring is kept so the crate stays useful for future
    //    sub-tick resolution work).
    // Both are ~3 cycles, so the SysTick ISR is still well under 20
    // cycles total — same load as before the crate was wired in.
    TICKS_10US_FAST.fetch_add(1, Ordering::Relaxed);
    SYSTICK_TIMER.systick_handler();
}

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
    if tick % 300 == 0 {
        let cur = AMPLITUDE_PCT.load(Ordering::Relaxed);
        let tgt = AMP_TARGET_PCT.load(Ordering::Relaxed);
        if cur < tgt {
            AMPLITUDE_PCT.store(cur + 1, Ordering::Relaxed);
        } else if cur > tgt {
            AMPLITUDE_PCT.store(cur - 1, Ordering::Relaxed);
        }
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
            let last = LAST_COMM_10US.load(Ordering::Relaxed);
            let since_ticks = ticks_10us().wrapping_sub(last);
            let since_us = if since_ticks < u32::MAX / 2 {
                since_ticks.saturating_mul(10)
            } else {
                0
            };
            // ZC starvation: 12 intervals (2 electrical revs) without
            // an ACCEPTED qZC = the loop is flying blind (stalled
            // rotor / zombie field) even though commutations continue.
            // Same reference-before-now read order as below.
            let last_qzc = LAST_QZC_10US.load(Ordering::Relaxed);
            let starve_ticks = ticks_10us().wrapping_sub(last_qzc);
            let starve_us = if starve_ticks < u32::MAX / 2 {
                starve_ticks.saturating_mul(10)
            } else {
                0
            };
            let starved =
                interval_us != 0 && last_qzc != 0 && starve_us > interval_us.max(500) * 12;
            // Runaway floor. Was 160 µs ("no plausible rotor under
            // ~1 kHz") — set in the 6.5 V / pre-advance era, and it
            // EXECUTED a healthy 1,050 Hz lock at amp 38 (bb: clean
            // ACC/REF chain at interval 158, then DSY d=3). Textbook
            // regime-expired guard. 60 µs = 2.8 kHz elec, beyond any
            // reachable speed at this voltage but far above the true
            // runaway signature (junk walks to the 24-50 µs
            // scheduler floor).
            let implausible = interval_us != 0 && interval_us < 60;
            if starved || implausible || (interval_us != 0 && since_us > interval_us.max(1_000) * 3)
            {
                bb_record(
                    if starved { 8 } else { 6 },
                    CURRENT_SECTOR.load(Ordering::Relaxed),
                    ((if starved { starve_us } else { since_us }) / 10).min(0xFFFF) as u16,
                );
                BB_FROZEN.store(true, Ordering::Relaxed);
                CL_ACTIVE.store(false, Ordering::Relaxed);
                MOTOR_ENABLED.store(false, Ordering::Relaxed);
                tim1_motor_pwm::all_off();
                comp2::set_exti_enabled(false);
                CL_STARVED.store(starved, Ordering::Relaxed);
                CL_DESYNC.store(true, Ordering::Relaxed);
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
        let inc = ANGLE_INC.load(Ordering::Relaxed);
        let mut accum = ANGLE_ACCUM.load(Ordering::Relaxed).wrapping_add(inc);
        while accum >= ANGLE_FULL_REV_FP {
            accum = accum.wrapping_sub(ANGLE_FULL_REV_FP);
        }
        ANGLE_ACCUM.store(accum, Ordering::Relaxed);
        let raw_angle = (accum >> 16) as i32;
        // Commutation angle = (raw + advance) rem-euclid 360. Signed
        // advance: positive = sector boundaries earlier in raw-angle
        // terms, negative = later. The rev-wrap point for EDGE_BUF
        // flipping is the sec 5→0 transition (below), which auto-
        // aligns with sec 0 regardless of advance sign.
        let adv = ADVANCE_DEG.load(Ordering::Relaxed) as i32;
        let commutation_angle = (raw_angle + adv).rem_euclid(360) as u16;
        let new_sector = open_loop::six_step_sector(commutation_angle);
        (new_sector, commutation_angle, new_sector != prev_sector)
    } else {
        (prev_sector, 0, false)
    };
    // Rev wrap = commutation sector 5 → 0 transition. This is where
    // sec 0 begins; the EDGE_BUF half-flip is aligned to this point so
    // each frozen half always starts at sec 0 regardless of advance.
    let rev_wrapped = sector_changed && prev_sector == 5 && sector == 0;

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
        unsafe {
            (*stm32::GPIOB::ptr())
                .bsrr
                .write(|w| w.bits(if was_high { 1 << (16 + 3) } else { 1 << 3 }));
        }
    }

    // EDGE_BUF rev-pair flip + sector-boundary recording, both inside
    // a critical section so COMP ISR (higher priority) can't snapshot
    // a torn (ACTIVE_HALF, HALF_START_TICK, REV_PHASE) tuple.
    if rev_wrapped || sector_changed {
        free(|_| {
            if rev_wrapped {
                // Two rev wraps = one EDGE_BUF half. XOR REV_PHASE on
                // every wrap, flip only when prev_phase==1 (= second
                // rev of the pair just completed).
                let prev_phase = REV_PHASE.fetch_xor(1, Ordering::Relaxed);
                if prev_phase == 1 && !EDGE_DUMP_FREEZE.load(Ordering::Relaxed) {
                    let now = ticks_10us();
                    let old_active = ACTIVE_HALF.load(Ordering::Relaxed) as usize;
                    let new_active = old_active ^ 1;
                    let old_half_start = HALF_START_TICK.load(Ordering::Relaxed);
                    // Clear new active half; previous pair's data
                    // stays in the soon-to-be-frozen half.
                    unsafe {
                        core::ptr::write_bytes(
                            EDGE_BUF[new_active].as_ptr() as *mut u8,
                            0,
                            HALF_TICKS,
                        );
                    }
                    // Zero the per-sector edge counters for the new
                    // active half so the next pair starts from 0.
                    for slot in 0..12 {
                        SECTOR_EDGE_COUNT[new_active][slot].store(0, Ordering::Relaxed);
                    }
                    // Finalize slot 0 of the half we're freezing (=
                    // start of just-completed pair = old HALF_START_TICK).
                    // Initialize slot 0 of the new active half to NOW
                    // (= start of the new pair). Together these
                    // guarantee slot 0 is always valid for completed
                    // pairs without depending on a sector-0 entry
                    // transition being recorded.
                    SECTOR_BOUNDARIES[old_active][0].store(old_half_start, Ordering::Relaxed);
                    SECTOR_BOUNDARIES[new_active][0].store(now, Ordering::Relaxed);
                    HALF_START_TICK.store(now, Ordering::Relaxed);
                    ACTIVE_HALF.store(new_active as u8, Ordering::Relaxed);
                }
            }
            // Record sector boundary tick into the (now possibly
            // updated) ACTIVE_HALF / REV_PHASE slot. For non-flip rev
            // wraps that's slot 6 of the old active half (rev 1
            // sec 0). For flip rev wraps it's slot 0 of the new
            // active half (rev 0 sec 0 of the new pair). For
            // mid-rev sector changes it's slot `phase*6+sector` of
            // the current active half.
            if sector_changed {
                let rev_phase = REV_PHASE.load(Ordering::Relaxed) as usize;
                let active = ACTIVE_HALF.load(Ordering::Relaxed) as usize;
                let slot = rev_phase * 6 + sector as usize;
                if slot < 12 {
                    SECTOR_BOUNDARIES[active][slot].store(ticks_10us(), Ordering::Relaxed);
                }
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
            let float_mask = FLOAT_SECTOR_MASK.load(Ordering::Relaxed);
            let in_float = (float_mask >> sector) & 1 != 0;
            let was_in_float = WAS_IN_FLOAT_SECTOR.load(Ordering::Relaxed);
            if in_float && !was_in_float {
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

/// MAGPIE/OWL float-window close: package the just-ended window into
/// a `WindowRec`, then reset the accumulators for the new one. Called
/// at every commutation, from whichever engine performed it — TIM7
/// (open loop) or the LPTIM2 one-shot (closed loop) — always inside a
/// critical section so the higher/equal-priority COMP ISR can't smear
/// an edge across the old/new window during snapshot-and-reset.
///
/// Interval smoothing lives in the COMP ISR (at the qZC itself);
/// here we only compute the boundary prediction error and break the
/// qZC chain when a window produced no qualified edge.
fn close_float_window(cs: &cortex_m::interrupt::CriticalSection, prev_sector: u8) {
    let now = ticks_10us();
    let now_us = ticks_1us();
    let start_us = SECTOR_START_US.load(Ordering::Relaxed);

    let qzc = WINDOW_QZC_US.load(Ordering::Relaxed);
    let mut pred_err: i16 = i16::MIN;
    if qzc != u32::MAX {
        let interval = OWL_INTERVAL_US.load(Ordering::Relaxed);
        if interval != 0 {
            let err = now_us.wrapping_sub(qzc) as i64 - (interval / 2) as i64;
            pred_err = err.clamp(i16::MIN as i64 + 1, i16::MAX as i64) as i16;
        }
    }
    if qzc == u32::MAX && CL_ACTIVE.load(Ordering::Relaxed) {
        bb_record(
            4,
            prev_sector,
            WINDOW_RAW.load(Ordering::Relaxed).min(0xFFFF) as u16,
        );
        // Re-acquisition trigger: only A/B windows count (phase-C
        // windows are dead-reckoned and legitimately ZC-less).
        if prev_sector != 0 && prev_sector != 3 {
            let run = CL_NOZ_RUN.load(Ordering::Relaxed).saturating_add(1);
            CL_NOZ_RUN.store(run, Ordering::Relaxed);
            if run >= 2 && !CL_REACQ.load(Ordering::Relaxed) {
                CL_REACQ.store(true, Ordering::Relaxed);
                // Break the qZC chain: recovery must be measured from
                // TWO fresh strict-confirmed ZCs, not from a stale
                // pre-spiral timestamp (a stale `last` hands the
                // re-seed an aliased delta — seen re-seeding 162 µs
                // and tripping the runaway floor).
                OWL_LAST_QZC_US.store(u32::MAX, Ordering::Relaxed);
                bb_record(
                    9,
                    prev_sector,
                    OWL_INTERVAL_US.load(Ordering::Relaxed).min(0xFFFF) as u16,
                );
            }
        }
    }
    // Windowless misses no longer hard-break the estimator chain —
    // dead-reckoned C windows legitimately close without a qZC; the
    // span counter (divide-by-spans, >3 = broken) handles both cases.
    let w = WINDOWS_SINCE_QZC.load(Ordering::Relaxed);
    WINDOWS_SINCE_QZC.store(w.saturating_add(1), Ordering::Relaxed);

    // Telemetry decimation (roadmap Phase 1): at 15 k windows/s the
    // 26 B records exceed the 2 Mbaud link (~200 kB/s). Above ~925 Hz
    // electrical stream every 5th window — 5 is coprime with 6 so the
    // sample keeps rotating through all sectors; the host sees the
    // seq gaps and per-sector stats stay unbiased.
    let decim_n = WREC_DECIM.fetch_add(1, Ordering::Relaxed);
    let iv_now = OWL_INTERVAL_US.load(Ordering::Relaxed);
    let stream_this = iv_now == 0 || iv_now >= 180 || decim_n % 5 == 0;
    if STREAM_ON.load(Ordering::Relaxed) && start_us != 0 && stream_this {
        let first_zc = WINDOW_FIRST_ZC_US.load(Ordering::Relaxed);
        let zc_off_us = if first_zc == u32::MAX {
            0xFFFF
        } else {
            first_zc.wrapping_sub(start_us).min(0xFFFE) as u16
        };
        let qzc_off_us = if qzc == u32::MAX {
            0xFFFF
        } else {
            qzc.wrapping_sub(start_us).min(0xFFFE) as u16
        };
        let i_n = WINDOW_I_N.load(Ordering::Relaxed);
        let rec = WindowRec {
            start_10us: start_us / 10,
            len_10us: (now_us.wrapping_sub(start_us) / 10).min(0xFFFF) as u16,
            zc_off_us,
            raw: WINDOW_RAW.load(Ordering::Relaxed).min(0xFFFF) as u16,
            valid: WINDOW_VALID.load(Ordering::Relaxed).min(0xFFFF) as u16,
            i_min: if i_n == 0 {
                0
            } else {
                WINDOW_I_MIN.load(Ordering::Relaxed)
            },
            i_max: WINDOW_I_MAX.load(Ordering::Relaxed),
            i_avg: if i_n == 0 {
                0
            } else {
                (WINDOW_I_SUM.load(Ordering::Relaxed) / i_n) as u16
            },
            qzc_off_us,
            pred_err_us: pred_err,
            vbat_raw: {
                let m = WINDOW_VBAT_MIN.swap(u16::MAX, Ordering::Relaxed);
                if m == u16::MAX {
                    VBAT_RAW_LIVE.load(Ordering::Relaxed)
                } else {
                    m
                }
            },
            sector: prev_sector,
            seq: WREC_SEQ.fetch_add(1, Ordering::Relaxed),
        };
        let mut prod = WREC_PROD.borrow(cs).borrow_mut();
        if let Some(p) = prod.as_mut() {
            // Drop on overflow — host sees the seq gap.
            let _ = p.enqueue(rec);
        }
    }
    WINDOW_RAW.store(0, Ordering::Relaxed);
    WINDOW_VALID.store(0, Ordering::Relaxed);
    WINDOW_FIRST_ZC_US.store(u32::MAX, Ordering::Relaxed);
    WINDOW_QZC_US.store(u32::MAX, Ordering::Relaxed);
    WINDOW_I_SUM.store(0, Ordering::Relaxed);
    WINDOW_I_N.store(0, Ordering::Relaxed);
    WINDOW_I_MIN.store(0x0FFF, Ordering::Relaxed);
    WINDOW_I_MAX.store(0, Ordering::Relaxed);
    CAND_ZC_US.store(u32::MAX, Ordering::Relaxed);
    WINDOW_GEN.store(
        WINDOW_GEN.load(Ordering::Relaxed).wrapping_add(1),
        Ordering::Relaxed,
    );
    SECTOR_START_US.store(now_us, Ordering::Relaxed);
    LAST_COMM_10US.store(now, Ordering::Relaxed);
}

/// Round-robin counter for the high-speed telemetry decimation.
static WREC_DECIM: AtomicU32 = AtomicU32::new(0);

/// FALCON commutation ISR — fires `interval·(30°−adv)/60°` after each
/// qualified ZC (scheduled by the COMP ISR). Shares priority 1 with
/// COMP so the two never nest.
#[interrupt]
fn LPTIM2() {
    minz::lptim2_oneshot::clear_flag();
    if !CL_ACTIVE.load(Ordering::Relaxed) || !MOTOR_ENABLED.load(Ordering::Relaxed) {
        return; // stale one-shot after disengage/kill
    }
    CL_COMM_COUNT.fetch_add(1, Ordering::Relaxed);
    let prev = CURRENT_SECTOR.load(Ordering::Relaxed);
    {
        // Black box: classify the commutation by the window it ends.
        let refined = SHOT_REFINED.swap(false, Ordering::Relaxed);
        let ev = if prev == 0 || prev == 3 {
            2 // DRK
        } else if refined {
            0 // REF
        } else {
            1 // BLD — A/B window commutated by the blind free-run
        };
        bb_record(
            ev,
            prev,
            OWL_INTERVAL_US.load(Ordering::Relaxed).min(0xFFFF) as u16,
        );
    }
    let sector = (prev + 1) % 6;
    let duty = open_loop::six_step_duty(max_duty(), AMPLITUDE_PCT.load(Ordering::Relaxed) as u16);
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
        // µs units (was 10 µs ticks — 15 % gate quantization at
        // high-speed window sizes).
        let g = if CL_REACQ.load(Ordering::Relaxed) {
            (interval * 2 / 25).max(10) // 8 %
        } else {
            // 30 % — the value every successful ladder ran at. (A
            // brief excursion to 20 % let early noise edges reach the
            // 1-confirm fast path and compound into estimator walks.)
            (interval * 3 / 10).max(20)
        };
        SECTOR_GATE_US.store(g, Ordering::Relaxed);
    }
    // Schedule the next commutation unconditionally:
    // - phase-C float windows (0/3, no ADC confirm): dead-reckon at
    //   exactly one estimator interval;
    // - A/B windows: a 1.5×interval FALLBACK shot, overwritten by the
    //   precise qZC accept when one lands. A missed ZC then costs one
    //   late commutation instead of a watchdog kill — that's what let
    //   v3.0 desync mid-acceleration.
    // AM32 semantics: the loop FREE-RUNS at the estimator interval —
    // every commutation immediately schedules the next one 1.0×T out
    // — and an accepted ZC merely RE-TIMES the pending shot to
    // zc + T·(30−adv)/60. During acceleration the un-refined windows
    // commutate at the trailing estimate (slightly long) while every
    // refined one pulls the phase back in; a 1.5×T fallback instead
    // compounded the lag until the watchdog fired.
    if interval != 0 {
        minz::lptim2_oneshot::schedule_us(interval);
    }
    // Scope trigger on each electrical rev, same as the open loop.
    if sector == 0 {
        let was_high = PB3_LEVEL.fetch_not(Ordering::Relaxed);
        unsafe {
            (*stm32::GPIOB::ptr())
                .bsrr
                .write(|w| w.bits(if was_high { 1 << (16 + 3) } else { 1 << 3 }));
        }
    }
    // Re-open the ear for the new window (clears any pending edge).
    comp2::set_exti_enabled(true);
}

#[interrupt]
fn TIM1_CC() {
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
    // One-byte COMP2 + sector sample per PWM period. UIF must be
    // cleared first or the IRQ re-fires immediately on return.
    // Byte layout: bit 0 = COMP2 value, bits 1..=3 = sector (0..=5).
    TIM1_UP_COUNT.fetch_add(1, Ordering::Relaxed);
    tim1_motor_pwm::clear_update_flag();
    // Harvest the PWM-synchronous current sample converted earlier in
    // this cycle (OC4REF falling edge at CNT=SAMPLE_TICKS triggered
    // it; by the update event it finished long ago). Sole writer of
    // the window accumulators between TIM7's `free`-wrapped resets,
    // and TIM7 (lower priority) can't interrupt us — plain load/store
    // min/max is race-free.
    let (pa_a, pa_b, i_raw) = adc_sync::last_frame();

    // FIRMWARE SAG KILL — pumped from the WRAP SLOT: harvesting +
    // restarting the injected vbat conversion here means it runs
    // 0-0.75 µs into the cycle, finished before the 1.25 µs regular
    // trigger — zero contention with the phase/current sequence
    // (the TIM7-resident version collided ~40 % of the time and
    // corrupted the sector confirms on two motors). Kill at −10 %
    // from the arm baseline (or the brownout backstop), debounced
    // 64 consecutive samples = 1.3 ms at 48 kHz — real sag is
    // milliseconds; single-sample glitches false-tripped once.
    if let Some(raw) = minz::adc_sync::vbat_pump() {
        VBAT_RAW_LIVE.store(raw, Ordering::Relaxed);
        if raw < VBAT_MIN_RAW.load(Ordering::Relaxed) {
            VBAT_MIN_RAW.store(raw, Ordering::Relaxed);
        }
        if raw < WINDOW_VBAT_MIN.load(Ordering::Relaxed) {
            WINDOW_VBAT_MIN.store(raw, Ordering::Relaxed);
        }
        let base = VBAT_BASELINE_RAW.load(Ordering::Relaxed);
        let thresh = (base - base / 10).max(VBAT_KILL_RAW);
        if raw < thresh && MOTOR_ENABLED.load(Ordering::Relaxed) {
            let run = VBAT_SAG_RUN.load(Ordering::Relaxed) + 1;
            if run >= 64 {
                VBAT_TRIP_RAW_SEEN.store(raw, Ordering::Relaxed);
                MOTOR_ENABLED.store(false, Ordering::Relaxed);
                tim1_motor_pwm::all_off();
                comp2::set_exti_enabled(false);
                CL_ACTIVE.store(false, Ordering::Relaxed);
                CL_ARMED.store(false, Ordering::Relaxed);
                VBAT_SAGGED.store(true, Ordering::Relaxed);
                VBAT_SAG_RUN.store(0, Ordering::Relaxed);
            } else {
                VBAT_SAG_RUN.store(run, Ordering::Relaxed);
            }
        } else {
            VBAT_SAG_RUN.store(0, Ordering::Relaxed);
        }
    }
    // Decaying-max vbus estimate (ADC counts through the phase
    // divider): the driven-high phase reads ≈ vbus in 4 of 6 sectors,
    // so the max refreshes constantly while spinning; τ ≈ 21 ms decay
    // tracks supply sag.
    let m = pa_a.max(pa_b);
    let est = VBUS_EST.load(Ordering::Relaxed);
    if m > est {
        VBUS_EST.store(m, Ordering::Relaxed);
    } else if est > 0 {
        VBUS_EST.store(est.saturating_sub((est >> 9).max(1)), Ordering::Relaxed);
    }
    LAST_I_RAW.store(i_raw, Ordering::Relaxed);
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
    let cand = CAND_ZC_US.load(Ordering::Relaxed);
    if cand != u32::MAX {
        let vbus = VBUS_EST.load(Ordering::Relaxed) as u32;
        let cur_sec = CURRENT_SECTOR.load(Ordering::Relaxed) & 7;
        let observed = match cur_sec {
            1 => (pa_b as u32) * 2 < pa_a as u32,
            4 => (pa_b as u32) * 2 < vbus + pa_a as u32,
            2 => (pa_a as u32) * 2 < pa_b as u32,
            5 => (pa_a as u32) * 2 < vbus + pa_b as u32,
            _ => comp2::value(),
        };
        if observed == CAND_EXPECTED.load(Ordering::Relaxed) {
            // Confirmation depth: the offline falcon_stats replay on
            // the probe captures showed 1-confirm in CLOSED-LOOP
            // conditions is 100 % accept / 1 % premature / +0.1±0.7
            // frames latency (vs 0 % / +1.1 for 2-confirm). The saved
            // PWM frame (~42 µs) raises the confirmation-latency
            // speed ceiling (~480 Hz at 2 confirms — hit at 7.5 V
            // where even amp 10 equilibrates above it). The 1 %
            // premature rate is backstopped by the symmetric interval
            // bound and the ZC-starvation watchdog.
            // CONDITIONAL depth: 1-confirm is only clean in locked
            // closed-loop conditions (probe replay: 1 % premature);
            // in open-loop/engage conditions it is 52-69 % premature
            // and seeds the estimator with junk — observed as engage
            // runaways to 144 µs after shipping unconditional
            // 1-confirm. So: 2 confirms until CL_ACTIVE, 1 after.
            // Re-acquisition demands full 2-confirm strictness —
            // trust is re-earned before the fast path resumes.
            let need: u8 = if CL_ACTIVE.load(Ordering::Relaxed) && !CL_REACQ.load(Ordering::Relaxed)
            {
                1
            } else {
                2
            };
            let n = CAND_CONFIRMS.load(Ordering::Relaxed) + 1;
            if n >= need {
                CAND_ZC_US.store(u32::MAX, Ordering::Relaxed);
                // Atomic accept: `free` excludes the commutation ISR
                // for the ~10 µs of estimator + re-schedule, and the
                // generation check drops candidates whose window
                // already closed (their ZC would re-time the wrong
                // shot).
                free(|_| {
                    if CAND_GEN.load(Ordering::Relaxed) == WINDOW_GEN.load(Ordering::Relaxed) {
                        accept_qualified_zc(cand);
                    }
                });
            } else {
                CAND_CONFIRMS.store(n, Ordering::Relaxed);
            }
        } else {
            if CL_ACTIVE.load(Ordering::Relaxed) {
                bb_record(5, cur_sec, CAND_CONFIRMS.load(Ordering::Relaxed) as u16);
            }
            CAND_ZC_US.store(u32::MAX, Ordering::Relaxed);
        }
    }

    // Overcurrent failsafe: 85 ms average vs the 1.5 A trip level.
    // This ISR is the accumulators' sole writer — plain load/store.
    // Kill directly from here (don't wait for main): all six gate
    // inputs to OUTPUT-LOW, TIM7 CCR updates off, COMP EXTI masked —
    // identical to the `w` key path.
    let acc = I_TRIP_ACC.load(Ordering::Relaxed) + i_raw as u32;
    let cnt = I_TRIP_CNT.load(Ordering::Relaxed) + 1;
    if cnt >= (1 << I_TRIP_SHIFT) {
        // Semantics matter: GECKO samples the shunt only INSIDE the
        // ON window, so this average is PHASE current — the battery
        // sees phase × duty. A phase-referred trip over-reads supply
        // draw by 1/duty and killed a healthy amp-54 run whose true
        // battery draw was ~1.6 A (PSU untouched at its 2.5 A
        // limit). AM32 never meets this bug because at 100 % duty
        // phase ≈ battery.
        // Under CL: battery-referred trip at ~2.2 A (the 2.5 A lab
        // supply in CC mode is the real protection; stall detection
        // is the ZC-starvation guard's job). Open loop: phase-
        // referred 2.0 A stays — it's the stall-heater guard and a
        // stalled drive at low duty IS phase ≈ battery.
        let avg_raw = acc >> I_TRIP_SHIFT;
        let tripped = if CL_ACTIVE.load(Ordering::Relaxed) {
            // THROTTLE-SCALED envelope (phase-referred, same units as
            // the lock maps' i_avg). The healthy locked curve is
            // near-linear in throttle: ~400 mA @ amp 30, ~520 @ 45,
            // ~780 @ 48. Envelope = 30·amp + 300 mA ≈ (amp·9/8 + 11)
            // raw counts — 2× above the healthy curve at every
            // throttle, while a divergence to 3-4 A (the burnt-motor
            // signature: in-sync current is linear, a stall/desync
            // jumps 5×) trips within one 85 ms window REGARDLESS of
            // throttle. Replaces a flat 2.8 A check that a 3.5 A
            // stall at amp 56 sailed under long enough to matter.
            // Gross-fault backstop only (~4 A phase average). The
            // tighter throttle-scaled envelopes (2× then 3.8× the
            // healthy curve) kept declaring walls in PASSABLE
            // terrain: AM32 runs this exact board/prop/voltage to
            // 100 % duty by riding THROUGH high-drag transitional
            // regions (~2.7 A at the ~1,350 Hz knee) — properly
            // commutated current in a locked motor is torque, and
            // the far side of the knee draws less again. Stall
            // burns are the starvation guard's job (ms), supply
            // collapse is the sag kill's, a wedged core is the
            // IWDG's — this trip only catches wiring/sense-level
            // faults.
            avg_raw > 150
        } else {
            avg_raw > I_TRIP_RAW // 2.0 A phase-referred
        };
        if tripped && MOTOR_ENABLED.load(Ordering::Relaxed) {
            MOTOR_ENABLED.store(false, Ordering::Relaxed);
            tim1_motor_pwm::all_off();
            comp2::set_exti_enabled(false);
            // Clear the CL flags too: a trip that leaves CL_ACTIVE
            // set produces a ZOMBIE status — `i` kept printing the
            // last "cl: ACTIVE f_e=..." line for 60 s of dead motor
            // and a ladder script sailed through ten fake rungs.
            CL_ACTIVE.store(false, Ordering::Relaxed);
            CL_ARMED.store(false, Ordering::Relaxed);
            OC_TRIPPED.store(true, Ordering::Relaxed);
        }
        I_TRIP_ACC.store(0, Ordering::Relaxed);
        I_TRIP_CNT.store(0, Ordering::Relaxed);
    } else {
        I_TRIP_ACC.store(acc, Ordering::Relaxed);
        I_TRIP_CNT.store(cnt, Ordering::Relaxed);
    }
    let idx = (PWM_SAMPLE_IDX.load(Ordering::Relaxed) & PWM_SAMPLE_MASK) as usize;
    let value = comp2::value() as u8;
    let sector = CURRENT_SECTOR.load(Ordering::Relaxed) & 0x07;
    PWM_SAMPLE_BUF[idx].store(value | (sector << 1), Ordering::Relaxed);
    PWM_SAMPLE_IDX.store(
        (idx as u32).wrapping_add(1) & PWM_SAMPLE_MASK,
        Ordering::Relaxed,
    );
}

#[interrupt]
fn COMP() {
    // EXTI line 22 is COMP2's output (COMP1 is line 21, unused here).
    // Ack the pending bit first so the IRQ doesn't immediately re-fire.
    comp2::clear_pending();
    COMP_COUNT.fetch_add(1, Ordering::Relaxed);
    WINDOW_RAW.fetch_add(1, Ordering::Relaxed);

    // Software PWM-edge blanking. The `TIM1_CC` ISR latches
    // `ticks_1us()` into `LAST_PWM_EDGE_US` at every PWM channel
    // transition. If we're inside the `BLANK_US` window after any
    // such transition, this edge is almost certainly PWM-coupled
    // ringing rather than a real BEMF event — drop it from the
    // EDGE_BUF / SECTOR_EDGE_COUNT recording.
    //
    // 1 µs resolution comes from `ticks_1us()` reading SYST.CVR as
    // the sub-10 µs-tick fraction. PWM cycle at 24 kHz = 41.67 µs,
    // so a sensible blank window is 1-30 µs.
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
    let user_blank = BLANK_US.load(Ordering::Relaxed) as u32;
    let blank_us = if interval_us != 0 {
        user_blank.min(interval_us / 75)
    } else {
        user_blank
    };
    if blank_us > 0 {
        let since_edge = ticks_1us().wrapping_sub(LAST_PWM_EDGE_US.load(Ordering::Relaxed));
        if since_edge < blank_us {
            let elapsed = ticks_1us().wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
            if elapsed >= SECTOR_GATE_US.load(Ordering::Relaxed) {
                VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
    }
    // Persistence depth for the qZC filter below: AM32-style
    // deepening as the time-blank fades.
    let persist_reads: u32 = if blank_us >= 5 { 5 } else { 12 };

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
        let elapsed = ticks_1us().wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
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
    let edge_idx = (ticks_10us().wrapping_sub(HALF_START_TICK.load(Ordering::Relaxed))
        & HALF_TICK_MASK) as usize;
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
    let elapsed = ticks_1us().wrapping_sub(SECTOR_START_US.load(Ordering::Relaxed));
    if elapsed < SECTOR_GATE_US.load(Ordering::Relaxed) {
        return;
    }
    VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
    // MAGPIE: per-window valid count + first-survivor timestamp.
    // This ISR is the sole writer between TIM7's `free`-wrapped
    // resets, so plain check-then-store is race-free.
    WINDOW_VALID.fetch_add(1, Ordering::Relaxed);
    if WINDOW_FIRST_ZC_US.load(Ordering::Relaxed) == u32::MAX {
        WINDOW_FIRST_ZC_US.store(ticks_1us(), Ordering::Relaxed);
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
            if CL_FAST_PATH.load(Ordering::Relaxed) && CL_ACTIVE.load(Ordering::Relaxed) {
                // SWIFT: accept right here — edge-timestamped,
                // AM32-style, zero wrap latency. Safe from this ISR:
                // LPTIM2 (commutation) shares priority 1 with COMP so
                // they never preempt each other, and TIM1_UP (prio 3)
                // is preempted — no `free` needed. WINDOW_QZC caps it
                // at one accept per window (mask-after-accept), and
                // all estimator bounds + guards apply unchanged.
                accept_qualified_zc(ticks_1us());
            } else if CAND_ZC_US.load(Ordering::Relaxed) == u32::MAX {
                // Record the candidate; TIM1_UP confirms or discards it.
                CAND_EXPECTED.store(expected, Ordering::Relaxed);
                CAND_CONFIRMS.store(0, Ordering::Relaxed);
                CAND_GEN.store(WINDOW_GEN.load(Ordering::Relaxed), Ordering::Relaxed);
                CAND_ZC_US.store(ticks_1us(), Ordering::Relaxed);
            }
        }
    }
}

/// FALCON v2 acceptance path — runs in TIM1_UP once a candidate has
/// held its post-ZC level across 2 PWM-cycle wrap samples: publish
/// the window's qualified ZC, update the OWL interval estimator, and
/// (closed loop) schedule the commutation one-shot, compensating the
/// delay for the confirmation latency.
fn accept_qualified_zc(zc_us: u32) {
    if WINDOW_QZC_US.load(Ordering::Relaxed) != u32::MAX {
        return; // window already has its ZC
    }
    WINDOW_QZC_US.store(zc_us, Ordering::Relaxed);

    // Span-divided interval update: a qZC-to-qZC delta that crossed a
    // dead-reckoned C window covers 2 intervals; >3 window closes
    // since the last qZC = chain broken (skip update, re-seed last).
    let last = OWL_LAST_QZC_US.load(Ordering::Relaxed);
    let spans = WINDOWS_SINCE_QZC.load(Ordering::Relaxed) as u32;
    WINDOWS_SINCE_QZC.store(0, Ordering::Relaxed);
    if last != u32::MAX && (1..=3).contains(&spans) {
        let new_int = zc_us.wrapping_sub(last) / spans;
        let old = OWL_INTERVAL_US.load(Ordering::Relaxed);
        if CL_REACQ.load(Ordering::Relaxed) && CL_ACTIVE.load(Ordering::Relaxed) {
            // Re-acquisition re-seed: both ZCs of this delta were
            // taken under the tiny gate + full 2-confirm strictness
            // (the chain was broken on entry, so spans==1 means two
            // FRESH measurements). Bound to [0.5, 2.0]×old — wide
            // enough to undo any walk-up/down the spiral caused,
            // narrow enough to reject aliased junk (the unbounded
            // version re-seeded 162 µs and tripped the runaway
            // floor).
            let old = OWL_INTERVAL_US.load(Ordering::Relaxed);
            if spans == 1
                && new_int > 100
                && new_int < 30_000
                && new_int > old / 2
                && new_int < old.saturating_mul(2)
            {
                OWL_INTERVAL_US.store(new_int, Ordering::Relaxed);
                CL_REACQ.store(false, Ordering::Relaxed);
            }
        } else {
            // Rate-of-change bounds. Tight under an established lock
            // (±25 %/window: real acceleration moves the interval a
            // fraction of a percent per window; the ~1.5× aliased
            // accept that seeds the lockout spiral cannot pass).
            // Engagement (old == 0) seeds directly.
            let sane = new_int > 100
                && new_int < 30_000
                && (old == 0
                    || (new_int < old.saturating_mul(5) / 4
                        && new_int > old.saturating_mul(4) / 5));
            if sane {
                let smoothed = if old == 0 {
                    new_int
                } else {
                    (3 * old + new_int) / 4
                };
                OWL_INTERVAL_US.store(smoothed, Ordering::Relaxed);
            }
        }
    }
    CL_NOZ_RUN.store(0, Ordering::Relaxed);
    OWL_LAST_QZC_US.store(zc_us, Ordering::Relaxed);
    LAST_QZC_10US.store(ticks_10us(), Ordering::Relaxed);

    // CL scheduling — only from A/B float windows (ADC-confirmed).
    // C windows (0/3) are dead-reckoned by the LPTIM2 ISR itself, so
    // a comp-rule qZC there must neither schedule nor engage.
    let interval = OWL_INTERVAL_US.load(Ordering::Relaxed);
    let sec = CURRENT_SECTOR.load(Ordering::Relaxed) & 7;
    if interval != 0 && sec != 0 && sec != 3 {
        if CL_ARMED.load(Ordering::Relaxed) {
            CL_ARMED.store(false, Ordering::Relaxed);
            CL_ACTIVE.store(true, Ordering::Relaxed);
            bb_record(7, sec as u8, interval.min(0xFFFF) as u16);
        }
        if CL_ACTIVE.load(Ordering::Relaxed) {
            // AUTO-ADVANCE ramp (roadmap Phase 1, AM32's auto_advance
            // cribbed in spirit): with `t`/`T` at 0 (the default),
            // advance follows measured speed — 0° below ~280 Hz
            // (where it was measured to do nothing), ramping to a
            // 12° cap by ~1.2 kHz. Evidence: at 850-950 Hz / 350 mA,
            // 8° was worth +108 Hz (the amp-34 droop was late-
            // commutation braking); a STATIC 16° broke the engage
            // transit — a speed-following ramp gives the transit 0°
            // and the top its timing automatically. A nonzero manual
            // setting overrides the ramp entirely.
            let manual = ADVANCE_DEG.load(Ordering::Relaxed) as i32;
            let adv = if manual != 0 {
                manual
            } else {
                ((600i32.saturating_sub(interval as i32)).max(0) / 45).min(12)
            };
            let elapsed = ticks_1us().wrapping_sub(zc_us) as i32;
            let delay = (interval as i32 * (30 - adv) / 60 - elapsed).max(24) as u32;
            minz::lptim2_oneshot::schedule_us(delay);
            SHOT_REFINED.store(true, Ordering::Relaxed);
            bb_record(3, sec as u8, delay.min(0xFFFF) as u16);
            // Deaf until the commutation (mask-after-accept).
            comp2::set_exti_enabled(false);
        }
    }
}
