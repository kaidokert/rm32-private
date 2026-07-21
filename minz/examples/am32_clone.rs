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

// --- Ramp rates (main.c:1746-1754; targets.h globals 3113/3117/3121) ---
/// `zero_crosses<150 || last_duty<150` → RAMP_SPEED_STARTUP.
const MAX_RAMP_STARTUP: i32 = 2;
/// else `average_interval>500` → RAMP_SPEED_LOW_RPM.
const MAX_RAMP_LOW_RPM: i32 = 6;
/// else → RAMP_SPEED_HIGH_RPM.
const MAX_RAMP_HIGH_RPM: i32 = 16;

// --- Low-rpm duty ceiling map (main.c:436-439,2450) ---
const LOW_RPM_LEVEL: i32 = 20; // main.c:436, thousand-erpm
const HIGH_RPM_LEVEL: i32 = 70; // main.c:437
const THROTTLE_MAX_AT_LOW_RPM: i32 = 400; // main.c:438
const THROTTLE_MAX_AT_HIGH_RPM: i32 = 2000; // main.c:439
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
// INTERVAL_TIMER = TIM2 (PSC=39 → 0.5 µs), COM_TIMER = TIM16 (PSC=39).
// Raw register access to hit AM32's exact macro semantics
// (peripherals.c:439-503, peripherals.h:15-26).
// ===============================================================

fn interval_timer_init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.tim2en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(39)); // AM32 peripherals.c:442 TIM2->PSC=39
    tim.arr.write(|w| unsafe { w.bits(0xFFFF) }); // main.c:443 ARR=0xFFFF
    tim.egr.write(|w| w.ug().set_bit());
    tim.cr1.modify(|_, w| w.cen().set_bit());
}

/// INTERVAL_TIMER_COUNT — peripherals.h:15.
#[inline]
fn interval_cnt() -> u32 {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.read().bits() & 0xFFFF
}

/// SET_INTERVAL_TIMER_COUNT — peripherals.h:22.
#[inline]
fn set_interval_cnt(v: u16) {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.write(|w| unsafe { w.bits(v as u32) });
}

/// MX_TIM16_Init (peripherals.c:485): PSC=39, ARR=0xFFFF, ARPE ON,
/// UIE off at boot. NVIC prio 0 set in main. The counter free-runs.
fn com_timer_init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb2enr
            .modify(|_, w| w.tim16en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(39)); // peripherals.c:495 Prescaler=39
    tim.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.modify(|_, w| w.uie().clear_bit()); // DISABLE_COM_TIMER_INT at boot
    // ARPE on (peripherals.c:501 EnableARRPreload), free-run CEN on.
    tim.cr1.modify(|_, w| w.arpe().set_bit().cen().set_bit());
}

/// SET_AND_ENABLE_COM_INT(time) — peripherals.h:19-21:
/// CNT=0, ARR=time, SR=0, DIER.UIE=1.
#[inline]
fn set_and_enable_com_int(time: u16) {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.cnt.write(|w| unsafe { w.cnt().bits(0) });
    tim.arr.write(|w| unsafe { w.arr().bits(time) });
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.modify(|_, w| w.uie().set_bit());
}

/// DISABLE_COM_TIMER_INT() — peripherals.h:17.
#[inline]
fn disable_com_timer_int() {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.dier.modify(|_, w| w.uie().clear_bit());
}

/// Write COM_TIMER->ARR directly (zcfoundroutine main.c:1884; vestigial
/// in polling — UIE is off there so it never fires).
#[inline]
fn com_set_arr(v: u16) {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.arr.write(|w| unsafe { w.arr().bits(v) });
}

#[inline]
fn com_clear_flag() {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.sr.write(|w| unsafe { w.bits(0) });
}

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
// map()/getAbsDif() — functions.c.
// ===============================================================

/// AM32 `map` (functions.c:22-40): recursive binary-search interpolation
/// with end clamping. Transliterated verbatim.
fn map(x: i32, in_min: i32, in_max: i32, out_min: i32, out_max: i32) -> i32 {
    if x >= in_max {
        return out_max;
    }
    if x <= in_min {
        return out_min;
    }
    if in_min > in_max {
        return map(x, in_max, in_min, out_max, out_min);
    }
    if out_min == out_max {
        return out_min;
    }
    let in_mid = (in_min + in_max) >> 1;
    let out_mid = (out_min + out_max) >> 1;
    if in_min == in_mid {
        return out_mid;
    }
    if x <= in_mid {
        map(x, in_min, in_mid, out_min, out_mid)
    } else {
        map(x, in_mid + 1, in_max, out_mid, out_max)
    }
}

/// getAbsDif (functions.c:42).
#[inline]
fn get_abs_dif(a: i32, b: i32) -> u32 {
    (a - b).unsigned_abs()
}

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
const ZCT_BATCH_LEN: u32 = 50;
const ZCT_BATCH_CI_TICKS: u32 = 200;
static ZCT_COMM_N: AtomicU32 = AtomicU32::new(0);
static ZCT_BATCHING: AtomicBool = AtomicBool::new(false);

