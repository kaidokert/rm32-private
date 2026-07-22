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
//!   - three bench-safety KILLS: hard overcurrent, absolute vbat floor,
//!     IWDG. None modulate the loop; they only stop it.
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

use core::cell::RefCell;
use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use cortex_m::interrupt::{Mutex, free};
use cortex_m::peripheral::{DWT, NVIC};
use cortex_m_rt::entry;

use minz::adc_sync;
use minz::am32_timers::{
    com_clear_flag, com_set_arr, com_timer_init, disable_com_timer_int, interval_cnt,
    interval_timer_init, set_and_enable_com_int, set_interval_cnt,
};
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::comp2::{self, ObservedPhase};
use minz::current_adc::SenseAdc;
use minz::hal::pac::interrupt;
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial};
use minz::hal::stm32;
use minz::hal::stm32::Interrupt;
use minz::priority;
use minz::tim1_motor_pwm::{self, max_duty};
use minz::uart_tx::{TX_RING_LEN, UartTxWriter};

use minz_core::am32::{self, Am32Intervals, UartCmd, UartDuty};
use minz_core::blackbox::{self, BlackBox, EV_ACC, EV_DSY, EV_REF, Event};
use minz_core::drive::edges_for;

use rtt_target::rprintln;

// ===============================================================
// AM32 constants — factory-default EEPROM, VIMDRONES_L431.
// (Values that depend on the flashed EEPROM image are flagged
//  BENCH-VERIFY; the build is correct regardless of their tuning.)
// ===============================================================

/// TIM1 base ARR = 24 kHz carrier. AM32 targets.h:5335
/// `TIM1_AUTORELOAD = CPU_FREQUENCY_MHZ*1e6/NOMINAL_PWM - 1 = 3332`.
const TIMER1_MAX_ARR: u16 = minz::TIM1_AUTORELOAD; // 3332
/// AM32's throttle/duty domain is 0..2000 (main.c throughout).
const DUTY_FULL: u16 = 2000;

/// tenKhzRoutine cadence. AM32 targets.h:177 `LOOP_FREQUENCY_HZ 20000`.
const LOOP_FREQUENCY_HZ: u32 = 20_000;

/// AM32 targets.h:5300 `TARGET_MIN_BEMF_COUNTS 3`.
const TARGET_MIN_BEMF_COUNTS: u16 = 3;
/// AM32 main.c:550 `bad_count_threshold = CPU_FREQUENCY_MHZ/24` = 80/24.
const BAD_COUNT_THRESHOLD: u16 = 80 / 24; // = 3

/// Polling<->interrupt changeover, 0.5 µs ticks. AM32 targets.h:5344
/// `POLLING_MODE_THRESHOLD 2000`; non-bi-dir keeps it whole
/// (loadEEpromSettings main.c:795).
const POLLING_MODE_CHANGEOVER: u32 = 2000;

/// Constant 15° advance: AUTO_ADVANCE off, advance_level default → 16
/// (loadEEpromSettings main.c:628-630); `advance = ci*16>>6 = ci/4`.
const TEMP_ADVANCE: u32 = 16;

/// Startup interval seed, 0.5 µs ticks. AM32 startMotor main.c:954.
const STARTUP_INTERVAL_TICKS: u32 = 10_000;
/// AM32 main.c:575 global init `commutation_interval = 12500`.
const INIT_INTERVAL_TICKS: u32 = 12_500;

// --- Duty pipeline defaults (loadEEpromSettings main.c:610-797) ---
// BENCH-VERIFY: these three resolve from the flashed EEPROM bytes,
// which are not in the source tree. Values below follow the code path
// for a fresh/default image (driving_brake_strength defaults to 10 at
// main.c:696-698, so the dead-time-override block main.c:700-721 is
// skipped) with the documented AM32-configurator factory bytes.
/// main.c:428 `minimum_duty_cycle = DEAD_TIME` (VIMDRONES DEAD_TIME=45).
const MINIMUM_DUTY_CYCLE: u16 = 45;
/// main.c:653 `min_startup_duty = minimum_duty_cycle + startup_power`
/// (startup_power factory ≈ 100).
const MIN_STARTUP_DUTY: u16 = MINIMUM_DUTY_CYCLE + 100; // 145
/// main.c:657 `startup_max_duty_cycle = minimum_duty_cycle + 400`.
const STARTUP_MAX_DUTY_CYCLE: u16 = MINIMUM_DUTY_CYCLE + 400; // 445

// Ramp rates (main.c:1746-1754) + low-rpm duty ceiling map
// (main.c:436-439) now live in `minz_core::am32` (with their AM32
// citations); the pipeline helpers there consume them.

/// main.c:390 `low_rpm_throttle_limit = 1` (on by default).
const LOW_RPM_THROTTLE_LIMIT: bool = true;

/// variable_pwm carrier ride (main.c:2192-2195). BENCH-VERIFY: the
/// factory `eepromBuffer.variable_pwm` byte is EEPROM-resident; the
/// mode-1 path is transliterated and enabled here.
const VARIABLE_PWM: u8 = 1;

/// bemf timeout re-kick threshold, 0.5 µs ticks. main.c:2495
/// `INTERVAL_TIMER_COUNT > 45000`.
const BEMF_TIMEOUT_TICKS: u32 = 45_000;

/// UART deadman: no command for 3 s → throttle 0. main.c:1425.
const UART_DEADMAN_LIMIT: u32 = 3 * LOOP_FREQUENCY_HZ; // 60000 ticks

/// 2 Mbaud link (both directions) — the minz-rig baud (AM32 fork
/// uart_duty_init main.c:1363, motor_tester2 parity).
const BAUD: u32 = 2_000_000;

// --- Bench-safety kill thresholds (observer ADC; kills only) ---
/// ~85 ms current average over raw injected ch8 counts.
const OC_KILL_RAW_AVG: u32 = 205;
const OC_WINDOW_TICKS: u32 = 1700; // ≈ 85 ms at 20 kHz
/// absolute vbat floor, raw injected ch11 counts (~5.95 V).
const VBAT_ABS_FLOOR_RAW: u16 = 793;

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

/// Latched fatal kill (bench-safety). Main prints and holds off.
static KILLED: AtomicBool = AtomicBool::new(false);
static KILL_REASON: AtomicU16 = AtomicU16::new(0); // 1=OC, 2=vbat
static I_RAW: AtomicU16 = AtomicU16::new(0);
static VBAT_RAW: AtomicU16 = AtomicU16::new(0);

