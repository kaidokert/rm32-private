//! AM32-L431 control-loop clone (operator directive 2026-07-20:
//! "cut ruthlessly and build up — literally do what AM32 does in its
//! 3 control loops").
//!
//! CONTROL is a line-cited transliteration of AM32 `Src/main.c` +
//! `Mcu/l431/Src/{stm32l4xx_it.c,comparator.c,peripherals.c}` for
//! target VIMDRONES_L431, factory-default EEPROM, with the fork's
//! `UART_DUTY_MODE` + `ZC_TRACE` bench protocols. Every control-path
//! constant / branch cites its AM32 line.
//!
//! Module layout: this program keeps the storage (statics + cluster
//! wiring), the orchestrators (main / main_entry + its bands), and the
//! vector table; the control layer lives host-tested in
//! `minz_core::{am32_control, am32_isr, am32_loop, zct_trace}`
//! reaching hardware only through the `minz_core::am32_hal` traits
//! (static dispatch — the zero-sized register impls are wired in the
//! `motor()` / `observer()` bundles below).
//!
//! The four AM32 contexts, mapped 1:1 to hardware here:
//!   1. COMP ISR  (their COMP_IRQHandler + interruptRoutine)  prio 0
//!      — gate on INTERVAL_TIMER, persistence, mask, timestamp, arm COM.
//!   2. COM ISR   (TIM1_UP_TIM16 vector -> PeriodElapsedCallback) prio 0
//!      — commutate, interval blend, advance, re-enable comp.
//!   3. TIM6 20 kHz (their tenKhzRoutine)  prio 3
//!      — duty pipeline + apply, polling-mode (old_routine) commutation.
//!   4. main while(1) — uart poll, e_com_time / average_interval,
//!      setInput duty setpoint, variable_pwm, desync_check, bemf
//!      timeout, filter_level / duty_cycle_maximum maps, telemetry.
//!
//! Quantities are AM32's: INTERVAL_TIMER (TIM2) + COM_TIMER (TIM16)
//! both tick at **0.5 µs** (PSC=39 @ 80 MHz) — so `commutation_interval`,
//! `average_interval`, `waitTime`, `thiszctime` are all 0.5 µs units,
//! exactly as in AM32.
//!
//! NON-CONTROL additions (observer-only, allowed by the directive):
//!   - ZC_TRACE 15-byte record stream (5B A9 sync) — AM32 fork verbatim.
//!   - 64-event black box (minz_core::blackbox), dumped on kill / `b`.
//!   - injected ADC (A/B/current/vbat) harvested in TIM6 for telemetry
//!     and the two bench-safety kills ONLY.
//!   - three bench-safety KILLS: hard overcurrent, boot-relative vbat
//!     floor (70% of first harvest), IWDG. None modulate the loop;
//!     they only stop it.
//!
//! Comparator-polarity note (load-bearing): minz's `comp2::value()` is
//! the INVERSE of AM32's `getCompOutputLevel()` (POLARITY wiring — see
//! minz/CLAUDE.md and `AM32 main.c:831 "polarity reversed"`). So AM32's
//! `getCompOutputLevel() == rising` (reject) transliterates to
//! `comp2::value() != rising` here, and the EXTI edge for each sector
//! uses the minz-verified `drive::edges_for(3, sector)` (equivalent to
//! AM32's `changeCompInput` `if(rising)` edge select in this polarity).

#![no_std]
#![no_main]

use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;

use minz::adc_sync::{self, InjAdc1};
use minz::am32_timers::{Am32Timers, com_timer_init, interval_timer_init};
use minz::bb::{Bb, CortexCs};
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2::{self, Comp2};
use minz::current_adc::SenseAdc;
use minz::hal::pac::interrupt;
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::priority;
use minz::tim1_motor_pwm::{self, Tim1Pwm};
use minz::tim6_loop::Tim6Loop;
use minz::uart_tx::{TX_RING_LEN, UartTxWriter};
use minz::{Am32Motor, Am32Observer};

use minz_core::am32::{UartDuty, ZCT_REC, ZctRing};
use minz_core::am32_control::{
    bemf_timeout_rekick, desync_check_band, honor_stop, set_input, variable_pwm_ride,
};
use minz_core::am32_hal::{Motor, Observer};
use minz_core::am32_isr::{comp_isr, tim1_up_tim16_isr, tim6_dacunder_isr};
use minz_core::am32_loop::{
    Bench, DELAY_CAP_CYC, DUTY_FULL, Drive, Duty, INIT_INTERVAL_TICKS, Sched,
    TARGET_MIN_BEMF_COUNTS, apply_uart_cmd, bemf_timeout_resets, filter_and_duty_max,
    min_bemf_schedule, store_average_interval,
};
use minz_core::zct_trace::ZctTrace;

use rtt_target::rprintln;

// ===============================================================
// Bench-only constants. The AM32 factory constants moved to
// `minz_core::am32_loop`; TIMER1_MAX_ARR (`minz::TIM1_AUTORELOAD`)
// is threaded into variable_pwm_ride as its `base_arr` parameter
// from main_entry; the bench-safety kill thresholds moved to
// minz_core::am32_isr (adc_harvest_and_safety).
// ===============================================================

/// 2 Mbaud link (both directions) — the minz-rig baud (AM32 fork
/// uart_duty_init main.c:1363, motor_tester2 parity).
const BAUD: u32 = 2_000_000;

// ===============================================================
// AM32 globals as atomics (main.c:400-576). All 0.5 µs / 2000-domain.
// ===============================================================

static COMMUTATION_INTERVAL: AtomicU32 = AtomicU32::new(INIT_INTERVAL_TICKS);
/// The last-6-intervals history (main.c:441,887). e_com_time is their sum.
static COMMUTATION_INTERVALS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_ZC: AtomicU16 = AtomicU16::new(0); // lastzctime
static THIS_ZC: AtomicU16 = AtomicU16::new(0); // thiszctime
static WAIT_TIME: AtomicU16 = AtomicU16::new(0);
static ZERO_CROSSES: AtomicU32 = AtomicU32::new(0);
static BEMF_COUNTER: AtomicU16 = AtomicU16::new(0);
static BAD_COUNT: AtomicU16 = AtomicU16::new(0);
static CURRENT_STEP: AtomicU16 = AtomicU16::new(1); // AM32 step 1..6
static RISING: AtomicBool = AtomicBool::new(true);
static OLD_ROUTINE: AtomicBool = AtomicBool::new(true); // polling at boot
static RUNNING: AtomicBool = AtomicBool::new(false);
static ZCFOUND: AtomicBool = AtomicBool::new(false);
static DESYNC_CHECK: AtomicBool = AtomicBool::new(false);
static DESYNC_HAPPENED: AtomicU32 = AtomicU32::new(0);
static BEMF_TIMEOUT_HAPPENED: AtomicU32 = AtomicU32::new(0);