// zct_write (main.c:1542-1567): one canonical row per commutation.
fn zct_write() {
    // Batch-decimation gate.
    let n = ZCT_COMM_N.fetch_add(1, Ordering::Relaxed);
    if n % ZCT_BATCH_LEN == 0 {
        // Batch boundary: re-evaluate the mode.
        ZCT_BATCHING.store(
            COMMUTATION_INTERVAL.load(Ordering::Relaxed) < ZCT_BATCH_CI_TICKS,
            Ordering::Relaxed,
        );
    }
    let batching = ZCT_BATCHING.load(Ordering::Relaxed);
    if batching && (n / ZCT_BATCH_LEN) % 2 == 1 {
        return; // the skipped half-duty of the batch cycle
    }
    let step = CURRENT_STEP.load(Ordering::Relaxed) as u8;
    let old = OLD_ROUTINE.load(Ordering::Relaxed);
    let thiszc = THIS_ZC.load(Ordering::Relaxed);
    let ci = COMMUTATION_INTERVAL.load(Ordering::Relaxed) as u16;
    let wait = WAIT_TIME.load(Ordering::Relaxed);
    let duty = DUTY_CYCLE.load(Ordering::Relaxed);
    let tk = TENKHZ_COUNTER.load(Ordering::Relaxed);
    let avg = AVERAGE_INTERVAL.load(Ordering::Relaxed) as u16;
    let rec: [u8; ZCT_REC] = [
        0x5B,
        0xA9,
        (step & 0x07)
            | if old { 0x80 } else { 0 }
            | if batching { 0x40 } else { 0 },
        thiszc as u8,
        (thiszc >> 8) as u8,
        ci as u8,
        (ci >> 8) as u8,
        wait as u8,
        (wait >> 8) as u8,
        duty as u8,
        (duty >> 8) as u8,
        tk as u8,
        (tk >> 8) as u8,
        avg as u8,
        (avg >> 8) as u8,
    ];
    free(|_| {
        let h = ZCT_HEAD.load(Ordering::Relaxed);
        let nx = (h + 1) % ZCT_N;
        if nx == ZCT_TAIL.load(Ordering::Relaxed) {
            ZCT_DROP.fetch_add(1, Ordering::Relaxed);
            ZCT_TAIL.store((ZCT_TAIL.load(Ordering::Relaxed) + 1) % ZCT_N, Ordering::Relaxed);
        }
        for (i, b) in rec.iter().enumerate() {
            ZCT_RING[h][i].store(*b as u16, Ordering::Relaxed);
        }
        ZCT_HEAD.store(nx, Ordering::Relaxed);
    });
}

// ===============================================================
// USART2 RX byte ring (ISR producer, main consumer).
// ===============================================================
const RX_N: usize = 256;
static RX_RING: [AtomicU16; RX_N] = [const { AtomicU16::new(0) }; RX_N];
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

// ===============================================================
// commutate() — main.c:854-894 (forward-only factory path).
// ===============================================================
fn commutate() {
    // step++ ; if step>6 { step=1; desync_check=1 }  (main.c:856-861)
    let mut step = CURRENT_STEP.load(Ordering::Relaxed);
    step += 1;
    if step > 6 {
        step = 1;
        DESYNC_CHECK.store(true, Ordering::Relaxed);
    }
    // rising = step % 2  (main.c:862)
    let rising = (step & 1) == 1;
    RISING.store(rising, Ordering::Relaxed);
    CURRENT_STEP.store(step, Ordering::Relaxed);
    let sector = (step - 1) as usize;

    // comStep(step) (main.c:876) — minz role-only flip; CCRs hold the
    // tick-shaped duty. AM32 wraps this in __disable_irq; set_roles_for_step
    // does its own interrupt::free.
    tim1_motor_pwm::set_roles_for_step(sector as u8);
    // changeCompInput() (main.c:879).
    change_comp_input(sector);

    // if average_interval > polling_mode_changeover+500 → old_routine=1
    // (main.c:881-883).
    if AVERAGE_INTERVAL.load(Ordering::Relaxed) > POLLING_MODE_CHANGEOVER + 500 {
        OLD_ROUTINE.store(true, Ordering::Relaxed);
    }
    // bemfcounter=0; zcfound=0 (main.c:885-886).
    BEMF_COUNTER.store(0, Ordering::Relaxed);
    ZCFOUND.store(false, Ordering::Relaxed);
    // commutation_intervals[step-1] = commutation_interval (main.c:887).
    COMMUTATION_INTERVALS[sector].store(COMMUTATION_INTERVAL.load(Ordering::Relaxed), Ordering::Relaxed);

    bb_record(EV_REF, sector as u8, COMMUTATION_INTERVAL.load(Ordering::Relaxed) as u16);
}