/// Stop request from the RX parser (bench pct=0 / 'w'): honored in main.
static STOP_REQ: AtomicBool = AtomicBool::new(false);
static DUMP_REQ: AtomicBool = AtomicBool::new(false);
static INFO_REQ: AtomicBool = AtomicBool::new(false);
static ZCT_STREAM_ON: AtomicBool = AtomicBool::new(true);

/// Which phase floats in each sector (minz textbook convention, matches
/// `set_roles_for_step`): sector 0/3 → C, 1/4 → B, 2/5 → A.
const SECTOR_FLOAT_PHASE: [ObservedPhase; 6] = [
    ObservedPhase::C,
    ObservedPhase::B,
    ObservedPhase::A,
    ObservedPhase::C,
    ObservedPhase::B,
    ObservedPhase::A,
];

// ===============================================================
// Black box (observer). minz_core ring behind a critical-section lock.
// ===============================================================
static BB: Mutex<RefCell<BlackBox>> = Mutex::new(RefCell::new(BlackBox::new()));

#[inline]
fn now_10us() -> u16 {
    (DWT::cycle_count() / 800) as u16
}

#[inline]
fn bb_record(ty: u8, sector: u8, data: u16) {
    let t = now_10us();
    free(|cs| BB.borrow(cs).borrow_mut().record(Event { t, ty, sector, data }));
}

// ===============================================================
// INTERVAL_TIMER = TIM2, COM_TIMER = TIM16 — the register wrappers
// (interval_timer_init / interval_cnt / set_interval_cnt /
// com_timer_init / set_and_enable_com_int / disable_com_timer_int /
// com_set_arr / com_clear_flag) live in `minz::am32_timers`.
// ===============================================================

// ===============================================================
// comparator.c transliteration.
// ===============================================================

/// maskPhaseInterrupts (comparator.c:9-12): clear EXTI IMR + clear flag.
#[inline]
fn mask_phase_interrupts() {
    comp2::set_exti_enabled(false);
    comp2::clear_pending();
}

/// enableCompInterrupts (comparator.c:14-16): set EXTI IMR, keep pending.
#[inline]
fn enable_comp_interrupts() {
    comp2::unmask_keep_pending();
}

/// changeCompInput (comparator.c:18-35): mux the floating phase and the
/// single expected-direction EXTI edge for the sector. edges_for(3,·) is
/// the minz-polarity-correct form of AM32's `if(rising)` edge select.
#[inline]
fn change_comp_input(sector: usize) {
    comp2::set_inm(SECTOR_FLOAT_PHASE[sector]);
    let (re, fe) = edges_for(3, sector as u8);
    comp2::set_exti_edges(re, fe);
}

// ===============================================================
// map()/getAbsDif() (functions.c) now live in `minz_core::am32`.
// ===============================================================

// ===============================================================
// ZC_TRACE ring — 15-byte records (main.c:1536-1567), 5B A9 sync.
// Producer: COM ISR (PeriodElapsedCallback) + polling zcfoundroutine.
// Consumer: main loop → USART1 DMA writer. Guarded with `free` because
// two ISR contexts (prio 0 COM, prio 3 TIM6) can both push.
// ===============================================================
const ZCT_REC: usize = 15;
const ZCT_N: usize = 32;
static ZCT_RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static ZCT_HEAD: AtomicUsize = AtomicUsize::new(0);
static ZCT_TAIL: AtomicUsize = AtomicUsize::new(0);
static ZCT_DROP: AtomicU32 = AtomicU32::new(0);

//// BATCH DECIMATION (operator directive 2026-07-20): above the wire's
/// bandwidth the trace switches to 50-commutations-on / 50-off batch
/// mode instead of losing records to saturation aliasing. Batches
/// (not 1-in-N) preserve CONSECUTIVE records so rolling-mean
/// excursion metrics stay valid inside each batch (~44 usable
/// samples per 50). Budget: 2 Mbaud ≈ 13.3k records/s; full rate
/// fits down to ci ≈ 150 ticks (75 µs); batching engages below
/// ci = 200 ticks (100 µs → 10k/s full → 5k/s batched, comfortable).
/// Mode is re-evaluated only at batch boundaries (no mid-batch
/// flapping); flag bit6 marks batched-mode records so the host can
/// segment (decoders mask bits0-2|7 — bit6 is backward-compatible).
static ZCT_COMM_N: AtomicU32 = AtomicU32::new(0);
static ZCT_BATCHING: AtomicBool = AtomicBool::new(false);

// ===============================================================
// USART2 RX byte ring (ISR producer, main consumer).
// ===============================================================
const RX_N: usize = 256;
static RX_RING: [AtomicU16; RX_N] = [const { AtomicU16::new(0) }; RX_N];
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

/// The coupled RX-ring state as one reference-struct (SPSC: the
/// USART2 ISR is the sole producer via `push`, main is the sole
/// consumer via `pop` — same relaxed-ordering semantics as the
/// loose statics it groups).
struct RxRing<'a> {
    ring: &'a [AtomicU16; RX_N],
    head: &'a AtomicUsize,
    tail: &'a AtomicUsize,
}

static RX: RxRing<'static> = RxRing {
    ring: &RX_RING,
    head: &RX_HEAD,
    tail: &RX_TAIL,
};

impl RxRing<'_> {
    /// Producer side (ISR): enqueue one byte; full ring drops.
    #[inline]
    fn push(&self, c: u16) {
        let h = self.head.load(Ordering::Relaxed);
        let nx = (h + 1) % RX_N;
        if nx != self.tail.load(Ordering::Relaxed) {
            self.ring[h].store(c, Ordering::Relaxed);
            self.head.store(nx, Ordering::Relaxed);
        }
    }

    /// Consumer side (main): dequeue one byte if available.
    #[inline]
    fn pop(&self) -> Option<u8> {
        let t = self.tail.load(Ordering::Relaxed);
        if t == self.head.load(Ordering::Relaxed) {
            return None;
        }
        let c = self.ring[t].load(Ordering::Relaxed) as u8;
        self.tail.store((t + 1) % RX_N, Ordering::Relaxed);
        Some(c)
    }
}