static FILTER_LEVEL: AtomicU16 = AtomicU16::new(5); // main.c:477
static MIN_BEMF_UP: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);
static MIN_BEMF_DOWN: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);

static INPUT: AtomicU16 = AtomicU16::new(0); // 0..2047 (main.c:557)
static ADJUSTED_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DUTY_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DEADMAN_TICKS: AtomicU32 = AtomicU32::new(0);

static DUTY_CYCLE_SETPOINT: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static LAST_DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE_MAXIMUM: AtomicU16 = AtomicU16::new(DUTY_FULL);
static TENKHZ_COUNTER: AtomicU16 = AtomicU16::new(0);
/// AM32's `tim1_arr` variable (main.c:434) — the live carrier ARR as
/// STATE, not a register readback (rm32's PwmOutput has no ARR
/// getter): variable_pwm_ride writes it, duty_apply's
/// `duty*tim1_arr/2000` rescale reads it (main.c:1790-1791). Seeded
/// at the base carrier ARR, like AM32's boot value.
static TIM1_ARR_SHADOW: AtomicU16 = AtomicU16::new(minz::TIM1_AUTORELOAD);

/// Latched fatal kill (bench-safety). Main prints and holds off.
static KILLED: AtomicBool = AtomicBool::new(false);
static KILL_REASON: AtomicU16 = AtomicU16::new(0); // 1=OC, 2=vbat
static I_RAW: AtomicU16 = AtomicU16::new(0);
static VBAT_RAW: AtomicU16 = AtomicU16::new(0);

/// Stop request from the RX parser (bench pct=0 / 'w'): honored in main.
static STOP_REQ: AtomicBool = AtomicBool::new(false);
static DUMP_REQ: AtomicBool = AtomicBool::new(false);
static INFO_REQ: AtomicBool = AtomicBool::new(false);
/// 'G' — GECKO free-run current-ring capture (main-context, one-shot).
static GECKO_REQ: AtomicBool = AtomicBool::new(false);
/// 'X' — WAXWING-lite phase-voltage-ring dump (main-context, one-shot).
static WAX_REQ: AtomicBool = AtomicBool::new(false);
/// 'H' — per-ISR duration histogram dump (main-context, one-shot).
static HIST_REQ: AtomicBool = AtomicBool::new(false);
/// 'F' — toggle the free-run ADC oversample (rm32-like scan injector)
/// continuously; reproduces the ADC-injection jitter on the clone.
static FREERUN_REQ: AtomicBool = AtomicBool::new(false);
static FREERUN_ON: AtomicBool = AtomicBool::new(false);
/// Drop-proof late-window counter (jitter metric for the rm32 head-to-
/// head): commutations whose ZC-to-ZC interval ran >=1.5x the average.
static LATE_WINDOWS: AtomicU32 = AtomicU32::new(0);
static ZCT_STREAM_ON: AtomicBool = AtomicBool::new(true);

// ===============================================================
// ISR-delay injection rig (bench-only causal test for the deaf-window
// PRIMASK hypothesis, 2026-07-26). Both default 0 = no effect. The
// TIM6 (priority 3) trampoline busy-waits `DELAY_IN_FREE_CYC` cycles
// INSIDE a `cortex_m::interrupt::free` critical section (masks COMP for
// its whole span) and `DELAY_OUT_FREE_CYC` cycles OUTSIDE any (COMP can
// preempt) — so the bench can see which (if either) induces a deaf-
// window wall on the known-good clone. 80 MHz → 80 cyc/µs; each hard-
// capped at `am32_loop::DELAY_CAP_CYC` (4000 cyc ≈ 50 µs) so a fat-
// finger can't wedge the tick under the 1 s IWDG. Bumped by the ']'/'['
// (in-free ±) and '\''/';' (out-free ±) keys via `apply_uart_cmd`;
// reported as `din=`/`dout=` on the info line.
// ===============================================================
static DELAY_IN_FREE_CYC: AtomicU32 = AtomicU32::new(0);
static DELAY_OUT_FREE_CYC: AtomicU32 = AtomicU32::new(0);

/// Which phase floats in each sector (minz textbook convention, matches
/// `set_roles_for_step`): sector 0/3 → C, 1/4 → B, 2/5 → A.
// (SECTOR_FLOAT_PHASE moved to comp2::AM32_SECTOR_FLOAT_PHASE with
// the comparator.c helpers; now_10us moved to minz::am32_timers.)

// ===============================================================
// Black box (observer) — behavior in minz::bb; the instance lives here.
// ===============================================================
static BB: Bb = Bb::new();

// ===============================================================
// The HAL bundles — zero-sized register impls (static dispatch, no
// dyn) threaded through minz_core::am32_control. `motor()` is the
// rm32-verbatim `MotorHal` bundle (the five `&mut self` seams, held
// by value — free ZSTs); `observer()` is the minz-owned bb/cs/adc/lt
// bundle. Each context builds its own instances via these wiring
// constructors. The only places that name them are the ISR
// trampolines and main_entry's top (the house rule).
// ===============================================================
#[inline(always)]
fn motor() -> Am32Motor {
    Motor {
        pwm: Tim1Pwm,
        comp: Comp2,
        phase: Tim1Pwm,
        interval: Am32Timers,
        com: Am32Timers,
    }
}

#[inline(always)]
fn observer() -> Am32Observer<'static> {
    Observer {
        bb: &BB,
        cs: &CortexCs,
        adc: &InjAdc1,
        lt: &Tim6Loop,
    }
}

// zcfoundroutine spin-guard counter (DEVIATION #1, minz::am32_control).
static ZCFR_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

// TIM6 duty-pipeline / overcurrent accumulator storage (reached via
// the Duty/Bench clusters; bodies in minz_core::am32_isr).
static RAMP_COUNT: AtomicU16 = AtomicU16::new(0);
static OC_ACC: AtomicU32 = AtomicU32::new(0);
static OC_CNT: AtomicU32 = AtomicU32::new(0);
static VBAT_LOW_TICKS: AtomicU32 = AtomicU32::new(0);
/// Boot-latched vbat kill floor (70% of first harvest; 0 = unlatched).
static VBAT_FLOOR_RAW: AtomicU16 = AtomicU16::new(0);
/// Bench probe for the rm32 camp-storm differential (2026-07-25):
/// total COMP ISR entries, counted in the trampoline. Paired with
/// zct comm_n on the info line -> avg entries/window.
static COMP_ENTRIES: AtomicU32 = AtomicU32::new(0);