// ===============================================================
// getBemfState() — main.c:817-852 (L431 `!getCompOutputLevel()` branch,
// which equals minz `comp2::value()`). Counts when the level matches the
// direction; a run of bad reads over threshold resets the counter.
// ===============================================================
fn get_bemf_state() {
    let cs = comp2::value(); // = !getCompOutputLevel() (main.c:831)
    let rising = RISING.load(Ordering::Relaxed);
    // rising: count when current_state; else count when !current_state.
    // Both reduce to `cs == rising` (main.c:833-851).
    if cs == rising {
        let v = BEMF_COUNTER.load(Ordering::Relaxed).saturating_add(1);
        BEMF_COUNTER.store(v, Ordering::Relaxed);
    } else {
        let bc = BAD_COUNT.load(Ordering::Relaxed) + 1;
        BAD_COUNT.store(bc, Ordering::Relaxed);
        if bc > BAD_COUNT_THRESHOLD {
            BEMF_COUNTER.store(0, Ordering::Relaxed);
        }
    }
}

// ===============================================================
// zcfoundroutine() — main.c:1868-1915 (polling mode, blocking).
// ===============================================================
static ZCFR_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

fn zcfoundroutine() {
    // thiszctime = INTERVAL_TIMER_COUNT; SET_INTERVAL_TIMER_COUNT(0)
    let thiszc = interval_cnt() as u16; // main.c:1870
    set_interval_cnt(0); // main.c:1871
    THIS_ZC.store(thiszc, Ordering::Relaxed);
    // commutation_interval = (thiszctime + 3*ci)/4  (main.c:1872)
    let ci_old = COMMUTATION_INTERVAL.load(Ordering::Relaxed);
    let ci = (thiszc as u32 + 3 * ci_old) / 4;
    COMMUTATION_INTERVAL.store(ci, Ordering::Relaxed);
    // advance = temp_advance*ci >> 6 ; waitTime = ci/2 - advance  (1873-4)
    let advance = (TEMP_ADVANCE * ci) >> 6;
    let wait = (ci / 2).saturating_sub(advance);
    WAIT_TIME.store(wait as u16, Ordering::Relaxed);

    // while INTERVAL_TIMER_COUNT < waitTime { if zero_crosses<5 break }
    // (main.c:1875-1879). DEVIATION #1: bounded by a spin guard — AM32's
    // loop is unbounded (a wedged INTERVAL_TIMER hangs the 20 kHz ISR
    // forever; AM32 accepts that, the IWDG would reboot). We add a loud
    // 65535-iteration break so the tick can't be captured indefinitely.
    let zc = ZERO_CROSSES.load(Ordering::Relaxed);
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
            ZCFR_GUARD_HITS.fetch_add(1, Ordering::Relaxed);
            break;
        }
    }

    com_set_arr(wait as u16); // COM_TIMER->ARR = waitTime (main.c:1884)
    commutate(); // main.c:1889
    zct_write(); // ZC_TRACE main.c:1891
    BEMF_COUNTER.store(0, Ordering::Relaxed); // main.c:1893
    BAD_COUNT.store(0, Ordering::Relaxed); // main.c:1894
    ZERO_CROSSES.store(zc.saturating_add(1), Ordering::Relaxed); // main.c:1896

    // changeover to interrupt mode (non-stall/non-rc_car path,
    // main.c:1908-1913): commutation_interval < polling_mode_changeover.
    if ci < POLLING_MODE_CHANGEOVER {
        OLD_ROUTINE.store(false, Ordering::Relaxed);
        enable_comp_interrupts();
    }
}

// ===============================================================
// startMotor() — main.c:950-959. Per the directive, comp interrupts are
// NOT enabled here (polling reads level, no EXTI) — the enable happens
// at the polling→interrupt changeover in zcfoundroutine. This is the one
// intentional divergence from AM32's line 958 `enableCompInterrupts()`.
// ===============================================================
fn start_motor() {
    if !RUNNING.load(Ordering::Relaxed) {
        commutate(); // main.c:953
        COMMUTATION_INTERVAL.store(STARTUP_INTERVAL_TICKS, Ordering::Relaxed); // :954
        set_interval_cnt(5000); // SET_INTERVAL_TIMER_COUNT(5000) main.c:955
        OLD_ROUTINE.store(true, Ordering::Relaxed);
        BEMF_COUNTER.store(0, Ordering::Relaxed);
        RUNNING.store(true, Ordering::Relaxed); // main.c:956
    }
}