// ===============================================================
// Cohesion clusters — each struct bundles the loose statics above
// that are written/read together, holding ONLY `&'static Atomic*`
// refs (storage is unchanged). Every servicing fn takes the clusters
// it touches as `&` params; the ONLY places that name the SCHED /
// DRIVE / DUTY / BENCH / ZCT instances are the ISR trampolines,
// main_entry's top (local wiring), and these definitions.
// ===============================================================

/// Scheduling / ZC-estimate cluster (all 0.5 µs units).
struct Sched<'a> {
    commutation_interval: &'a AtomicU32,
    interval_hist: &'a [AtomicU32; 6],
    average_interval: &'a AtomicU32,
    last_average_interval: &'a AtomicU32,
    last_zc: &'a AtomicU16,
    this_zc: &'a AtomicU16,
    wait_time: &'a AtomicU16,
}

/// Commutation / run-state cluster + its event counters.
struct Drive<'a> {
    current_step: &'a AtomicU16,
    rising: &'a AtomicBool,
    old_routine: &'a AtomicBool,
    running: &'a AtomicBool,
    zcfound: &'a AtomicBool,
    bemf_counter: &'a AtomicU16,
    min_bemf_up: &'a AtomicU16,
    min_bemf_down: &'a AtomicU16,
    zero_crosses: &'a AtomicU32,
    filter_level: &'a AtomicU16,
    bad_count: &'a AtomicU16,
    desync_check: &'a AtomicBool,
    desync_happened: &'a AtomicU32,
    bemf_timeout_happened: &'a AtomicU32,
    tenkhz_counter: &'a AtomicU16,
    zcfr_guard_hits: &'a AtomicU32,
}

/// Duty pipeline cluster (0..2000 domain) + the latched kill.
struct Duty<'a> {
    input: &'a AtomicU16,
    adjusted_input: &'a AtomicU16,
    uart_duty_input: &'a AtomicU16,
    duty_cycle_setpoint: &'a AtomicU16,
    duty_cycle: &'a AtomicU16,
    last_duty_cycle: &'a AtomicU16,
    duty_cycle_maximum: &'a AtomicU16,
    ramp_count: &'a AtomicU16,
    killed: &'a AtomicBool,
    kill_reason: &'a AtomicU16,
}

/// Observer / bench cluster (ADC harvest, OC accumulator, req flags).
struct Bench<'a> {
    uart_deadman_ticks: &'a AtomicU32,
    i_raw: &'a AtomicU16,
    vbat_raw: &'a AtomicU16,
    oc_acc: &'a AtomicU32,
    oc_cnt: &'a AtomicU32,
    stop_req: &'a AtomicBool,
    dump_req: &'a AtomicBool,
    info_req: &'a AtomicBool,
    zct_stream_on: &'a AtomicBool,
}

/// ZC_TRACE ring cluster — 15-byte records + batch-decimation state.
struct ZctTrace<'a> {
    ring: &'a [[AtomicU16; ZCT_REC]; ZCT_N],
    head: &'a AtomicUsize,
    tail: &'a AtomicUsize,
    drop: &'a AtomicU32,
    comm_n: &'a AtomicU32,
    batching: &'a AtomicBool,
}

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
};

static BENCH: Bench<'static> = Bench {
    uart_deadman_ticks: &UART_DEADMAN_TICKS,
    i_raw: &I_RAW,
    vbat_raw: &VBAT_RAW,
    oc_acc: &OC_ACC,
    oc_cnt: &OC_CNT,
    stop_req: &STOP_REQ,
    dump_req: &DUMP_REQ,
    info_req: &INFO_REQ,
    zct_stream_on: &ZCT_STREAM_ON,
};

static ZCT: ZctTrace<'static> = ZctTrace {
    ring: &ZCT_RING,
    head: &ZCT_HEAD,
    tail: &ZCT_TAIL,
    drop: &ZCT_DROP,
    comm_n: &ZCT_COMM_N,
    batching: &ZCT_BATCHING,
};

impl Sched<'_> {
    /// The 6-slot `commutation_intervals[]` history over the firmware
    /// statics (minz_core owns the push/sum/e_com_time logic) —
    /// main.c:441,887.
    #[inline]
    fn intervals(&self) -> Am32Intervals<'_> {
        Am32Intervals::new(self.interval_hist)
    }
}

impl ZctTrace<'_> {
    // zct_write (main.c:1542-1567): one canonical row per commutation.
    #[inline]
    fn write(&self, sched: &Sched, drive: &Drive, duty: &Duty) {
        // Batch-decimation gate (am32::zct_batch_gate). Single CI snapshot
        // used for both the gate and the packed record (the value is stable
        // within the calling ISR — this was two separate loads before).
        let n = self.comm_n.fetch_add(1, Ordering::Relaxed);
        let ci_ticks = sched.commutation_interval.load(Ordering::Relaxed);
        let (record, batching) =
            am32::zct_batch_gate(n, ci_ticks, self.batching.load(Ordering::Relaxed));
        self.batching.store(batching, Ordering::Relaxed);
        if !record {
            return; // the skipped half-duty of the batch cycle
        }
        let rec: [u8; ZCT_REC] = am32::zct_pack(
            drive.current_step.load(Ordering::Relaxed) as u8,
            drive.old_routine.load(Ordering::Relaxed),
            batching,
            sched.this_zc.load(Ordering::Relaxed),
            ci_ticks as u16,
            sched.wait_time.load(Ordering::Relaxed),
            duty.duty_cycle.load(Ordering::Relaxed),
            drive.tenkhz_counter.load(Ordering::Relaxed),
            sched.average_interval.load(Ordering::Relaxed) as u16,
        );
        free(|_| {
            let h = self.head.load(Ordering::Relaxed);
            let nx = (h + 1) % ZCT_N;
            if nx == self.tail.load(Ordering::Relaxed) {
                self.drop.fetch_add(1, Ordering::Relaxed);
                self.tail
                    .store((self.tail.load(Ordering::Relaxed) + 1) % ZCT_N, Ordering::Relaxed);
            }
            for (i, b) in rec.iter().enumerate() {
                self.ring[h][i].store(*b as u16, Ordering::Relaxed);
            }
            self.head.store(nx, Ordering::Relaxed);
        });
    }

    /// Drain up to 3 ZC_TRACE records into the USART1 DMA ring (main.c:2347-2357).
    #[inline]
    fn drain(&self, tx: &mut UartTxWriter) {
        let mut nrec = 0;
        while self.tail.load(Ordering::Relaxed) != self.head.load(Ordering::Relaxed) && nrec < 3 {
            let t = self.tail.load(Ordering::Relaxed);
            for i in 0..ZCT_REC {
                let _ = tx.push(self.ring[t][i].load(Ordering::Relaxed) as u8);
            }
            self.tail.store((t + 1) % ZCT_N, Ordering::Relaxed);
            nrec += 1;
        }
    }
}