// ===============================================================
// Per-ISR duration histogram (CPU-margin control instrument, 2026-07-26
// — the control leg for the rm32 `ten_khz_tick` A/B). The clone has NO
// 1 kHz PID block, so its TIM6 tick should stay tight where rm32's has
// a 15% tail at 25-32 µs; this histogram measures it.
//
// BIN LAW — MUST match rm32's byte-for-byte for the bin-by-bin compare:
//   idx = min(delta_cycles >> SHIFT, NBINS-1)
// with SHIFT=8 (256-cycle bins = 3.2 µs @ 80 MHz DWT.CYCCNT), NBINS=16
// (covers 0..4096 cyc ≈ 0..51 µs). Bin 0 = underflow naturally (<3.2 µs);
// bin 15 = overflow, all passes ≥ 3840 cyc ≈ ≥48 µs. The delta is a
// wrapping u32 cycle subtraction, so it is correct across a CYCCNT wrap.
// Rows: index 0 = TIM6, 1 = TIM16 (COM), 2 = COMP.
// Always-on (constant cost) so a later wall-window capture works too.
// ===============================================================
const HIST_SHIFT: u32 = 8;
const HIST_NBINS: usize = 16;
const HIST_TIM6: usize = 0;
const HIST_TIM16: usize = 1;
const HIST_COMP: usize = 2;
static ISR_HIST: [[AtomicU32; HIST_NBINS]; 3] =
    [const { [const { AtomicU32::new(0) }; HIST_NBINS] }; 3];

/// Record one ISR pass duration into [`ISR_HIST`]. Constant-cost and
/// branchless (`.min` lowers to a `cmov`); it MUST NOT grow a
/// value-dependent heavy path — the project's constant-per-tick rule
/// (and the [t16] scar): this instrument must not become the blip it
/// measures. `start_cyc` is the DWT.CYCCNT read taken as the first line
/// of the trampoline (same idiom as `minz::am32_timers::ticks_10us`).
#[inline]
fn hist_record(isr: usize, start_cyc: u32) {
    let d = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(start_cyc);
    let idx = ((d >> HIST_SHIFT) as usize).min(HIST_NBINS - 1);
    ISR_HIST[isr][idx].fetch_add(1, Ordering::Relaxed);
}

// ===============================================================
// WAXWING-lite phase-voltage rings (`X` key, 2026-07-26) — the
// voltage-domain demag instrument, companion to the GECKO current
// microscope. The injected ADC already samples phase A (ch9/PA4)
// and phase B (ch10/PA5) mid-ON every PWM cycle; the TIM6 harvest
// discards both. `wax_tick` (TIM6 trampoline) rings them at the
// 20 kHz tick with position metadata; the host reconstructs the
// per-window demag-clamp profile by EQUIVALENT-TIME scatter — many
// windows folded onto one axis of INTERVAL_TIMER position (0.5 µs
// ticks since the last ZC/commutation reference), each sample's
// position first corrected for HARVEST SKEW via the packed TIM1.CNT
// (the injected burst fired at CNT==CCR4; the ring write happens up
// to a full carrier period later). Rings are ALWAYS-ON (4 atomic
// stores/tick is negligible), so a dump shows the LAST ~52 ms —
// post-mortem capture works after any event.
// ===============================================================
const WAX_N: usize = 1024;
/// Phase A raw 12-bit (injected ch9/PA4, mid-ON).
static WAX_A: [AtomicU16; WAX_N] = [const { AtomicU16::new(0) }; WAX_N];
/// Phase B raw 12-bit (injected ch10/PA5, mid-ON).
static WAX_B: [AtomicU16; WAX_N] = [const { AtomicU16::new(0) }; WAX_N];
/// INTERVAL_TIMER (TIM2) CNT at ring-write time — 0.5 µs position in
/// the current commutation window.
static WAX_POS: [AtomicU16; WAX_N] = [const { AtomicU16::new(0) }; WAX_N];
/// Packed `(post_zc << 15) | (step << 12) | (TIM1.CNT & 0x0FFF)` — AM32
/// step 1..6 (bits 12..14) plus the carrier counter (bits 0..11, for
/// harvest-skew correction: ARR ≤ 3332 < 4096, so 12 bits always hold
/// CNT). bit15 = normalized post-ZC (`value()==rising`); rm32-comparable
/// (both = raw!=rising) for the WAXWING comp-fraction cross-check.
static WAX_T1S: [AtomicU16; WAX_N] = [const { AtomicU16::new(0) }; WAX_N];
/// Next slot `wax_tick` writes (sole writer = the TIM6 trampoline).
static WAX_HEAD: AtomicUsize = AtomicUsize::new(0);

// ===============================================================
// INTERVAL_TIMER = TIM2, COM_TIMER = TIM16 — the register wrappers
// (interval_timer_init / interval_cnt / set_interval_cnt /
// com_timer_init / set_and_enable_com_int / disable_com_timer_int /
// com_set_arr / com_clear_flag) live in `minz::am32_timers`.
// ===============================================================

// ===============================================================
// comparator.c transliteration — moved to minz::comp2 (the am32_*
// helpers + AM32_SECTOR_FLOAT_PHASE live with their peripheral).
// ===============================================================

// ===============================================================
// map()/getAbsDif() (functions.c) now live in `minz_core::am32`.
// ===============================================================

// ===============================================================
// ZC_TRACE ring — 15-byte records (main.c:1536-1567), 5B A9 sync.
// Behavior (write incl. batch decimation, drain) in
// minz_core::zct_trace; this file owns only the storage and wiring.
// ===============================================================
const ZCT_N: usize = 32;
static ZCT_RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static ZCT_HEAD: AtomicUsize = AtomicUsize::new(0);
static ZCT_TAIL: AtomicUsize = AtomicUsize::new(0);
static ZCT_DROP: AtomicU32 = AtomicU32::new(0);
static ZCT_COMM_N: AtomicU32 = AtomicU32::new(0);
static ZCT_BATCHING: AtomicBool = AtomicBool::new(false);

// ===============================================================
// USART2 RX: DMA-circular, no ISR, no statics here — ring + tail
// live in minz::usart2_rx (ported back from rm32 bench_uart.rs);
// main drains via minz::usart2_rx::pop() in rx_drain.
// (minz_core::am32::RxRing keeps its host tests but has no firmware
// user anymore.)
// ===============================================================

// ===============================================================
// Cohesion clusters — each struct bundles the loose statics above
// that are written/read together, holding ONLY `&'static Atomic*`
// refs (storage is unchanged). Every servicing fn takes the clusters
// it touches as `&` params; the ONLY places that name the SCHED /
// DRIVE / DUTY / BENCH / ZCT instances are the ISR trampolines,
// main_entry's top (local wiring), and these definitions.
// (Sched / Drive / Duty / Bench struct types + the pure band fns live
// in minz_core::am32_loop, host-tested there; ZctTrace behavior lives
// in minz_core::zct_trace — critical section + byte sink arrive
// through the am32_hal seams.)
// ===============================================================