// ===============================================================
// Bench-safety kill: float all legs, latch, mask comp, freeze bb.
// ===============================================================
fn safety_kill(reason: u16) {
    tim1_motor_pwm::all_off();
    RUNNING.store(false, Ordering::Relaxed);
    DUTY_CYCLE_SETPOINT.store(0, Ordering::Relaxed);
    DUTY_CYCLE.store(0, Ordering::Relaxed);
    LAST_DUTY_CYCLE.store(0, Ordering::Relaxed);
    OLD_ROUTINE.store(true, Ordering::Relaxed);
    mask_phase_interrupts();
    disable_com_timer_int();
    KILL_REASON.store(reason, Ordering::Relaxed);
    KILLED.store(true, Ordering::Relaxed);
    free(|cs| BB.borrow(cs).borrow_mut().freeze());
}

// ===============================================================
fn main_entry(tx_writer: &mut UartTxWriter) -> ! {
    // Main-context UART parser state (mirrors uart_duty_poll main.c:1367).
    let mut uart_acc: u32 = 0;
    let mut uart_acc_n: u8 = 0;
    // ramp_count local-ish (ramp_divider=0 → ramp every tenKhz tick, so
    // main only needs the maps below; ramp lives in the TIM6 ISR).

    loop {
        minz::iwdg::refresh();

        // ---- drain USART2 RX ring → parser (uart_duty_poll) --------
        while RX_TAIL.load(Ordering::Relaxed) != RX_HEAD.load(Ordering::Relaxed) {
            let t = RX_TAIL.load(Ordering::Relaxed);
            let c = RX_RING[t].load(Ordering::Relaxed) as u8;
            RX_TAIL.store((t + 1) % RX_N, Ordering::Relaxed);
            parse_rx_byte(c, &mut uart_acc, &mut uart_acc_n);
        }

        // ---- honor bench stop request ------------------------------
        if STOP_REQ.swap(false, Ordering::Relaxed) {
            tim1_motor_pwm::all_off();
            RUNNING.store(false, Ordering::Relaxed);
            OLD_ROUTINE.store(true, Ordering::Relaxed);
            ZERO_CROSSES.store(0, Ordering::Relaxed);
            DUTY_CYCLE_SETPOINT.store(0, Ordering::Relaxed);
            DUTY_CYCLE.store(0, Ordering::Relaxed);
            LAST_DUTY_CYCLE.store(0, Ordering::Relaxed);
            mask_phase_interrupts();
            disable_com_timer_int();
        }

        // ---- e_com_time (main.c:2159) ------------------------------
        let mut sum: i32 = 0;
        for s in &COMMUTATION_INTERVALS {
            sum += s.load(Ordering::Relaxed) as i32;
        }
        let e_com_time = (sum + 4) >> 1; // 0.5 µs units

        // input = uart_duty_get()  (main.c:1131) then setInput()
        set_input();

        // min_bemf_counts schedule (main.c:2177-2188, non-bi-dir).
        if ZERO_CROSSES.load(Ordering::Relaxed) < 5 {
            MIN_BEMF_UP.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
            MIN_BEMF_DOWN.store(TARGET_MIN_BEMF_COUNTS * 2, Ordering::Relaxed);
        } else {
            MIN_BEMF_UP.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
            MIN_BEMF_DOWN.store(TARGET_MIN_BEMF_COUNTS, Ordering::Relaxed);
        }

        // variable_pwm (main.c:2192-2195). mode 1: carrier rides the
        // commutation interval; duty ratio is preserved by the tenKhz
        // `duty*tim1_arr/2000` rescale.
        if VARIABLE_PWM == 1 {
            let ci = COMMUTATION_INTERVAL.load(Ordering::Relaxed) as i32;
            let arr = map(ci, 96, 200, (TIMER1_MAX_ARR / 2) as i32, TIMER1_MAX_ARR as i32);
            tim1_motor_pwm::set_carrier_arr(arr as u16);
        }

        // average_interval = e_com_time / 3  (main.c:2283)
        let average_interval = if e_com_time > 0 { (e_com_time / 3) as u32 } else { 0 };
        AVERAGE_INTERVAL.store(average_interval, Ordering::Relaxed);

        // desync_check block (main.c:2284-2300) — non-bi-dir subset.
        if DESYNC_CHECK.load(Ordering::Relaxed) && ZERO_CROSSES.load(Ordering::Relaxed) > 10 {
            let lai = LAST_AVERAGE_INTERVAL.load(Ordering::Relaxed) as i32;
            if get_abs_dif(lai, average_interval as i32) > (average_interval >> 1)
                && average_interval < 2000
            {
                ZERO_CROSSES.store(0, Ordering::Relaxed); // main.c:2286
                DESYNC_HAPPENED.fetch_add(1, Ordering::Relaxed); // :2287
                // (!bi_direction && input>47) || commutation_interval>1000 → running=0
                let input = INPUT.load(Ordering::Relaxed);
                let ci = COMMUTATION_INTERVAL.load(Ordering::Relaxed);
                if input > 47 || ci > 1000 {
                    RUNNING.store(false, Ordering::Relaxed);
                }
                OLD_ROUTINE.store(true, Ordering::Relaxed); // :2291
                LAST_DUTY_CYCLE.store(MIN_STARTUP_DUTY / 2, Ordering::Relaxed); // :2295
                bb_record(EV_DSY, (CURRENT_STEP.load(Ordering::Relaxed) - 1) as u8, average_interval as u16);
            }
            DESYNC_CHECK.store(false, Ordering::Relaxed); // :2297
            LAST_AVERAGE_INTERVAL.store(average_interval, Ordering::Relaxed); // :2299
        }

        // ---- low-rpm duty ceiling + filter_level (main.c:2441-2469) --
        let running = RUNNING.load(Ordering::Relaxed);
        let e_rpm: i32 = if running && e_com_time > 0 { 600000 / e_com_time } else { 0 };
        let k_erpm = e_rpm / 10;
        let duty_max = if LOW_RPM_THROTTLE_LIMIT {
            map(k_erpm, LOW_RPM_LEVEL, HIGH_RPM_LEVEL, THROTTLE_MAX_AT_LOW_RPM, THROTTLE_MAX_AT_HIGH_RPM)
        } else {
            2000
        };
        DUTY_CYCLE_MAXIMUM.store(duty_max as u16, Ordering::Relaxed);

        let zc = ZERO_CROSSES.load(Ordering::Relaxed);
        let ci = COMMUTATION_INTERVAL.load(Ordering::Relaxed);
        let mut filter = if zc < 100 && ci > 500 {
            12
        } else {
            map(average_interval as i32, 100, 500, 3, 12)
        };
        if ci < 50 {
            filter = 2;
        }
        FILTER_LEVEL.store(filter as u16, Ordering::Relaxed);

        // ---- bemf timeout leniency resets (main.c:2261-2273) --------
        let adj = ADJUSTED_INPUT.load(Ordering::Relaxed);
        if zc > 1000 || adj == 0 {
            BEMF_TIMEOUT_HAPPENED.store(0, Ordering::Relaxed);
        }
        if zc > 100 && adj < 200 {
            BEMF_TIMEOUT_HAPPENED.store(0, Ordering::Relaxed);
        }

        // ---- bemf timeout re-kick (main.c:2495-2509) ----------------
        if interval_cnt() > BEMF_TIMEOUT_TICKS && running {
            BEMF_TIMEOUT_HAPPENED.fetch_add(1, Ordering::Relaxed);
            mask_phase_interrupts();
            OLD_ROUTINE.store(true, Ordering::Relaxed);
            if INPUT.load(Ordering::Relaxed) < 48 {
                RUNNING.store(false, Ordering::Relaxed);
                COMMUTATION_INTERVAL.store(5000, Ordering::Relaxed);
            }
            ZERO_CROSSES.store(0, Ordering::Relaxed);
            zcfoundroutine();
        }

        // ---- telemetry: drain ZC_TRACE ring → USART1 DMA writer -----
        if ZCT_STREAM_ON.load(Ordering::Relaxed) {
            drain_zct(tx_writer);
        }
        tx_writer.service();

        // ---- on-demand info / bb dump / kill notice -----------------
        if INFO_REQ.swap(false, Ordering::Relaxed) {
            print_info(tx_writer);
        }
        if DUMP_REQ.swap(false, Ordering::Relaxed) {
            dump_bb(tx_writer);
        }
        if KILLED.swap(false, Ordering::Relaxed) {
            let reason = KILL_REASON.load(Ordering::Relaxed);
            let _ = write!(
                tx_writer,
                "!! KILL reason={} (1=OC 2=vbat) iraw={} vbat={}\r\n",
                reason,
                I_RAW.load(Ordering::Relaxed),
                VBAT_RAW.load(Ordering::Relaxed),
            );
            dump_bb(tx_writer);
        }
    }
}