// ===============================================================
// commutate() — main.c:854-894 (forward-only factory path).
// ===============================================================
fn commutate(sched: &Sched, drive: &Drive) {
    // step++ ; if step>6 { step=1; desync_check=1 }  (main.c:856-861)
    let mut step = drive.current_step.load(Ordering::Relaxed);
    step += 1;
    if step > 6 {
        step = 1;
        drive.desync_check.store(true, Ordering::Relaxed);
    }
    // rising = step % 2  (main.c:862)
    let rising = (step & 1) == 1;
    drive.rising.store(rising, Ordering::Relaxed);
    drive.current_step.store(step, Ordering::Relaxed);
    let sector = (step - 1) as usize;

    // comStep(step) (main.c:876) — minz role-only flip; CCRs hold the
    // tick-shaped duty. AM32 wraps this in __disable_irq; set_roles_for_step
    // does its own interrupt::free.
    tim1_motor_pwm::set_roles_for_step(sector as u8);
    // changeCompInput() (main.c:879).
    change_comp_input(sector);

    // if average_interval > polling_mode_changeover+500 → old_routine=1
    // (main.c:881-883).
    if sched.average_interval.load(Ordering::Relaxed) > POLLING_MODE_CHANGEOVER + 500 {
        drive.old_routine.store(true, Ordering::Relaxed);
    }
    // bemfcounter=0; zcfound=0 (main.c:885-886).
    drive.bemf_counter.store(0, Ordering::Relaxed);
    drive.zcfound.store(false, Ordering::Relaxed);
    // commutation_intervals[step-1] = commutation_interval (main.c:887).
    sched.intervals().push(sector, sched.commutation_interval.load(Ordering::Relaxed));

    bb_record(EV_REF, sector as u8, sched.commutation_interval.load(Ordering::Relaxed) as u16);
}

// ===============================================================
// getBemfState() — main.c:817-852 (L431 `!getCompOutputLevel()` branch,
// which equals minz `comp2::value()`). Counts when the level matches the
// direction; a run of bad reads over threshold resets the counter.
// ===============================================================
fn get_bemf_state(drive: &Drive) {
    let cs = comp2::value(); // = !getCompOutputLevel() (main.c:831)
    let rising = drive.rising.load(Ordering::Relaxed);
    // rising: count when current_state; else count when !current_state.
    // Both reduce to `cs == rising` (main.c:833-851).
    let (bemf, bad) = am32::bemf_count_step(
        drive.bemf_counter.load(Ordering::Relaxed),
        drive.bad_count.load(Ordering::Relaxed),
        cs == rising,
        BAD_COUNT_THRESHOLD,
    );
    drive.bemf_counter.store(bemf, Ordering::Relaxed);
    drive.bad_count.store(bad, Ordering::Relaxed);
}

// ===============================================================
// zcfoundroutine() — main.c:1868-1915 (polling mode, blocking).
// ===============================================================
static ZCFR_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

fn zcfoundroutine(sched: &Sched, drive: &Drive, zct: &ZctTrace, duty: &Duty) {
    // thiszctime = INTERVAL_TIMER_COUNT; SET_INTERVAL_TIMER_COUNT(0)
    let thiszc = interval_cnt() as u16; // main.c:1870
    set_interval_cnt(0); // main.c:1871
    sched.this_zc.store(thiszc, Ordering::Relaxed);
    // commutation_interval = (thiszctime + 3*ci)/4  (main.c:1872)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let ci = am32::polling_blend(thiszc as u32, ci_old);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = temp_advance*ci >> 6 ; waitTime = ci/2 - advance  (1873-4)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);

    // while INTERVAL_TIMER_COUNT < waitTime { if zero_crosses<5 break }
    // (main.c:1875-1879). DEVIATION #1: bounded by a spin guard — AM32's
    // loop is unbounded (a wedged INTERVAL_TIMER hangs the 20 kHz ISR
    // forever; AM32 accepts that, the IWDG would reboot). We add a loud
    // 65535-iteration break so the tick can't be captured indefinitely.
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    let mut guard: u32 = 0;
    loop {
        if interval_cnt() >= wait {
            break;
        }
        if zc < 5 {
            break;
        }
        guard += 1;
        if guard > 65_535 {
            drive.zcfr_guard_hits.fetch_add(1, Ordering::Relaxed);
            break;
        }
    }

    com_set_arr(wait as u16); // COM_TIMER->ARR = waitTime (main.c:1884)
    commutate(sched, drive); // main.c:1889
    zct.write(sched, drive, duty); // ZC_TRACE main.c:1891
    drive.bemf_counter.store(0, Ordering::Relaxed); // main.c:1893
    drive.bad_count.store(0, Ordering::Relaxed); // main.c:1894
    drive.zero_crosses.store(zc.saturating_add(1), Ordering::Relaxed); // main.c:1896

    // changeover to interrupt mode (non-stall/non-rc_car path,
    // main.c:1908-1913): commutation_interval < polling_mode_changeover.
    if ci < POLLING_MODE_CHANGEOVER {
        drive.old_routine.store(false, Ordering::Relaxed);
        enable_comp_interrupts();
    }
}

// ===============================================================
// startMotor() — main.c:950-959. Per the directive, comp interrupts are
// NOT enabled here (polling reads level, no EXTI) — the enable happens
// at the polling→interrupt changeover in zcfoundroutine. This is the one
// intentional divergence from AM32's line 958 `enableCompInterrupts()`.
// ===============================================================
fn start_motor(sched: &Sched, drive: &Drive) {
    if !drive.running.load(Ordering::Relaxed) {
        commutate(sched, drive); // main.c:953
        sched.commutation_interval.store(STARTUP_INTERVAL_TICKS, Ordering::Relaxed); // :954
        set_interval_cnt(5000); // SET_INTERVAL_TIMER_COUNT(5000) main.c:955
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.bemf_counter.store(0, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed); // main.c:956
    }
}