static SCHED: Sched<'static> = Sched {
    commutation_interval: &COMMUTATION_INTERVAL,
    interval_hist: &COMMUTATION_INTERVALS,
    average_interval: &AVERAGE_INTERVAL,
    last_average_interval: &LAST_AVERAGE_INTERVAL,
    last_zc: &LAST_ZC,
    this_zc: &THIS_ZC,
    wait_time: &WAIT_TIME,
};

static DRIVE: Drive<'static> = Drive {
    current_step: &CURRENT_STEP,
    rising: &RISING,
    old_routine: &OLD_ROUTINE,
    running: &RUNNING,
    zcfound: &ZCFOUND,
    bemf_counter: &BEMF_COUNTER,
    min_bemf_up: &MIN_BEMF_UP,
    min_bemf_down: &MIN_BEMF_DOWN,
    zero_crosses: &ZERO_CROSSES,
    filter_level: &FILTER_LEVEL,
    bad_count: &BAD_COUNT,
    desync_check: &DESYNC_CHECK,
    desync_happened: &DESYNC_HAPPENED,
    bemf_timeout_happened: &BEMF_TIMEOUT_HAPPENED,
    tenkhz_counter: &TENKHZ_COUNTER,
    zcfr_guard_hits: &ZCFR_GUARD_HITS,
};

static DUTY: Duty<'static> = Duty {
    input: &INPUT,
    adjusted_input: &ADJUSTED_INPUT,
    uart_duty_input: &UART_DUTY_INPUT,
    duty_cycle_setpoint: &DUTY_CYCLE_SETPOINT,
    duty_cycle: &DUTY_CYCLE,
    last_duty_cycle: &LAST_DUTY_CYCLE,
    duty_cycle_maximum: &DUTY_CYCLE_MAXIMUM,
    ramp_count: &RAMP_COUNT,
    killed: &KILLED,
    kill_reason: &KILL_REASON,
    tim1_arr: &TIM1_ARR_SHADOW,
};

static BENCH: Bench<'static> = Bench {
    uart_deadman_ticks: &UART_DEADMAN_TICKS,
    i_raw: &I_RAW,
    vbat_raw: &VBAT_RAW,
    oc_acc: &OC_ACC,
    oc_cnt: &OC_CNT,
    vbat_low_ticks: &VBAT_LOW_TICKS,
    vbat_floor_raw: &VBAT_FLOOR_RAW,
    stop_req: &STOP_REQ,
    dump_req: &DUMP_REQ,
    info_req: &INFO_REQ,
    gecko_req: &GECKO_REQ,
    wax_req: &WAX_REQ,
    hist_req: &HIST_REQ,
    freerun_req: &FREERUN_REQ,
    zct_stream_on: &ZCT_STREAM_ON,
    delay_in_free: &DELAY_IN_FREE_CYC,
    delay_out_free: &DELAY_OUT_FREE_CYC,
};

static ZCT: ZctTrace<'static, ZCT_N> = ZctTrace {
    ring: ZctRing {
        ring: &ZCT_RING,
        head: &ZCT_HEAD,
        tail: &ZCT_TAIL,
        drop: &ZCT_DROP,
    },
    comm_n: &ZCT_COMM_N,
    batching: &ZCT_BATCHING,
};

// ===============================================================
// commutate / zcfoundroutine / startMotor / safety_kill / honor_stop /
// variable_pwm_ride / bemf_timeout_rekick / desync_check_band /
// setInput — host-tested in minz_core::am32_control over the
// am32_hal traits (the pure bands stay in minz_core::am32_loop).
// ===============================================================

// ===============================================================
fn main_entry(tx_writer: &mut UartTxWriter) -> ! {
    // Wire the cluster locals once (the only main-context place allowed to
    // name the static instances). Everything below references these.
    let sched = &SCHED;
    let drive = &DRIVE;
    let duty = &DUTY;
    let bench = &BENCH;
    let zct = &ZCT;
    let mut motor = motor();
    let hal = &mut motor;
    let obs = &observer();
    // AM32 TIMER1_MAX_ARR (targets.h:5335) — the base carrier ARR,
    // threaded into variable_pwm_ride (core can't see minz's const).
    let base_arr = minz::TIM1_AUTORELOAD;

    // Main-context UART parser state (mirrors uart_duty_poll main.c:1367).
    let mut uart = UartDuty::new();
    // ramp_count local-ish (ramp_divider=0 → ramp every tenKhz tick, so
    // main only needs the maps below; ramp lives in the TIM6 ISR).

    // Self-monitoring instrument (feature="monitor"): fed in main context so
    // it adds zero cost to the prio-0 commutation/COMP ISRs.
    #[cfg(feature = "monitor")]
    let mut monitor = minz::monitor::Monitor::new();

    // krabilorean qualification instrument (feature="krabimon"): same onboard
    // role as the ratch22 monitor, fed in main context (off the hot ISR path).
    #[cfg(feature = "krabimon")]
    let mut krabimon = minz::krabimon::Krabimon::new();

    loop {
        minz::iwdg::refresh();
        rx_drain(&mut uart, duty, bench);
        honor_stop(drive, duty, bench, hal);

        // e_com_time (main.c:2159): (sum+4)>>1, 0.5 µs units. Threaded into
        // the average_interval + low-rpm ceiling bands below.
        let e_com_time = sched.intervals().e_com_time();
        // input = uart_duty_get()  (main.c:1131) then setInput()
        set_input(sched, drive, duty, hal, obs);
        min_bemf_schedule(drive);
        variable_pwm_ride(sched, duty, base_arr, hal);

        let average_interval = store_average_interval(sched, e_com_time);
        desync_check_band(sched, drive, duty, obs, average_interval);

        let (running, zc) = filter_and_duty_max(sched, drive, duty, e_com_time, average_interval);
        bemf_timeout_resets(drive, duty, zc);
        bemf_timeout_rekick(sched, drive, duty, zct, hal, obs, running);

        // Monitor: feed each NEW commutation interval through ratch22 and
        // check for surprises (main context, off the hot ISR path).
        #[cfg(feature = "monitor")]
        monitor.poll(
            zct.comm_n.load(Ordering::Relaxed),
            sched.commutation_interval.load(Ordering::Relaxed),
            bench.i_raw.load(Ordering::Relaxed),
        );
        #[cfg(feature = "krabimon")]
        krabimon.poll(
            zct.comm_n.load(Ordering::Relaxed),
            sched.commutation_interval.load(Ordering::Relaxed),
            bench.i_raw.load(Ordering::Relaxed),
        );

        telemetry_drain(bench, zct, tx_writer);
        handle_requests(sched, drive, duty, bench, zct, tx_writer);
    }
}