/// setInput() duty-setpoint block — main.c:1180-1326 factory subset
/// (armed always true; no sine, no brake, no current limit).
fn set_input() {
    // input = uart_duty_get()  (main.c:1131,1423-1431).
    let input = UART_DUTY_INPUT.load(Ordering::Relaxed);
    INPUT.store(input, Ordering::Relaxed);

    let running = RUNNING.load(Ordering::Relaxed);
    if input >= 47 {
        // main.c:1182-1196
        if !running {
            tim1_motor_pwm::all_off(); // main.c:1184
            if !OLD_ROUTINE.load(Ordering::Relaxed) {
                start_motor(); // main.c:1185-1187
            }
            RUNNING.store(true, Ordering::Relaxed); // main.c:1188
            LAST_DUTY_CYCLE.store(MIN_STARTUP_DUTY, Ordering::Relaxed); // :1189
        }
        // duty_cycle_setpoint = map(input, 47, 2047, minimum_duty_cycle, 2000)
        let sp = map(input as i32, 47, 2047, MINIMUM_DUTY_CYCLE as i32, 2000);
        DUTY_CYCLE_SETPOINT.store(sp as u16, Ordering::Relaxed);
    } else {
        // input < 47 (main.c:1203-1300, comp_pwm subset)
        DUTY_CYCLE_SETPOINT.store(0, Ordering::Relaxed);
        if !running {
            OLD_ROUTINE.store(true, Ordering::Relaxed);
            ZERO_CROSSES.store(0, Ordering::Relaxed);
            BAD_COUNT.store(0, Ordering::Relaxed);
            tim1_motor_pwm::all_off();
        }
    }

    // startup clamp (main.c:1302-1314), non-bi-dir (30 >> 0 = 30).
    let mut sp = DUTY_CYCLE_SETPOINT.load(Ordering::Relaxed);
    if input >= 47 && ZERO_CROSSES.load(Ordering::Relaxed) < 30 {
        if sp < MIN_STARTUP_DUTY {
            sp = MIN_STARTUP_DUTY;
        }
        if sp > STARTUP_MAX_DUTY_CYCLE {
            sp = STARTUP_MAX_DUTY_CYCLE;
        }
    }
    let dmax = DUTY_CYCLE_MAXIMUM.load(Ordering::Relaxed);
    if sp > dmax {
        sp = dmax;
    }
    DUTY_CYCLE_SETPOINT.store(sp, Ordering::Relaxed);
}