// ===============================================================
// Bench-safety kill: float all legs, latch, mask comp, freeze bb.
// ===============================================================
fn safety_kill(drive: &Drive, duty: &Duty, reason: u16) {
    tim1_motor_pwm::all_off();
    drive.running.store(false, Ordering::Relaxed);
    duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
    duty.duty_cycle.store(0, Ordering::Relaxed);
    duty.last_duty_cycle.store(0, Ordering::Relaxed);
    drive.old_routine.store(true, Ordering::Relaxed);
    mask_phase_interrupts();
    disable_com_timer_int();
    duty.kill_reason.store(reason, Ordering::Relaxed);
    duty.killed.store(true, Ordering::Relaxed);
    free(|cs| BB.borrow(cs).borrow_mut().freeze());
}

// ===============================================================
fn main_entry(tx_writer: &mut UartTxWriter) -> ! {
    // Wire the cluster locals once (the only main-context place allowed to
    // name the static instances). Everything below references these.
    let sched = &SCHED;
    let drive = &DRIVE;
    let duty = &DUTY;
    let bench = &BENCH;
    let zct = &ZCT;
    let rx = &RX;

    // Main-context UART parser state (mirrors uart_duty_poll main.c:1367).
    let mut uart = UartDuty::new();
    // ramp_count local-ish (ramp_divider=0 → ramp every tenKhz tick, so
    // main only needs the maps below; ramp lives in the TIM6 ISR).

    loop {
        minz::iwdg::refresh();

        // ---- drain USART2 RX ring → parser (uart_duty_poll) --------
        while let Some(c) = rx.pop() {
            apply_uart_cmd(duty, bench, uart.step(c));
        }

        // ---- honor bench stop request ------------------------------
        if bench.stop_req.swap(false, Ordering::Relaxed) {
            tim1_motor_pwm::all_off();
            drive.running.store(false, Ordering::Relaxed);
            drive.old_routine.store(true, Ordering::Relaxed);
            drive.zero_crosses.store(0, Ordering::Relaxed);
            duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
            duty.duty_cycle.store(0, Ordering::Relaxed);
            duty.last_duty_cycle.store(0, Ordering::Relaxed);
            mask_phase_interrupts();
            disable_com_timer_int();
        }

        // ---- e_com_time (main.c:2159) ------------------------------
        let e_com_time = sched.intervals().e_com_time(); // (sum+4)>>1, 0.5 µs units

        // input = uart_duty_get()  (main.c:1131) then setInput()
        set_input(sched, drive, duty);

        // min_bemf_counts schedule (main.c:2177-2188, non-bi-dir).
        if drive.zero_crosses.load(Ordering::Relaxed) < 5 {
            drive.min_bemf_up.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
            drive.min_bemf_down.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
        } else {
            drive.min_bemf_up.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
            drive.min_bemf_down.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
        }

        // variable_pwm (main.c:2192-2195). mode 1: carrier rides the
        // commutation interval; duty ratio is preserved by the tenKhz
        // `duty*tim1_arr/2000` rescale.
        if VARIABLE_PWM == 1 {
            let ci = sched.commutation_interval.load(Ordering::Relaxed) as i32;
            let arr = am32::map(ci, 96, 200, (TIMER1_MAX_ARR / 2) as i32, TIMER1_MAX_ARR as i32);
            tim1_motor_pwm::set_carrier_arr(arr as u16);
        }

        // average_interval = e_com_time / 3  (main.c:2283)
        let average_interval = if e_com_time > 0 { (e_com_time / 3) as u32 } else { 0 };
        sched.average_interval.store(average_interval, Ordering::Relaxed);

        // desync_check block (main.c:2284-2300) — non-bi-dir subset.
        if drive.desync_check.load(Ordering::Relaxed) && drive.zero_crosses.load(Ordering::Relaxed) > 10 {
            let lai = sched.last_average_interval.load(Ordering::Relaxed);
            if am32::desync_due(lai, average_interval) {
                drive.zero_crosses.store(0, Ordering::Relaxed); // main.c:2286
                drive.desync_happened.fetch_add(1, Ordering::Relaxed); // :2287
                // (!bi_direction && input>47) || commutation_interval>1000 → running=0
                let input = duty.input.load(Ordering::Relaxed);
                let ci = sched.commutation_interval.load(Ordering::Relaxed);
                if input > 47 || ci > 1000 {
                    drive.running.store(false, Ordering::Relaxed);
                }
                drive.old_routine.store(true, Ordering::Relaxed); // :2291
                duty.last_duty_cycle.store(MIN_STARTUP_DUTY / 2, Ordering::Relaxed); // :2295
                bb_record(EV_DSY, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, average_interval as u16);
            }
            drive.desync_check.store(false, Ordering::Relaxed); // :2297
            sched.last_average_interval.store(average_interval, Ordering::Relaxed); // :2299
        }

        // ---- low-rpm duty ceiling + filter_level (main.c:2441-2469) --
        let running = drive.running.load(Ordering::Relaxed);
        let duty_max = am32::low_rpm_duty_ceiling(e_com_time, running, LOW_RPM_THROTTLE_LIMIT);
        duty.duty_cycle_maximum.store(duty_max, Ordering::Relaxed);

        let zc = drive.zero_crosses.load(Ordering::Relaxed);
        let ci = sched.commutation_interval.load(Ordering::Relaxed);
        let mut filter = if zc < 100 && ci > 500 {
            12
        } else {
            am32::map(average_interval as i32, 100, 500, 3, 12)
        };
        if ci < 50 {
            filter = 2;
        }
        drive.filter_level.store(filter as u16, Ordering::Relaxed);

        // ---- bemf timeout leniency resets (main.c:2261-2273) --------
        let adj = duty.adjusted_input.load(Ordering::Relaxed);
        if zc > 1000 || adj == 0 {
            drive.bemf_timeout_happened.store(0, Ordering::Relaxed);
        }
        if zc > 100 && adj < 200 {
            drive.bemf_timeout_happened.store(0, Ordering::Relaxed);
        }

        // ---- bemf timeout re-kick (main.c:2495-2509) ----------------
        if interval_cnt() > BEMF_TIMEOUT_TICKS && running {
            drive.bemf_timeout_happened.fetch_add(1, Ordering::Relaxed);
            mask_phase_interrupts();
            drive.old_routine.store(true, Ordering::Relaxed);
            if duty.input.load(Ordering::Relaxed) < 48 {
                drive.running.store(false, Ordering::Relaxed);
                sched.commutation_interval.store(5000, Ordering::Relaxed);
            }
            drive.zero_crosses.store(0, Ordering::Relaxed);
            zcfoundroutine(sched, drive, zct, duty);
        }

        // ---- telemetry: drain ZC_TRACE ring → USART1 DMA writer -----
        if bench.zct_stream_on.load(Ordering::Relaxed) {
            zct.drain(tx_writer);
        }
        tx_writer.service();

        // ---- on-demand info / bb dump / kill notice -----------------
        if bench.info_req.swap(false, Ordering::Relaxed) {
            print_info(sched, drive, duty, bench, zct, tx_writer);
        }
        if bench.dump_req.swap(false, Ordering::Relaxed) {
            dump_bb(tx_writer);
        }
        if duty.killed.swap(false, Ordering::Relaxed) {
            let reason = duty.kill_reason.load(Ordering::Relaxed);
            let _ = write!(
                tx_writer,
                "!! KILL reason={} (1=OC 2=vbat) iraw={} vbat={}\r\n",
                reason,
                bench.i_raw.load(Ordering::Relaxed),
                bench.vbat_raw.load(Ordering::Relaxed),
            );
            dump_bb(tx_writer);
        }
    }
}