/// RX drain band — drain the USART2 DMA ring → uart_duty parser
/// dispatch (uart_duty_poll main.c:1367). Bytes arrive via DMA1_CH6
/// (minz::usart2_rx), so this poll is the ONLY RX consumer — no ISR.
#[inline]
fn rx_drain(uart: &mut UartDuty, duty: &Duty, bench: &Bench) {
    while let Some(c) = minz::usart2_rx::pop() {
        // Monitor control keys are intercepted here (not valid UART_DUTY
        // keys, so the number parser never sees them) — runtime tier/K dials.
        #[cfg(feature = "monitor")]
        if matches!(c, b'k' | b'K' | b'm') {
            minz::monitor::key(c);
            continue;
        }
        // krabimon tier dial ('m'); only when the monitor isn't also claiming it.
        #[cfg(all(feature = "krabimon", not(feature = "monitor")))]
        if c == b'm' {
            minz::krabimon::key(c);
            continue;
        }
        apply_uart_cmd(duty, bench, uart.step(c));
    }
}

/// telemetry band: drain ZC_TRACE ring → USART1 DMA writer, then service.
#[inline]
fn telemetry_drain(bench: &Bench, zct: &ZctTrace<ZCT_N>, tx: &mut UartTxWriter) {
    if bench.zct_stream_on.load(Ordering::Relaxed) {
        zct.drain(|b| {
            let _ = tx.push(b);
        });
    }
    tx.service();
}

/// on-demand info / bb dump / kill-notice band.
#[inline]
fn handle_requests(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    bench: &Bench,
    zct: &ZctTrace<ZCT_N>,
    tx: &mut UartTxWriter,
) {
    if bench.info_req.swap(false, Ordering::Relaxed) {
        print_info(sched, drive, duty, bench, zct, tx);
    }
    if bench.dump_req.swap(false, Ordering::Relaxed) {
        dump_bb(tx);
    }
    if bench.gecko_req.swap(false, Ordering::Relaxed) {
        gecko_dump(sched, tx);
    }
    if bench.wax_req.swap(false, Ordering::Relaxed) {
        wax_dump(sched, duty, tx);
    }
    if bench.hist_req.swap(false, Ordering::Relaxed) {
        hist_dump(tx);
    }
    // 'F' — toggle the free-run ADC oversample continuously (the
    // rm32-like scan injector) to reproduce the jitter on the clone.
    if bench.freerun_req.swap(false, Ordering::Relaxed) {
        if FREERUN_ON.load(Ordering::Relaxed) {
            adc_sync::oversample_stop();
            FREERUN_ON.store(false, Ordering::Relaxed);
        } else {
            adc_sync::oversample_start();
            FREERUN_ON.store(true, Ordering::Relaxed);
        }
        let _ = write!(
            BlockingFmt { tx: &mut *tx },
            "FR freerun={}\r\n",
            FREERUN_ON.load(Ordering::Relaxed) as u8
        );
    }
    // Kill NOTICE is one-shot via kill_reason; `killed` itself stays
    // LATCHED (TIM6 duty/polling gate + the set_input inert gate hold
    // until reset — consuming it here made the fault restartable, the
    // kill/restart oscillation the 2026-07-23 structure report found).
    let reason = duty.kill_reason.swap(0, Ordering::Relaxed);
    if reason != 0 {
        let _ = write!(
            tx,
            "!! KILL reason={} (1=OC 2=vbat) iraw={} vbat={}\r\n",
            reason,
            bench.i_raw.load(Ordering::Relaxed),
            bench.vbat_raw.load(Ordering::Relaxed),
        );
        dump_bb(tx);
    }
}

