//! Bench motor tester: open-loop TIM1 PWM driven by a software amplitude
//! that the host can adjust over UART.
//!
//! - **RX**: soft-UART on **PA0** via LPTIM1 + EXTI0 (mirrors
//!   `bitbang_uart_lptim1.rs`). Single-key commands from the host.
//! - **TX**: hardware USART1 on **PB6** for status messages back to host.
//!   PB7 is also claimed as USART1_RX even though we don't use it,
//!   because the HAL's `Serial::usart1` constructor wants both pins.
//! - **Motor PWM**: TIM1 driving HIN1-3 / LIN1-3 via the AM32 pin map
//!   (same as `output_pins.rs`).
//!
//! Wire to host:
//! - USB-TTL TX  → PA0 (host → ESC, key commands)
//! - PB6         → USB-TTL RX (ESC → host, status messages)
//! - GND ↔ GND
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
use fugit::HertzU32 as Hertz;
use heapless::Deque;
use heapless::spsc::Queue;
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2;
use minz::current_adc::SenseAdc;
use minz::hal::gpio::gpioa::PA0;
use minz::hal::gpio::{Edge, ExtiPin, Input, PullUp};
use minz::hal::lptimer::{ClockSource, Event, LowPowerTimer, LowPowerTimerConfig, PreScaler};
use minz::hal::pac::{LPTIM1, USART1, interrupt};
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial, Tx};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::idle_loop::IdleLoop;
use minz::open_loop::{self, Waveform};
use minz::priority;
use minz::softuart::{IrqAck, Rx, SoftUart};
use minz::tim1_motor_pwm::{self, max_duty};
use minz::{SYSTICK, tim7_drive};
use portable_atomic::{AtomicBool, AtomicI8, AtomicU8, AtomicU16, AtomicU32, Ordering};
use rtt_target::rprintln;

const BAUD: Hertz = Hertz::Hz(9600);
const OVERSAMPLE: usize = 4;
const SAMPLE: Hertz = Hertz::Hz(BAUD.raw() * OVERSAMPLE as u32);
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
const AMP_MAX: u16 = 20;
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

type Uart = SoftUart<'static, { BAUD.raw() }, { SYSTICK.raw() }, OVERSAMPLE>;
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
static LPTIM1_COUNT: AtomicU32 = AtomicU32::new(0);
static EXTI0_COUNT: AtomicU32 = AtomicU32::new(0);
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

/// 10 µs tick of TICKS_10US captured at the moment the COMP EXTI line
/// is unmasked (= start of a float sector). The ISR uses this with
/// `SECTOR_HALF_TICKS` to gate edge acceptance.
static SECTOR_START_TICK: AtomicU32 = AtomicU32::new(0);

/// Half of one float-sector duration in 10 µs ticks. Computed in
/// `main` from `electrical_hz`; refreshed when the user changes f.
static SECTOR_HALF_TICKS: AtomicU32 = AtomicU32::new(0);