/// setInput() duty-setpoint block — main.c:1180-1326 factory subset
/// (armed always true; no sine, no brake, no current limit).
fn set_input(sched: &Sched, drive: &Drive, duty: &Duty) {
    // input = uart_duty_get()  (main.c:1131,1423-1431).
    let input = duty.uart_duty_input.load(Ordering::Relaxed);
    duty.input.store(input, Ordering::Relaxed);

    let running = drive.running.load(Ordering::Relaxed);
    if input >= 47 {
        // main.c:1182-1196
        if !running {
            tim1_motor_pwm::all_off(); // main.c:1184
            if !drive.old_routine.load(Ordering::Relaxed) {
                start_motor(sched, drive); // main.c:1185-1187
            }
            drive.running.store(true, Ordering::Relaxed); // main.c:1188
            duty.last_duty_cycle.store(MIN_STARTUP_DUTY, Ordering::Relaxed); // :1189
        }
    } else {
        // input < 47 (main.c:1203-1300, comp_pwm subset)
        if !running {
            drive.old_routine.store(true, Ordering::Relaxed);
            drive.zero_crosses.store(0, Ordering::Relaxed);
            drive.bad_count.store(0, Ordering::Relaxed);
            tim1_motor_pwm::all_off();
        }
    }

    // duty_cycle_setpoint = map(input, 47, 2047, minimum_duty_cycle, 2000)
    // when armed, else 0 (am32::duty_setpoint). Stored here (mirrors the
    // two per-branch stores main.c:1194/1206) then re-clamped below —
    // the intermediate value is what a preempting TIM6 tick reads.
    duty.duty_cycle_setpoint.store(
        am32::duty_setpoint(input, MINIMUM_DUTY_CYCLE, DUTY_FULL),
        Ordering::Relaxed,
    );
    let mut sp = duty.duty_cycle_setpoint.load(Ordering::Relaxed);

    // startup clamp (main.c:1302-1314), non-bi-dir (30 >> 0 = 30).
    if input >= 47 && drive.zero_crosses.load(Ordering::Relaxed) < 30 {
        if sp < MIN_STARTUP_DUTY {
            sp = MIN_STARTUP_DUTY;
        }
        if sp > STARTUP_MAX_DUTY_CYCLE {
            sp = STARTUP_MAX_DUTY_CYCLE;
        }
    }
    let dmax = duty.duty_cycle_maximum.load(Ordering::Relaxed);
    if sp > dmax {
        sp = dmax;
    }
    duty.duty_cycle_setpoint.store(sp, Ordering::Relaxed);
}

/// Apply a decoded UART command (am32::UartDuty parses; this performs
/// the side effects) — uart_duty_poll main.c:1367-1416 + bench keys.
fn apply_uart_cmd(duty: &Duty, bench: &Bench, cmd: Option<UartCmd>) {
    match cmd {
        // main.c:1376-1381,1397-1398 stop ('s'/'w' and a committed 0):
        // zero throttle + adjusted + deadman and request the bench stop.
        Some(UartCmd::Stop) => {
            duty.uart_duty_input.store(0, Ordering::Relaxed);
            duty.adjusted_input.store(0, Ordering::Relaxed);
            bench.uart_deadman_ticks.store(0, Ordering::Relaxed);
            bench.stop_req.store(true, Ordering::Relaxed);
        }
        Some(UartCmd::SetThrottle(inn)) => {
            duty.uart_duty_input.store(inn, Ordering::Relaxed);
            // adjusted_input mirror (main.c:1410)
            duty.adjusted_input.store(if inn <= 48 { 0 } else { inn }, Ordering::Relaxed);
            bench.uart_deadman_ticks.store(0, Ordering::Relaxed);
        }
        Some(UartCmd::TraceToggle) => {
            let on = !bench.zct_stream_on.load(Ordering::Relaxed);
            bench.zct_stream_on.store(on, Ordering::Relaxed);
        }
        Some(UartCmd::Info) => bench.info_req.store(true, Ordering::Relaxed),
        Some(UartCmd::BbDump) => bench.dump_req.store(true, Ordering::Relaxed),
        None => {}
    }
}

fn print_info(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace, tx: &mut UartTxWriter) {
    let _ = write!(
        tx,
        "i step={} old={} run={} ci={} avg={} zc={} duty={} iraw={} vbat={} drop={} guard={}\r\n",
        drive.current_step.load(Ordering::Relaxed),
        drive.old_routine.load(Ordering::Relaxed) as u8,
        drive.running.load(Ordering::Relaxed) as u8,
        sched.commutation_interval.load(Ordering::Relaxed),
        sched.average_interval.load(Ordering::Relaxed),
        drive.zero_crosses.load(Ordering::Relaxed),
        duty.duty_cycle.load(Ordering::Relaxed),
        bench.i_raw.load(Ordering::Relaxed),
        bench.vbat_raw.load(Ordering::Relaxed),
        zct.drop.load(Ordering::Relaxed),
        drive.zcfr_guard_hits.load(Ordering::Relaxed),
    );
}