fn print_info(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    bench: &Bench,
    zct: &ZctTrace<ZCT_N>,
    tx: &mut UartTxWriter,
) {
    let _ = write!(
        tx,
        "i step={} old={} run={} ci={} avg={} zc={} duty={} iraw={} vbat={} drop={} guard={} killed={}\r\n",
        drive.current_step.load(Ordering::Relaxed),
        drive.old_routine.load(Ordering::Relaxed) as u8,
        drive.running.load(Ordering::Relaxed) as u8,
        sched.commutation_interval.load(Ordering::Relaxed),
        sched.average_interval.load(Ordering::Relaxed),
        drive.zero_crosses.load(Ordering::Relaxed),
        duty.duty_cycle.load(Ordering::Relaxed),
        bench.i_raw.load(Ordering::Relaxed),
        bench.vbat_raw.load(Ordering::Relaxed),
        zct.ring.drop.load(Ordering::Relaxed),
        drive.zcfr_guard_hits.load(Ordering::Relaxed),
        duty.killed.load(Ordering::Relaxed) as u8,
    );
    // Camp-storm probe line (bench diagnostic, reads the probe static
    // directly): COMP entries vs commutations since boot.
    let _ = write!(
        tx,
        "ce={} comm={} dsy={} bt={} vfl={} din={} dout={} late={}\r\n",
        COMP_ENTRIES.load(Ordering::Relaxed),
        zct.comm_n.load(Ordering::Relaxed),
        drive.desync_happened.load(Ordering::Relaxed),
        drive.bemf_timeout_happened.load(Ordering::Relaxed),
        VBAT_FLOOR_RAW.load(Ordering::Relaxed),
        DELAY_IN_FREE_CYC.load(Ordering::Relaxed),
        DELAY_OUT_FREE_CYC.load(Ordering::Relaxed),
        LATE_WINDOWS.load(Ordering::Relaxed),
    );
    // Monitor line (feature="monitor"): surprises / samples, the in-situ DWT
    // cost per update, and the EW interval mean/variance the band is built on.
    #[cfg(feature = "monitor")]
    {
        use minz::monitor as mon;
        let _ = write!(
            tx,
            "mon tier={} k={} n={} cyc={} min={} isurp={} imean={} ivar={} \
             csurp={} cmean={} cvar={} skew={} kurt={} eps={}\r\n",
            mon::TIER.load(Ordering::Relaxed),
            mon::K.load(Ordering::Relaxed),
            mon::SAMPLES.load(Ordering::Relaxed),
            mon::LAST_CYC.load(Ordering::Relaxed),
            mon::MIN_CYC.load(Ordering::Relaxed),
            mon::CI_SURP.load(Ordering::Relaxed),
            mon::CI_MEAN_TICKS.load(Ordering::Relaxed),
            mon::CI_VAR_TICKS2.load(Ordering::Relaxed),
            mon::CUR_SURP.load(Ordering::Relaxed),
            mon::CUR_MEAN.load(Ordering::Relaxed),
            mon::CUR_VAR.load(Ordering::Relaxed),
            mon::SKEW_M.load(Ordering::Relaxed),
            mon::KURT_M.load(Ordering::Relaxed),
            mon::EPOCHS.load(Ordering::Relaxed),
        );
        // Onboard analytics (full tier, current channel): short/long trend
        // slope, P² median/p90, window-retuned histogram epoch + counts.
        let _ = write!(
            tx,
            "mon.a win={} tS={} tL={} p50={} p90={} hep={} h={},{},{},{},{},{},{},{}\r\n",
            mon::AN_WIN.load(Ordering::Relaxed),
            mon::AN_TREND_SHORT_M.load(Ordering::Relaxed),
            mon::AN_TREND_LONG_M.load(Ordering::Relaxed),
            mon::AN_P50.load(Ordering::Relaxed),
            mon::AN_P90.load(Ordering::Relaxed),
            mon::AN_HIST_EPOCH.load(Ordering::Relaxed),
            mon::AN_HIST[0].load(Ordering::Relaxed),
            mon::AN_HIST[1].load(Ordering::Relaxed),
            mon::AN_HIST[2].load(Ordering::Relaxed),
            mon::AN_HIST[3].load(Ordering::Relaxed),
            mon::AN_HIST[4].load(Ordering::Relaxed),
            mon::AN_HIST[5].load(Ordering::Relaxed),
            mon::AN_HIST[6].load(Ordering::Relaxed),
            mon::AN_HIST[7].load(Ordering::Relaxed),
        );
    }
    // krabilorean qualification line (feature="krabimon"): online core_merge
    // per-channel stats + measured online cost; then the windowed BasicProfile
    // batch cost + histogram mode + autocorrelation regularity markers.
    #[cfg(feature = "krabimon")]
    {
        use minz::krabimon as krab;
        let _ = write!(
            tx,
            "krab tier={} n={} cyc={} min={} | i[mn={} mx={} avg={} var={} mad={}] \
             c[mn={} mx={} avg={} mad={}]\r\n",
            krab::TIER.load(Ordering::Relaxed),
            krab::SAMPLES.load(Ordering::Relaxed),
            krab::LAST_CYC.load(Ordering::Relaxed),
            krab::MIN_CYC.load(Ordering::Relaxed),
            krab::I_MIN.load(Ordering::Relaxed),
            krab::I_MAX.load(Ordering::Relaxed),
            krab::I_MEAN.load(Ordering::Relaxed),
            krab::I_VAR.load(Ordering::Relaxed),
            krab::I_MAD.load(Ordering::Relaxed),
            krab::C_MIN.load(Ordering::Relaxed),
            krab::C_MAX.load(Ordering::Relaxed),
            krab::C_MEAN.load(Ordering::Relaxed),
            krab::C_MAD.load(Ordering::Relaxed),
        );
        // Windowed batch path: epochs, on-target batch cost (incl. u128 variance
        // + autocorrelation products), window variance, histogram mode bin, and
        // the autocorrelation markers (lag-1 acf ×1000, first zero-cross lag,
        // first local-min lag) — the commutation-regularity signal.
        let _ = write!(
            tx,
            "krab.w win={} wcyc={} wmin={} wvar={} mode={} acf1={} zc={} lmin={}\r\n",
            krab::WIN_N.load(Ordering::Relaxed),
            krab::WIN_CYC.load(Ordering::Relaxed),
            krab::WIN_MIN_CYC.load(Ordering::Relaxed),
            krab::W_VAR.load(Ordering::Relaxed),
            krab::W_MODE.load(Ordering::Relaxed),
            krab::W_ACF1.load(Ordering::Relaxed),
            krab::W_ZC.load(Ordering::Relaxed),
            krab::W_LMIN.load(Ordering::Relaxed),
        );
    }
}

fn dump_bb(tx: &mut UartTxWriter) {
    BB.dump(|b| {
        for &byte in b {
            let _ = tx.push(byte);
        }
    });
}

// ===============================================================
// GECKO on-demand current microscope (`G` key, 2026-07-26 — the rm32
// battery-wall hunt's healthy-reference capture). Runs INLINE in main
// context: free-run oversample ON → ring wraps once → freeze → ASCII
// hex dump → oversample OFF (back to inject-only; the free-run must
// not run during normal operation — the +13-count injected-ch8 bias).
// The ~10 KB dump exceeds the 4 KiB TX ring, so bytes go through a
// bounded push-service soft-spin; main blocks ~50 ms at 2 Mbaud —
// acceptable for this diagnostic (ISRs preempt freely, IWDG is 1 s).
// ===============================================================

/// Push one byte, soft-spinning `service()` while the TX ring is full
/// (`push` returns `false` on a full ring — nothing may be dropped in
/// a GECKO dump). Bounded: at 2 Mbaud the ring drains in ~20 ms, so
/// the guard only trips if the wire itself is dead.
fn push_blocking(tx: &mut UartTxWriter, b: u8) {
    for _ in 0..1_000_000u32 {
        if tx.push(b) {
            return;
        }
        tx.service();
    }
}

/// `core::fmt::Write` adapter over [`push_blocking`] (the header /
/// footer lines must not drop bytes like the plain `write!` path does).
struct BlockingFmt<'a> {
    tx: &'a mut UartTxWriter,
}

impl core::fmt::Write for BlockingFmt<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            push_blocking(self.tx, b);
        }
        Ok(())
    }
}

/// Push one u16 as 4 lowercase hex chars (shared by the GECKO and
/// WAXWING dump lines).
fn push_hex_u16(tx: &mut UartTxWriter, v: u16) {
    for shift in [12u32, 8, 4, 0] {
        let n = ((v >> shift) & 0xF) as u8;
        push_blocking(tx, if n < 10 { b'0' + n } else { b'a' + n - 10 });
    }
}

/// One dump line: 16 ring words as 4-hex-char u16s, space-separated.
fn gecko_line(tx: &mut UartTxWriter, base: usize) {
    for k in 0..16 {
        push_hex_u16(tx, adc_sync::cur_word(base + k));
        push_blocking(tx, if k == 15 { b'\r' } else { b' ' });
    }
    push_blocking(tx, b'\n');
}

/// The `G` capture band: oversample → wrap-once wait → freeze → dump
/// (raw ring order; host reorders from `start=`) → back to inject-only.
fn gecko_dump(sched: &Sched, tx: &mut UartTxWriter) {
    adc_sync::oversample_start();
    // Ring holds ≈0.55 ms (~150 samples/PWM cycle); wait ~1 ms so it
    // wraps once and every slot is fresh. Timing need not be precise —
    // just >0.6 ms; a counted cycle delay is plenty.
    cortex_m::asm::delay(80_000); // ~1 ms at 80 MHz
    let start = adc_sync::freeze_current();
    let ci = sched.commutation_interval.load(Ordering::Relaxed);
    let _ = write!(
        BlockingFmt { tx: &mut *tx },
        "GK n={} start={} ci={}\r\n",
        adc_sync::CUR_FRAMES,
        start,
        ci
    );
    for line in 0..(adc_sync::CUR_FRAMES / 16) {
        gecko_line(tx, line * 16);
    }
    let _ = write!(BlockingFmt { tx: &mut *tx }, "GK END\r\n");
    // Leave the free-run OFF: freeze already ADSTP'd; this is the
    // documented return-to-inject-only call (no resume_current).
    adc_sync::oversample_stop();
}