/// uart_duty_poll byte handler — main.c:1367-1416, plus bench keys.
fn parse_rx_byte(c: u8, uart_acc: &mut u32, uart_acc_n: &mut u8) {
    match c {
        b'0'..=b'9' => {
            if *uart_acc_n < 4 {
                *uart_acc = *uart_acc * 10 + (c - b'0') as u32;
                *uart_acc_n += 1;
            }
        }
        b's' | b'w' => {
            // main.c:1376-1381 stop; 'w' also latches a bench kill/disarm.
            UART_DUTY_INPUT.store(0, Ordering::Relaxed);
            ADJUSTED_INPUT.store(0, Ordering::Relaxed);
            UART_DEADMAN_TICKS.store(0, Ordering::Relaxed);
            *uart_acc = 0;
            *uart_acc_n = 0;
            STOP_REQ.store(true, Ordering::Relaxed);
        }
        b'Z' => {
            let on = !ZCT_STREAM_ON.load(Ordering::Relaxed);
            ZCT_STREAM_ON.store(on, Ordering::Relaxed);
        }
        b'i' => INFO_REQ.store(true, Ordering::Relaxed),
        b'b' => DUMP_REQ.store(true, Ordering::Relaxed),
        _ => {
            // any other byte = terminator → commit (main.c:1382-1415)
            if *uart_acc_n != 0 {
                let v = *uart_acc;
                let mut inn: u32 = if v <= 100 {
                    v * 20 + 47 // percent (main.c:1387)
                } else if v <= 1000 {
                    v * 2 + 47 // permille (main.c:1389)
                } else {
                    47
                };
                if v == 0 {
                    inn = 0; // 0 = STOP (main.c:1397-1398)
                    STOP_REQ.store(true, Ordering::Relaxed);
                } else {
                    if inn < 48 {
                        inn = 48;
                    }
                    if inn > 2047 {
                        inn = 2047;
                    }
                }
                UART_DUTY_INPUT.store(inn as u16, Ordering::Relaxed);
                // adjusted_input mirror (main.c:1410)
                ADJUSTED_INPUT.store(if inn <= 48 { 0 } else { inn as u16 }, Ordering::Relaxed);
                UART_DEADMAN_TICKS.store(0, Ordering::Relaxed);
            }
            *uart_acc = 0;
            *uart_acc_n = 0;
        }
    }
}

/// Drain up to 3 ZC_TRACE records into the USART1 DMA ring (main.c:2347-2357).
fn drain_zct(tx: &mut UartTxWriter) {
    let mut nrec = 0;
    while ZCT_TAIL.load(Ordering::Relaxed) != ZCT_HEAD.load(Ordering::Relaxed) && nrec < 3 {
        let t = ZCT_TAIL.load(Ordering::Relaxed);
        for i in 0..ZCT_REC {
            let _ = tx.push(ZCT_RING[t][i].load(Ordering::Relaxed) as u8);
        }
        ZCT_TAIL.store((t + 1) % ZCT_N, Ordering::Relaxed);
        nrec += 1;
    }
}