/// Half-sector duration in 10 µs ticks at a given electrical
/// frequency. 6 sectors per electrical rev → full sector duration =
/// `SYSTICK / (6 × f)`; half is `SYSTICK / (12 × f)`. At f=60 →
/// 138 ticks ≈ 1.38 ms. At f=600 → 13 ticks ≈ 130 µs.
const fn sector_half_ticks(electrical_hz: u32) -> u32 {
    SYSTICK.raw() / (12 * electrical_hz)
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
static AMPLITUDE_PCT: AtomicU8 = AtomicU8::new(0);

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

type RxPin = PA0<Input<PullUp>>;
type RxTimer = LowPowerTimer<LPTIM1>;
type RxBundle = Rx<RxPin, RxTimer, Uart>;

static RX: Mutex<RefCell<Option<RxBundle>>> = Mutex::new(RefCell::new(None));

/// Bounded software byte queue + USART1 TX, used only from the main
/// context (no ISR access → plain `Deque`, not SPSC). `write!` macros
/// target this via the [`fmt::Write`] impl which just enqueues bytes —
/// no busy-wait on `TXE`. The motor loop calls [`service`] each idle
/// `nop` pass; that tries to shift exactly one byte from the queue head
/// to USART1's TDR via the *non-blocking* `embedded-hal` write. If TXE
/// isn't set we leave the byte and try again next pass.
///
/// Bytes are dropped if the queue is full — fine for status updates;
/// not OK if you wanted reliable delivery.
///
/// [`service`]: UartTxWriter::service
struct UartTxWriter {
    tx: Tx<USART1>,
    queue: Deque<u8, TX_QUEUE_LEN>,
}

const TX_QUEUE_LEN: usize = 256;

impl UartTxWriter {
    fn new(tx: Tx<USART1>) -> Self {
        Self {
            tx,
            queue: Deque::new(),
        }
    }

    /// Try to push one byte from the queue head to USART1.TDR. No-op if
    /// the queue is empty or TXE is not yet set.
    fn service(&mut self) {
        if let Some(&b) = self.queue.front()
            && self.tx.write(b).is_ok()
        {
            self.queue.pop_front();
        }
    }

    /// Block-write a byte slice straight to USART1, after first
    /// draining the pending queue so byte order is preserved. Used
    /// by `L` (PWM sample dump) where the output is ~1 KB — far
    /// bigger than the 64-byte queue and not worth growing it for.
    fn write_blocking(&mut self, bytes: &[u8]) {
        while let Some(&b) = self.queue.front() {
            while self.tx.write(b).is_err() {}
            self.queue.pop_front();
        }
        for &b in bytes {
            while self.tx.write(b).is_err() {}
        }
    }
}

impl core::fmt::Write for UartTxWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // Drop bytes on overflow — status messages aren't critical.
            let _ = self.queue.push_back(b);
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
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut dp = stm32::Peripherals::take().unwrap();
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
        "motor_tester: clocks sysclk={} pclk1={}",
        clocks.sysclk().raw(),
        clocks.pclk1().raw(),
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

    // Soft-UART RX on PA0: pull-up input + EXTI0 falling edge.
    let mut rx_pin = gpioa
        .pa0
        .into_pull_up_input(&mut gpioa.moder, &mut gpioa.pupdr);
    rx_pin.make_interrupt_source(&mut dp.SYSCFG, &mut apb2);
    rx_pin.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    rx_pin.enable_interrupt(&mut dp.EXTI);

    // Bench instrumentation: PA3 = ADC1_IN8 (supply current),
    // PA6 = ADC1_IN11 (battery voltage). Oneshot HAL reads, ~16 µs each.
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let mut sense_adc = SenseAdc::new(
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
        Config::default().baudrate(BAUD.raw().bps()),
        clocks,
        &mut apb2,
    );
    let (mut tx, _) = serial.split();

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

    // TIM7 motor-drive heartbeat at `MOTOR_DRIVE_HZ` (= 6 kHz). The
    // `TIM7` ISR (below) reads the motor-drive atomics each tick and
    // writes TIM1's CCRs (sine path) or commutates (six-step path).
    // Main loop never touches TIM1 directly anymore.
    tim7_drive::init(dp.TIM7, &mut apb1r1, MOTOR_DRIVE_HZ);

    // LPTIM1 for soft-UART sample rate (= OVERSAMPLE × BAUD).
    let lptim_ticks_per_sample = clocks.pclk1().raw() / SAMPLE.raw();
    let arr = (lptim_ticks_per_sample - 1) as u16;
    let cmp = arr / 2;
    let lptim_cfg = LowPowerTimerConfig::default()
        .clock_source(ClockSource::PCLK)
        .prescaler(PreScaler::U1)
        .arr_value(arr)
        .compare_value(cmp);
    let mut timer = LowPowerTimer::lptim1(dp.LPTIM1, lptim_cfg, &mut apb1r1, &mut ccipr, clocks);
    // `listen` toggles ENABLE to write IER, wiping ARR/CMP — re-program.
    timer.listen(Event::CompareMatch);
    timer.set_autoreload(arr);
    timer.set_compare_match(cmp);

    let (producer, mut consumer) = RX_QUEUE.split();

    free(|cs| {
        RX.borrow(cs)
            .replace(Some(Rx::new(rx_pin, timer, SoftUart::new(producer))));
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
    ANGLE_INC.store(angle_inc_fp(FREQ_START), Ordering::Relaxed);
    FLOAT_SECTOR_MASK.store(
        float_sector_mask(comp2::ObservedPhase::A),
        Ordering::Relaxed,
    );
    SECTOR_HALF_TICKS.store(sector_half_ticks(FREQ_START), Ordering::Relaxed);
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
    writeln!(&mut tx, "\r\n=== Hello, this is motor tester ===\r").ok();
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
    writeln!(&mut tx, "Cycle observed phase: p   (A=PA4, B=PA5, C=PB7)\r").ok();
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
        "Cycle commut advance: t   (0 / 20 / 40 / -40 / -20 deg)\r"
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
        "Freeze/dump edge buf: E   (1st press: freeze + dump; 2nd press: resume)\r"
    )
    .ok();
    writeln!(
        &mut tx,
        "Boot: OUTPUT OFF, f = 0 Hz, amp = {} % (cap {}). Press r/q to arm.\r",
        AMP_START, AMP_MAX,
    )
    .ok();

    let mut tx_writer = UartTxWriter::new(tx);

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
    // LPTIM1=4. EXTI0 isn't in the canonical `set_irq_prios` list (it
    // pairs with LPTIM1 for the bench's soft-UART) — set it explicitly
    // at the same level as LPTIM1 so the soft-UART subsystem is the
    // lowest-priority tier as a whole.
    unsafe {
        priority::set_irq_prios();
        priority::set_irq_prio(Interrupt::EXTI0, priority::PRIO_LPTIM1);
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
        NVIC::unmask(Interrupt::LPTIM1);
        NVIC::unmask(Interrupt::EXTI0);
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
    let mut last_i_lptim: u32 = LPTIM1_COUNT.load(Ordering::Relaxed);
    let mut last_i_exti0: u32 = EXTI0_COUNT.load(Ordering::Relaxed);
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
                    b'm' => {
                        waveform = match waveform {
                            Waveform::Sine => Waveform::SixStep,
                            Waveform::SixStep => Waveform::Sine,
                        };
                    }
                    b'r' => {
                        // Panic reset: back to a known-good idle config.
                        // The existing prev_amp / prev_hz / prev_mode delta
                        // checks below pick this up and print the changes.
                        waveform = Waveform::SixStep;
                        amplitude_pct = AMP_START;
                        electrical_hz = FREQ_START;
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
                        waveform = Waveform::SixStep;
                        amplitude_pct = AMP_START;
                        electrical_hz = 50;
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
                            observed_phase.name(),
                            COMP_RATE.load(Ordering::Relaxed),
                            VALID_COMP_RATE.load(Ordering::Relaxed),
                            comp2::value() as u8,
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
                        comp2::set_observed_phase(observed_phase);
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
                        // Oneshot reads on both instrumentation pins
                        // (~16 µs each). Convert to engineering units
                        // using the empirical bench calibration above.
                        let i_raw = sense_adc.isns_raw();
                        let i_mv = sense_adc.adc_to_mv(i_raw);
                        let i_ma = i_mv as u32 * 1000 / ISNS_MV_PER_AMP;
                        let v_raw = sense_adc.vbat_raw();
                        let v_mv = sense_adc.adc_to_mv(v_raw);
                        let v_supply_mv = v_mv as u32 * VBAT_DIVIDER_X100 / 100;
                        write!(
                            &mut tx_writer,
                            "vbat={}.{:03}V isns={}.{:03}A cpu={}% (raw v={} i={})\r\n",
                            v_supply_mv / 1000,
                            v_supply_mv % 1000,
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
                        let now_lptim = LPTIM1_COUNT.load(Ordering::Relaxed);
                        let now_exti0 = EXTI0_COUNT.load(Ordering::Relaxed);
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
                                 lptim1={} exti0={} (dt={}.{:02}s)  blank={}us\r\n",
                                rate(now_comp.wrapping_sub(last_i_comp)),
                                rate(now_tim1.wrapping_sub(last_i_tim1)),
                                rate(now_tim1_cc.wrapping_sub(last_i_tim1_cc)),
                                rate(now_tim7.wrapping_sub(last_i_tim7)),
                                rate(now_lptim.wrapping_sub(last_i_lptim)),
                                rate(now_exti0.wrapping_sub(last_i_exti0)),
                                dtick / 100_000,
                                (dtick % 100_000) / 1000,
                                BLANK_US.load(Ordering::Relaxed),
                            )
                            .ok();
                        }
                        last_i_tick = now_tick;
                        last_i_comp = now_comp;
                        last_i_lptim = now_lptim;
                        last_i_exti0 = now_exti0;
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
                    b't' => {
                        // Cycle commutation advance: 0 → 20 → 40 → -40 → -20 → 0.
                        // Positive = float window earlier in raw-angle
                        // terms (compensate rotor lag). Negative =
                        // float window later (compensate rotor lead, or
                        // explore the asymmetry of the BEMF response).
                        let cur = ADVANCE_DEG.load(Ordering::Relaxed);
                        let next: i8 = match cur {
                            0 => 20,
                            20 => 40,
                            40 => -40,
                            -40 => -20,
                            _ => 0,
                        };
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
                    AMPLITUDE_PCT.store(amplitude_pct as u8, Ordering::Relaxed);
                    write!(&mut tx_writer, "amp={}\r\n", amplitude_pct).ok();
                }
                if electrical_hz != prev_hz {
                    ANGLE_INC.store(angle_inc_fp(electrical_hz), Ordering::Relaxed);
                    SECTOR_HALF_TICKS.store(sector_half_ticks(electrical_hz), Ordering::Relaxed);
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
fn EXTI0() {
    EXTI0_COUNT.fetch_add(1, Ordering::Relaxed);
    free(|cs| {
        let mut rx_borrow = RX.borrow(cs).borrow_mut();
        let Some(rx) = rx_borrow.as_mut() else { return };
        rx.pin.clear_interrupt_pending_bit();
        rx.uart.on_falling_edge(ticks_10us());
    });
}

#[interrupt]
fn LPTIM1() {
    LPTIM1_COUNT.fetch_add(1, Ordering::Relaxed);
    free(|cs| {
        let mut rx_borrow = RX.borrow(cs).borrow_mut();
        let Some(rx) = rx_borrow.as_mut() else { return };
        rx.timer.ack();
        rx.uart.on_sample(rx.pin.is_high());
    });
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
    TIM7_COUNT.fetch_add(1, Ordering::Relaxed);

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

        // Track entry into the observed phase's float window so the
        // COMP-ISR's time-window gate (`valid` rate counter) has a
        // meaningful `SECTOR_START_TICK` reference. EXTI is not gated
        // here — edges are recorded into `EDGE_BUF` across the whole
        // rev.
        let float_mask = FLOAT_SECTOR_MASK.load(Ordering::Relaxed);
        let in_float = (float_mask >> sector) & 1 != 0;
        let was_in_float = WAS_IN_FLOAT_SECTOR.load(Ordering::Relaxed);
        if in_float && !was_in_float {
            SECTOR_START_TICK.store(ticks_10us(), Ordering::Relaxed);
        }
        WAS_IN_FLOAT_SECTOR.store(in_float, Ordering::Relaxed);
    } else {
        // Sine: continuous 3-phase, no float window.
        let (c1, c2, c3) = open_loop::sine_duties(angle, arr, amp);
        tim1_motor_pwm::set_duties(c1, c2, c3);
        WAS_IN_FLOAT_SECTOR.store(false, Ordering::Relaxed);
    }
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
    let blank_us = BLANK_US.load(Ordering::Relaxed) as u32;
    if blank_us > 0 {
        let since_edge = ticks_1us().wrapping_sub(LAST_PWM_EDGE_US.load(Ordering::Relaxed));
        if since_edge < blank_us {
            let elapsed = ticks_10us().wrapping_sub(SECTOR_START_TICK.load(Ordering::Relaxed));
            if elapsed >= SECTOR_HALF_TICKS.load(Ordering::Relaxed) {
                VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            return;
        }
    }

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
        let elapsed = ticks_10us().wrapping_sub(SECTOR_START_TICK.load(Ordering::Relaxed));
        if elapsed >= SECTOR_HALF_TICKS.load(Ordering::Relaxed) {
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
    let elapsed = ticks_10us().wrapping_sub(SECTOR_START_TICK.load(Ordering::Relaxed));
    if elapsed < SECTOR_HALF_TICKS.load(Ordering::Relaxed) {
        return;
    }
    VALID_COMP_COUNT.fetch_add(1, Ordering::Relaxed);
}