// ===============================================================
// WAXWING-lite dump band (`X` key — see the ring statics above for
// the instrument). Mirrors the GECKO path: main-context dump through
// push_blocking (nothing may drop). Dumping WHILE THE MOTOR RUNS is
// the intended use — the rings keep writing during the dump (TIM6
// preempts main), so records near the snapshotted head may be torn;
// the host discards ±8 records around head. 1024 records × 4 u16 ≈
// 18 KB ASCII ≈ 90 ms blocking at 2 Mbaud (ISRs preempt freely,
// IWDG is 1 s) — acceptable for this diagnostic.
// ===============================================================

/// TIM6-trampoline ring write — one record per 20 kHz tick. JDR
/// reads are idempotent/non-destructive (the core harvest reads them
/// again inside `tim6_dacunder_isr`).
#[inline]
fn wax_tick() {
    let (a, b, _cur, _vbat) = adc_sync::inj_read();
    let pos = minz::am32_timers::interval_cnt() as u16; // 0.5 µs ticks
    let t1 = tim1_motor_pwm::tim1_cnt() & 0x0FFF;
    let step = CURRENT_STEP.load(Ordering::Relaxed);
    // bit15 = normalized post-ZC (value()==rising); rm32-comparable (both =
    // raw!=rising); WAXWING comp-fraction cross-check 2026-07-27. Computing
    // it in-firmware from the clone's own value()/rising means no inversion
    // can creep into the host decode.
    let v = minz::comp2::value();
    let r = RISING.load(Ordering::Relaxed);
    let post_zc = v == r;
    let h = WAX_HEAD.load(Ordering::Relaxed);
    WAX_A[h].store(a, Ordering::Relaxed);
    WAX_B[h].store(b, Ordering::Relaxed);
    WAX_POS[h].store(pos, Ordering::Relaxed);
    WAX_T1S[h].store(
        (step << 12) | t1 | ((post_zc as u16) << 15),
        Ordering::Relaxed,
    );
    WAX_HEAD.store((h + 1) % WAX_N, Ordering::Relaxed);
}

/// One dump line: 4 records, each `A B POS T1S` as 4-hex-char u16s —
/// 16 space-separated hex fields per line (the GECKO line shape).
fn wax_line(tx: &mut UartTxWriter, base: usize) {
    for r in 0..4 {
        let i = base + r;
        let rec = [
            WAX_A[i].load(Ordering::Relaxed),
            WAX_B[i].load(Ordering::Relaxed),
            WAX_POS[i].load(Ordering::Relaxed),
            WAX_T1S[i].load(Ordering::Relaxed),
        ];
        for (k, v) in rec.into_iter().enumerate() {
            push_hex_u16(tx, v);
            push_blocking(tx, if r == 3 && k == 3 { b'\r' } else { b' ' });
        }
    }
    push_blocking(tx, b'\n');
}

/// The `X` dump band: snapshot head → header → 256 lines (raw ring
/// order; host reorders from `head=`) → footer. Nothing is frozen —
/// the rings are CPU-written and stay live (band doc above).
fn wax_dump(sched: &Sched, duty: &Duty, tx: &mut UartTxWriter) {
    let head = WAX_HEAD.load(Ordering::Relaxed);
    let ci = sched.commutation_interval.load(Ordering::Relaxed);
    let arr = duty.tim1_arr.load(Ordering::Relaxed);
    let _ = write!(
        BlockingFmt { tx: &mut *tx },
        "WX n={} head={} ci={} arr={}\r\n",
        WAX_N,
        head,
        ci,
        arr
    );
    for line in 0..(WAX_N / 4) {
        wax_line(tx, line * 4);
    }
    let _ = write!(BlockingFmt { tx: &mut *tx }, "WX END\r\n");
}

// ===============================================================
// Per-ISR duration histogram dump (`H` key — see the ISR_HIST statics
// for the bin law). Small (~200 B ASCII), but routed through
// BlockingFmt like the other dumps so no counts drop if the ring is
// momentarily full. Format (the byte-comparison contract vs rm32):
//   HG shift=8 nbins=16\r\n
//   TIM6  <16 decimal counts, space-separated>\r\n
//   TIM16 <16 counts>\r\n
//   COMP  <16 counts>\r\n
//   HG END\r\n
// ===============================================================
fn hist_dump(tx: &mut UartTxWriter) {
    let mut w = BlockingFmt { tx };
    let _ = write!(w, "HG shift={} nbins={}\r\n", HIST_SHIFT, HIST_NBINS);
    for (isr, name) in [
        (HIST_TIM6, "TIM6"),
        (HIST_TIM16, "TIM16"),
        (HIST_COMP, "COMP"),
    ] {
        let _ = write!(w, "{}", name);
        for bin in &ISR_HIST[isr] {
            let _ = write!(w, " {}", bin.load(Ordering::Relaxed));
        }
        let _ = write!(w, "\r\n");
    }
    let _ = write!(w, "HG END\r\n");
}