fn print_info(tx: &mut UartTxWriter) {
    let _ = write!(
        tx,
        "i step={} old={} run={} ci={} avg={} zc={} duty={} iraw={} vbat={} drop={} guard={}\r\n",
        CURRENT_STEP.load(Ordering::Relaxed),
        OLD_ROUTINE.load(Ordering::Relaxed) as u8,
        RUNNING.load(Ordering::Relaxed) as u8,
        COMMUTATION_INTERVAL.load(Ordering::Relaxed),
        AVERAGE_INTERVAL.load(Ordering::Relaxed),
        ZERO_CROSSES.load(Ordering::Relaxed),
        DUTY_CYCLE.load(Ordering::Relaxed),
        I_RAW.load(Ordering::Relaxed),
        VBAT_RAW.load(Ordering::Relaxed),
        ZCT_DROP.load(Ordering::Relaxed),
        ZCFR_GUARD_HITS.load(Ordering::Relaxed),
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
    let exti = unsafe { &*stm32::EXTI::ptr() };
    if exti.pr1.read().pr22().bit_is_set() {
        // if INTERVAL_TIMER->CNT > average_interval>>1  (it.c:280)
        if interval_cnt() > (AVERAGE_INTERVAL.load(Ordering::Relaxed) >> 1) {
            comp2::clear_pending(); // it.c:281
            interrupt_routine(); // it.c:282
        } else {
            // gate closed: clear ONLY if the level sits at the pre-ZC
            // level (AM32: getCompOutputLevel()==rising; minz-inverted →
            // comp2::value() != rising). Else LEAVE PENDING (their camp:
            // a post-ZC crossing re-fires until the gate opens).
            if comp2::value() != RISING.load(Ordering::Relaxed) {
                comp2::clear_pending(); // it.c:284-285
            }
        }
    }
}

/// interruptRoutine — main.c:918-948.
fn interrupt_routine() {
    // persistence: reject while the level is still pre-ZC (main.c:932-940;
    // `getCompOutputLevel()==rising` → return, inverted to `value != rising`).
    let filter = FILTER_LEVEL.load(Ordering::Relaxed);
    let rising = RISING.load(Ordering::Relaxed);
    for _ in 0..filter {
        if comp2::value() != rising {
            return;
        }
    }
    free(|_| {
        mask_phase_interrupts(); // main.c:942
        LAST_ZC.store(THIS_ZC.load(Ordering::Relaxed), Ordering::Relaxed); // :943
        let t = interval_cnt() as u16; // :944 thiszctime = INTERVAL_TIMER_COUNT
        THIS_ZC.store(t, Ordering::Relaxed);
        set_interval_cnt(0); // :945
        set_and_enable_com_int(WAIT_TIME.load(Ordering::Relaxed).wrapping_add(1)); // :946
    });
    bb_record(EV_ACC, (CURRENT_STEP.load(Ordering::Relaxed) - 1) as u8, THIS_ZC.load(Ordering::Relaxed));
}

// ===============================================================
// COM ISR (TIM16 wrap on the shared TIM1_UP_TIM16 vector) —
// PeriodElapsedCallback main.c:896-916. Priority 0.
// ===============================================================
#[interrupt]
fn TIM1_UP_TIM16() {
    com_clear_flag(); // ack TIM16 UIF (TIM1.UIE is off, so this is the COM tick)
    disable_com_timer_int(); // main.c:898
    commutate(); // :899
    // commutation_interval = (ci + (lastzctime+thiszctime)/2) / 2  (:900)
    let ci_old = COMMUTATION_INTERVAL.load(Ordering::Relaxed);
    let lz = LAST_ZC.load(Ordering::Relaxed) as u32;
    let tz = THIS_ZC.load(Ordering::Relaxed) as u32;
    let ci = (ci_old + ((lz + tz) >> 1)) >> 1;
    COMMUTATION_INTERVAL.store(ci, Ordering::Relaxed);
    // advance = ci*temp_advance>>6 ; waitTime = ci/2 - advance  (:902-906)
    let advance = (ci * TEMP_ADVANCE) >> 6;
    let wait = (ci >> 1).saturating_sub(advance);
    WAIT_TIME.store(wait as u16, Ordering::Relaxed);
    zct_write(); // ZC_TRACE main.c:908
    if !OLD_ROUTINE.load(Ordering::Relaxed) {
        enable_comp_interrupts(); // main.c:910-912
    }
    let zc = ZERO_CROSSES.load(Ordering::Relaxed);
    if zc < 10000 {
        ZERO_CROSSES.store(zc + 1, Ordering::Relaxed); // main.c:913-915
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
    minz::tim6_loop::clear_flag();

    // duty_cycle = duty_cycle_setpoint (main.c:1611); tenkhzcounter++ (:1612)
    let mut duty = DUTY_CYCLE_SETPOINT.load(Ordering::Relaxed) as i32;
    TENKHZ_COUNTER.store(TENKHZ_COUNTER.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);

    if !KILLED.load(Ordering::Relaxed) {
        // ---- duty ramp (main.c:1736-1791). ramp_divider=0 → every tick.
        let _ = RAMP_COUNT.fetch_add(1, Ordering::Relaxed);
        let last = LAST_DUTY_CYCLE.load(Ordering::Relaxed) as i32;
        let zc = ZERO_CROSSES.load(Ordering::Relaxed);
        let avg = AVERAGE_INTERVAL.load(Ordering::Relaxed);
        let max_change = if zc < 150 || last < 150 {
            MAX_RAMP_STARTUP
        } else if avg > 500 {
            MAX_RAMP_LOW_RPM
        } else {
            MAX_RAMP_HIGH_RPM
        };
        // signed clamp — equivalent to AM32's two unsigned one-sided
        // clamps (main.c:1760-1766) without underflow panics.
        if duty - last > max_change {
            duty = last + max_change;
        }
        if last - duty > max_change {
            duty = last - max_change;
        }
        let duty = duty.clamp(0, 2000) as u16;
        DUTY_CYCLE.store(duty, Ordering::Relaxed);

        // ---- apply (main.c:1771-1791) ----
        let tim1_arr = max_duty() as u32;
        let running = RUNNING.load(Ordering::Relaxed);
        let input = INPUT.load(Ordering::Relaxed);
        let base = (duty as u32 * tim1_arr) / 2000;
        let adjusted = if running && input > 47 { base + 1 } else { base };
        LAST_DUTY_CYCLE.store(duty, Ordering::Relaxed); // main.c:1789
        tim1_motor_pwm::set_carrier_arr(tim1_arr as u16); // SET_AUTO_RELOAD_PWM (:1790)
        tim1_motor_pwm::set_duty(adjusted as u16); // SET_DUTY_CYCLE_ALL (:1791)

        // ---- old_routine polling (main.c:1679-1696) ----
        if OLD_ROUTINE.load(Ordering::Relaxed) && running {
            mask_phase_interrupts(); // main.c:1681
            get_bemf_state(); // :1682
            if !ZCFOUND.load(Ordering::Relaxed) {
                let rising = RISING.load(Ordering::Relaxed);
                let bc = BEMF_COUNTER.load(Ordering::Relaxed);
                let thresh = if rising {
                    MIN_BEMF_UP.load(Ordering::Relaxed)
                } else {
                    MIN_BEMF_DOWN.load(Ordering::Relaxed)
                };
                if bc > thresh {
                    ZCFOUND.store(true, Ordering::Relaxed);
                    zcfoundroutine();
                }
            }
        }
    }

    // ---- UART deadman (main.c:1425): 3 s no command → throttle 0 ----
    let dm = UART_DEADMAN_TICKS.load(Ordering::Relaxed) + 1;
    if dm > UART_DEADMAN_LIMIT {
        UART_DUTY_INPUT.store(0, Ordering::Relaxed);
        ADJUSTED_INPUT.store(0, Ordering::Relaxed);
        UART_DEADMAN_TICKS.store(UART_DEADMAN_LIMIT + 1, Ordering::Relaxed);
    } else {
        UART_DEADMAN_TICKS.store(dm, Ordering::Relaxed);
    }

    // ---- observer ADC harvest + bench-safety kills ----
    let (_a, _b, cur, vbat) = adc_sync::inj_read();
    I_RAW.store(cur, Ordering::Relaxed);
    VBAT_RAW.store(vbat, Ordering::Relaxed);
    let acc = OC_ACC.load(Ordering::Relaxed) + cur as u32;
    let cnt = OC_CNT.load(Ordering::Relaxed) + 1;
    if cnt >= OC_WINDOW_TICKS {
        if acc / OC_WINDOW_TICKS > OC_KILL_RAW_AVG {
            safety_kill(1);
        }
        OC_ACC.store(0, Ordering::Relaxed);
        OC_CNT.store(0, Ordering::Relaxed);
    } else {
        OC_ACC.store(acc, Ordering::Relaxed);
        OC_CNT.store(cnt, Ordering::Relaxed);
    }
    if vbat < VBAT_ABS_FLOOR_RAW && RUNNING.load(Ordering::Relaxed) {
        safety_kill(2);
    }
}

// ===============================================================
// USART2 RX ISR — enqueue bytes (priority 2). Parser runs in main.
// ===============================================================
#[interrupt]
fn USART2() {
    let usart = unsafe { &*stm32::USART2::ptr() };
    while usart.isr.read().rxne().bit_is_set() {
        let c = usart.rdr.read().bits() as u16;
        let h = RX_HEAD.load(Ordering::Relaxed);
        let nx = (h + 1) % RX_N;
        if nx != RX_TAIL.load(Ordering::Relaxed) {
            RX_RING[h].store(c, Ordering::Relaxed);
            RX_HEAD.store(nx, Ordering::Relaxed);
        }
    }
    // Clear overrun/framing/noise errors (main.c:1417-1419).
    if usart.isr.read().ore().bit_is_set() || usart.isr.read().fe().bit_is_set() || usart.isr.read().nf().bit_is_set() {
        usart.icr.write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
    }
}