fn dump_bb(tx: &mut UartTxWriter) {
    free(|cs| {
        let bb = BB.borrow(cs).borrow();
        blackbox::format_dump(bb.replay().copied(), |b: &[u8]| {
            for &byte in b {
                let _ = tx.push(byte);
            }
        });
    });
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

    rprintln!("am32_clone: sysclk={} pclk1={}", clocks.sysclk().raw(), clocks.pclk1().raw());

    // DWT cycle counter for observer timestamps (no periodic ISR).
    let _syst = cp.SYST;
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    unsafe { core::ptr::write_volatile(0xE000_1004 as *mut u32, 0) };

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // USART2 RX on PA2 via CR2.SWAP: AF7 open-drain + pull-up.
    let mut rx2 = gpioa
        .pa2
        .into_alternate_open_drain::<7>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    rx2.internal_pull_up(&mut gpioa.pupdr, true);

    // ADC sense pins: PA3 (IN8 current), PA6 (IN11 vbat).
    let pa3 = gpioa.pa3.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let pa6 = gpioa.pa6.into_analog(&mut gpioa.moder, &mut gpioa.pupdr);
    let _sense = SenseAdc::new(dp.ADC1, dp.ADC_COMMON, pa3, pa6, &mut ahb2, &mut ccipr, clocks);

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
    let mut usart_tx = gpiob
        .pb6
        .into_alternate_open_drain::<7>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    usart_tx.internal_pull_up(&mut gpiob.pupdr, true);
    let serial = Serial::usart1(dp.USART1, (usart_tx,), Config::default().baudrate(BAUD.bps()), clocks, &mut apb2);
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

    // USART2 receiver (RXNE IRQ enqueues bytes).
    minz::usart2_rx::init_pa2_rx(dp.USART2, clocks.pclk1().raw(), BAUD);

    // Boot: energize step (AM32 init `comStep(2)` main.c:2087) at duty 0
    // so the polling startup has a driven pair + a matching comp mux, but
    // no current (duty 0 = low-side hold). CURRENT_STEP stays 1.
    tim1_motor_pwm::set_roles_for_step(1);
    change_comp_input(1);
    tim1_motor_pwm::set_duty(0);
    comp2::set_exti_enabled(false); // masked until interrupt mode engages

    writeln!(&mut tx, "\r\n=== am32_clone (UART_DUTY_MODE + ZC_TRACE, {} baud) ===\r", BAUD).ok();
    writeln!(&mut tx, "throttle: '<pct>\\n' (0..100) | 's'/'w' stop | 'Z' trace | 'i' info | 'b' bb\r").ok();

    let mut tx_writer = UartTxWriter::new(tx, TX_RING);

    // Priorities: COMP=0, TIM1_UP_TIM16(=COM)=0, TIM6=3, USART2=2
    // (peripherals.c:450,491 + directive). <<4 IPR encoding via priority.rs.
    unsafe {
        priority::set_prigroup_preempt4_sub0();
        priority::set_irq_prio(Interrupt::COMP, 0);
        priority::set_irq_prio(Interrupt::TIM1_UP_TIM16, 0);
        priority::set_irq_prio(Interrupt::TIM6_DACUNDER, 3);
        priority::set_irq_prio(Interrupt::USART2, 2);
        NVIC::unmask(Interrupt::COMP);
        NVIC::unmask(Interrupt::TIM1_UP_TIM16);
        NVIC::unmask(Interrupt::TIM6_DACUNDER);
        NVIC::unmask(Interrupt::USART2);
    }

    // panic::ensure_rtt() (in init) left PRIMASK=1 — re-enable IRQs.
    unsafe { cortex_m::interrupt::enable() };

    main_entry(&mut tx_writer)
}

// ===============================================================
// COMP ISR — stm32l4xx_it.c:276-290 + interruptRoutine main.c:918-948.
// Priority 0.
// ===============================================================
#[interrupt]
fn COMP() {
    comp_isr(&SCHED, &DRIVE)
}

#[inline]
fn comp_isr(sched: &Sched, drive: &Drive) {
    let exti = unsafe { &*stm32::EXTI::ptr() };
    if exti.pr1.read().pr22().bit_is_set() {
        // if INTERVAL_TIMER->CNT > average_interval>>1  (it.c:280)
        if interval_cnt() > (sched.average_interval.load(Ordering::Relaxed) >> 1) {
            comp2::clear_pending(); // it.c:281
            interrupt_routine(sched, drive); // it.c:282
        } else {
            // gate closed: clear ONLY if the level sits at the pre-ZC
            // level (AM32: getCompOutputLevel()==rising; minz-inverted →
            // comp2::value() != rising). Else LEAVE PENDING (their camp:
            // a post-ZC crossing re-fires until the gate opens).
            if comp2::value() != drive.rising.load(Ordering::Relaxed) {
                comp2::clear_pending(); // it.c:284-285
            }
        }
    }
}

/// interruptRoutine — main.c:918-948.
#[inline]
fn interrupt_routine(sched: &Sched, drive: &Drive) {
    // persistence: reject while the level is still pre-ZC (main.c:932-940;
    // `getCompOutputLevel()==rising` → return, inverted to `value != rising`).
    let filter = drive.filter_level.load(Ordering::Relaxed);
    let rising = drive.rising.load(Ordering::Relaxed);
    for _ in 0..filter {
        if comp2::value() != rising {
            return;
        }
    }
    free(|_| {
        mask_phase_interrupts(); // main.c:942
        sched.last_zc.store(sched.this_zc.load(Ordering::Relaxed), Ordering::Relaxed); // :943
        let t = interval_cnt() as u16; // :944 thiszctime = INTERVAL_TIMER_COUNT
        sched.this_zc.store(t, Ordering::Relaxed);
        set_interval_cnt(0); // :945
        set_and_enable_com_int(sched.wait_time.load(Ordering::Relaxed).wrapping_add(1)); // :946
    });
    bb_record(EV_ACC, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, sched.this_zc.load(Ordering::Relaxed));
}

// ===============================================================
// COM ISR (TIM16 wrap on the shared TIM1_UP_TIM16 vector) —
// PeriodElapsedCallback main.c:896-916. Priority 0.
// ===============================================================
#[interrupt]
fn TIM1_UP_TIM16() {
    tim1_up_tim16_isr(&SCHED, &DRIVE, &ZCT, &DUTY)
}