// ===============================================================
#[entry]
fn main() -> ! {
    static mut TX_RING: [u8; TX_RING_LEN] = [0; TX_RING_LEN];

    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        mut cp,
        clocks,
        mut ahb2,
        mut apb2,
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    rprintln!(
        "am32_clone: sysclk={} pclk1={}",
        clocks.sysclk().raw(),
        clocks.pclk1().raw()
    );

    // DWT cycle counter for observer timestamps (no periodic ISR).
    let _syst = cp.SYST;
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    unsafe { core::ptr::write_volatile(0xE000_1004 as *mut u32, 0) };

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // USART2 RX on PA2 via CR2.SWAP: AF7 open-drain + pull-up.
    let mut rx2 = gpioa.pa2.into_alternate_open_drain::<7>(
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
    );
    rx2.internal_pull_up(&mut gpioa.pupdr, true);

    // ADC sense pins: PA3 (IN8 current), PA6 (IN11 vbat).
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let _sense = SenseAdc::new(
        dp.ADC1,
        dp.ADC_COMMON,
        pa3,
        pa6,
        &mut ahb2,
        &mut ccipr,
        clocks,
    );

    // Motor PWM pins → TIM1 AF1.
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

    // USART1 TX (PB6, half-duplex init then push-pull) — the trace/telemetry link.
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
    minz::uart_tx::usart1_tx_push_pull_pb6();

    // COMP2 BEMF sense.
    let pb4 = gpiob.pb4.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pb7 = gpiob.pb7.into_analog(&mut gpiob.moder, &mut gpiob.pupdr);
    let pa5 = gpioa.pa5.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa4 = gpioa.pa4.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    comp2::init(pb4, pb7, pa5, pa4, &mut apb2);
    comp2::configure_exti_both_edges();

    // TIM1 motor PWM. TIM1.UIE stays OFF (AM32 zero-CPU-at-carrier); the
    // TIM1_UP_TIM16 vector belongs solely to TIM16 (the COM timer).
    tim1_motor_pwm::init(dp.TIM1, &mut apb2);

    // Injected ADC (A/B/current/vbat) via TIM1 TRGO2 — observer only.
    adc_sync::start(adc_sync::SAMPLE_TICKS);

    // AM32 timers: INTERVAL_TIMER=TIM2, COM_TIMER=TIM16, tenKhz=TIM6.
    interval_timer_init();
    com_timer_init();
    minz::tim6_loop::init();

    // IWDG — the guard that survives the MCU (bench-safety kill #3).
    minz::iwdg::start_1s();

    // USART2 receiver — DMA1_CH6 circular, main polls (usart2_rx::pop).
    minz::usart2_rx::init_pa2_rx(dp.USART2, clocks.pclk1().raw(), BAUD);

    // Boot: energize step (AM32 init `comStep(2)` main.c:2087) at duty 0
    // so the polling startup has a driven pair + a matching comp mux, but
    // no current (duty 0 = low-side hold). CURRENT_STEP stays 1.
    tim1_motor_pwm::set_roles_for_step(1);
    comp2::am32_change_comp_input(1);
    tim1_motor_pwm::set_duty(0);
    comp2::set_exti_enabled(false); // masked until interrupt mode engages

    writeln!(
        &mut tx,
        "\r\n=== am32_clone (UART_DUTY_MODE + ZC_TRACE, {} baud) ===\r",
        BAUD
    )
    .ok();
    writeln!(
        &mut tx,
        "throttle: '<pct>\\n' (0..100) | 's'/'w' stop | 'Z' trace | 'i' info | 'b' bb | 'G' gecko | 'X' wax | 'H' hist | din -/+ '[' ']' | dout -/+ ';' '\r"
    )
    .ok();

    let mut tx_writer = UartTxWriter::new(tx, TX_RING);

    // Priorities: COMP=0, TIM1_UP_TIM16(=COM)=0, TIM6=3
    // (peripherals.c:450,491 + directive). <<4 IPR encoding via priority.rs.
    // USART2 has no vector anymore — RX is DMA-circular (usart2_rx).
    unsafe {
        priority::set_prigroup_preempt4_sub0();
        priority::set_irq_prio(Interrupt::COMP, 0);
        priority::set_irq_prio(Interrupt::TIM1_UP_TIM16, 0);
        priority::set_irq_prio(Interrupt::TIM6_DACUNDER, 3);
        NVIC::unmask(Interrupt::COMP);
        NVIC::unmask(Interrupt::TIM1_UP_TIM16);
        NVIC::unmask(Interrupt::TIM6_DACUNDER);
    }

    // panic::ensure_rtt() (in init) left PRIMASK=1 — re-enable IRQs.
    unsafe { cortex_m::interrupt::enable() };

    main_entry(&mut tx_writer)
}

// ===============================================================
// THE VECTOR TABLE — every #[interrupt] trampoline, together at the
// file's end. Trampolines are the ONLY functions that name the
// static instances; each servicing body (minz_core::am32_isr) takes
// its world as parameters.
// ===============================================================

/// COMP (priority 0) — the ZC chain.
#[interrupt]
fn COMP() {
    let s = cortex_m::peripheral::DWT::cycle_count(); // hist: time whole pass
    COMP_ENTRIES.fetch_add(1, Ordering::Relaxed); // camp-storm probe
    let mut motor = motor();
    comp_isr(&SCHED, &DRIVE, &mut motor, &observer());
    hist_record(HIST_COMP, s);
}

/// TIM16 wrap on the shared vector (priority 0) — the COM tick.
#[interrupt]
fn TIM1_UP_TIM16() {
    let s = cortex_m::peripheral::DWT::cycle_count(); // hist: time whole pass
    let mut motor = motor();
    tim1_up_tim16_isr(&SCHED, &DRIVE, &ZCT, &DUTY, &mut motor, &observer());
    hist_record(HIST_TIM16, s);
    // Drop-proof late-window counter (jitter head-to-head vs rm32): a
    // commutation whose measured ZC-to-ZC interval ran >=1.5x the
    // running average = a "late window". Counter, not a stream — reads
    // clean at 100% where the trace batches. THIS_ZC/AVERAGE_INTERVAL
    // are 0.5 µs ticks; guard avg>20 so it only counts while locked.
    let tz = THIS_ZC.load(Ordering::Relaxed) as u32;
    let avg = AVERAGE_INTERVAL.load(Ordering::Relaxed);
    if avg > 20 && tz > avg + (avg >> 1) {
        LATE_WINDOWS.fetch_add(1, Ordering::Relaxed);
    }
}

/// TIM6 19.6 kHz (priority 3) — tenKhzRoutine.
#[interrupt]
fn TIM6_DACUNDER() {
    let s = cortex_m::peripheral::DWT::cycle_count(); // hist: time whole pass
    wax_tick(); // WAXWING ring write (observer-only, like COMP_ENTRIES)
    let mut motor = motor();
    tim6_dacunder_isr(&SCHED, &DRIVE, &DUTY, &BENCH, &ZCT, &mut motor, &observer());
    hist_record(HIST_TIM6, s);
    // ISR-delay injection rig (bench causal test) — placed AFTER the real
    // tick body AND after `hist_record`, so the histogram still measures
    // the tick's genuine own-work and the injected delay is a separate,
    // deliberate perturbation (not folded into the timing it measures).
    // OUT-free first (COMP can preempt it while it runs), then IN-free
    // inside a critical section (masks COMP for its whole span — the
    // PRIMASK-overlap variable under test). Both default 0 = inert; each
    // hard-capped so a fat-finger can't wedge the tick under the IWDG.
    let d_out = DELAY_OUT_FREE_CYC.load(Ordering::Relaxed);
    if d_out > 0 {
        cortex_m::asm::delay(d_out.min(DELAY_CAP_CYC));
    }
    let d_in = DELAY_IN_FREE_CYC.load(Ordering::Relaxed);
    if d_in > 0 {
        cortex_m::interrupt::free(|_| cortex_m::asm::delay(d_in.min(DELAY_CAP_CYC)));
    }
}