#[inline]
fn tim1_up_tim16_isr(sched: &Sched, drive: &Drive, zct: &ZctTrace, duty: &Duty) {
    com_clear_flag(); // ack TIM16 UIF (TIM1.UIE is off, so this is the COM tick)
    disable_com_timer_int(); // main.c:898
    commutate(sched, drive); // :899
    // commutation_interval = (ci + (lastzctime+thiszctime)/2) / 2  (:900)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let lz = sched.last_zc.load(Ordering::Relaxed) as u32;
    let tz = sched.this_zc.load(Ordering::Relaxed) as u32;
    let ci = am32::blend_interval(ci_old, lz, tz);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = ci*temp_advance>>6 ; waitTime = ci/2 - advance  (:902-906)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);
    zct.write(sched, drive, duty); // ZC_TRACE main.c:908
    if !drive.old_routine.load(Ordering::Relaxed) {
        enable_comp_interrupts(); // main.c:910-912
    }
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    if zc < 10000 {
        drive.zero_crosses.store(zc + 1, Ordering::Relaxed); // main.c:913-915
    }
}

// ===============================================================
// TIM6 ISR — tenKhzRoutine main.c:1608-1809. Priority 3.
// ===============================================================
static RAMP_COUNT: AtomicU16 = AtomicU16::new(0);
static OC_ACC: AtomicU32 = AtomicU32::new(0);
static OC_CNT: AtomicU32 = AtomicU32::new(0);

#[interrupt]
fn TIM6_DACUNDER() {
    tim6_dacunder_isr(&SCHED, &DRIVE, &DUTY, &BENCH, &ZCT)
}

#[inline]
fn tim6_dacunder_isr(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace) {
    minz::tim6_loop::clear_flag();

    // duty_cycle = duty_cycle_setpoint (main.c:1611); tenkhzcounter++ (:1612)
    let duty_val = duty.duty_cycle_setpoint.load(Ordering::Relaxed) as i32;
    drive.tenkhz_counter.store(drive.tenkhz_counter.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);

    if !duty.killed.load(Ordering::Relaxed) {
        // ---- duty ramp (main.c:1736-1791). ramp_divider=0 → every tick.
        let _ = duty.ramp_count.fetch_add(1, Ordering::Relaxed);
        let last = duty.last_duty_cycle.load(Ordering::Relaxed) as i32;
        let zc = drive.zero_crosses.load(Ordering::Relaxed);
        let avg = sched.average_interval.load(Ordering::Relaxed);
        // Ramp rate selection + one-sided step clamp + 0..2000 domain
        // clamp (am32::ramp_rate / ramp_toward).
        let rate = am32::ramp_rate(zc, last, avg);
        let duty_val = am32::ramp_toward(last, duty_val, rate);
        duty.duty_cycle.store(duty_val, Ordering::Relaxed);

        // ---- apply (main.c:1771-1791) ----
        let tim1_arr = max_duty() as u32;
        let running = drive.running.load(Ordering::Relaxed);
        let input = duty.input.load(Ordering::Relaxed);
        let base = (duty_val as u32 * tim1_arr) / 2000;
        let adjusted = if running && input > 47 { base + 1 } else { base };
        duty.last_duty_cycle.store(duty_val, Ordering::Relaxed); // main.c:1789
        tim1_motor_pwm::set_carrier_arr(tim1_arr as u16); // SET_AUTO_RELOAD_PWM (:1790)
        tim1_motor_pwm::set_duty(adjusted as u16); // SET_DUTY_CYCLE_ALL (:1791)

        // ---- old_routine polling (main.c:1679-1696) ----
        if drive.old_routine.load(Ordering::Relaxed) && running {
            mask_phase_interrupts(); // main.c:1681
            get_bemf_state(drive); // :1682
            if !drive.zcfound.load(Ordering::Relaxed) {
                let rising = drive.rising.load(Ordering::Relaxed);
                let bc = drive.bemf_counter.load(Ordering::Relaxed);
                let thresh = if rising {
                    drive.min_bemf_up.load(Ordering::Relaxed)
                } else {
                    drive.min_bemf_down.load(Ordering::Relaxed)
                };
                if bc > thresh {
                    drive.zcfound.store(true, Ordering::Relaxed);
                    zcfoundroutine(sched, drive, zct, duty);
                }
            }
        }
    }

    // ---- UART deadman (main.c:1425): 3 s no command → throttle 0 ----
    let dm = bench.uart_deadman_ticks.load(Ordering::Relaxed) + 1;
    if dm > UART_DEADMAN_LIMIT {
        duty.uart_duty_input.store(0, Ordering::Relaxed);
        duty.adjusted_input.store(0, Ordering::Relaxed);
        bench.uart_deadman_ticks.store(UART_DEADMAN_LIMIT + 1, Ordering::Relaxed);
    } else {
        bench.uart_deadman_ticks.store(dm, Ordering::Relaxed);
    }

    // ---- observer ADC harvest + bench-safety kills ----
    let (_a, _b, cur, vbat) = adc_sync::inj_read();
    bench.i_raw.store(cur, Ordering::Relaxed);
    bench.vbat_raw.store(vbat, Ordering::Relaxed);
    let (acc, cnt, tripped) = am32::oc_step(
        bench.oc_acc.load(Ordering::Relaxed),
        bench.oc_cnt.load(Ordering::Relaxed),
        cur,
        OC_WINDOW_TICKS,
        OC_KILL_RAW_AVG,
    );
    bench.oc_acc.store(acc, Ordering::Relaxed);
    bench.oc_cnt.store(cnt, Ordering::Relaxed);
    if tripped {
        safety_kill(drive, duty, 1);
    }
    if vbat < VBAT_ABS_FLOOR_RAW && drive.running.load(Ordering::Relaxed) {
        safety_kill(drive, duty, 2);
    }
}

// ===============================================================
// USART2 RX ISR — enqueue bytes (priority 2). Parser runs in main.
// ===============================================================
#[interrupt]
fn USART2() {
    usart2_isr(&RX)
}

/// Fully decoupled from globals: the ring it services arrives as a
/// parameter (the trampoline owns the wiring).
#[inline]
fn usart2_isr(rx: &RxRing) {
    let usart = unsafe { &*stm32::USART2::ptr() };
    while usart.isr.read().rxne().bit_is_set() {
        rx.push(usart.rdr.read().bits() as u16);
    }
    // Clear overrun/framing/noise errors (main.c:1417-1419).
    if usart.isr.read().ore().bit_is_set() || usart.isr.read().fe().bit_is_set() || usart.isr.read().nf().bit_is_set() {
        usart.icr.write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
    }
}
