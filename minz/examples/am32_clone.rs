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
//! `HAL` bundle below).
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

use core::fmt::Write as _;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering};

use cortex_m::peripheral::NVIC;
use cortex_m_rt::entry;

use minz::Am32Hal;
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

use minz_core::am32::{RxRing, UartDuty, ZCT_REC, ZctRing};
use minz_core::am32_control::{
    bemf_timeout_rekick, desync_check_band, honor_stop, set_input, variable_pwm_ride,
};
use minz_core::am32_hal::Hal;
use minz_core::am32_isr::{comp_isr, tim1_up_tim16_isr, tim6_dacunder_isr};
use minz_core::zct_trace::ZctTrace;
use minz_core::am32_loop::{
    Bench, DUTY_FULL, Drive, Duty, INIT_INTERVAL_TICKS, Sched, TARGET_MIN_BEMF_COUNTS,
    apply_uart_cmd, bemf_timeout_resets, filter_and_duty_max, min_bemf_schedule,
    store_average_interval,
};

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
static ZCT_STREAM_ON: AtomicBool = AtomicBool::new(true);

/// Which phase floats in each sector (minz textbook convention, matches
/// `set_roles_for_step`): sector 0/3 → C, 1/4 → B, 2/5 → A.
// (SECTOR_FLOAT_PHASE moved to comp2::AM32_SECTOR_FLOAT_PHASE with
// the comparator.c helpers; now_10us moved to minz::am32_timers.)

// ===============================================================
// Black box (observer) — behavior in minz::bb; the instance lives here.
// ===============================================================
static BB: Bb = Bb::new();

// ===============================================================
// The HAL bundle — zero-sized register impls (static dispatch, no
// dyn) threaded through minz_core::am32_control. The rm32 timer
// seams (`IntervalTimer`/`ComTimer`) take `&mut self`, so the bundle
// holds them by value (free — ZSTs) and each context builds its own
// instance via this wiring constructor. The only places that name it
// are the ISR trampolines and main_entry's top (the house rule).
// ===============================================================
#[inline(always)]
fn hal() -> Am32Hal<'static> {
    Hal {
        interval: Am32Timers,
        com: Am32Timers,
        pwm: Tim1Pwm,
        phase: Tim1Pwm,
        comp: Comp2,
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
// USART2 RX byte ring (ISR producer, main consumer).
// ===============================================================
const RX_N: usize = 256;
static RX_RING: [AtomicU16; RX_N] = [const { AtomicU16::new(0) }; RX_N];
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

/// The coupled RX-ring state, grouped as `minz_core::am32::RxRing`
/// (push/pop host-tested there) and serviced by
/// `minz::usart2_rx::service_rx` — this file owns only the storage
/// and the wiring.
static RX: RxRing<'static, RX_N> = RxRing {
    ring: &RX_RING,
    head: &RX_HEAD,
    tail: &RX_TAIL,
};

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
    stop_req: &STOP_REQ,
    dump_req: &DUMP_REQ,
    info_req: &INFO_REQ,
    zct_stream_on: &ZCT_STREAM_ON,
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
    let rx = &RX;
    let mut hal = hal();
    let hal = &mut hal;
    // AM32 TIMER1_MAX_ARR (targets.h:5335) — the base carrier ARR,
    // threaded into variable_pwm_ride (core can't see minz's const).
    let base_arr = minz::TIM1_AUTORELOAD;

    // Main-context UART parser state (mirrors uart_duty_poll main.c:1367).
    let mut uart = UartDuty::new();
    // ramp_count local-ish (ramp_divider=0 → ramp every tenKhz tick, so
    // main only needs the maps below; ramp lives in the TIM6 ISR).

    loop {
        minz::iwdg::refresh();
        rx_drain(rx, &mut uart, duty, bench);
        honor_stop(drive, duty, bench, hal);

        // e_com_time (main.c:2159): (sum+4)>>1, 0.5 µs units. Threaded into
        // the average_interval + low-rpm ceiling bands below.
        let e_com_time = sched.intervals().e_com_time();
        // input = uart_duty_get()  (main.c:1131) then setInput()
        set_input(sched, drive, duty, hal);
        min_bemf_schedule(drive);
        variable_pwm_ride(sched, duty, base_arr, hal);

        let average_interval = store_average_interval(sched, e_com_time);
        desync_check_band(sched, drive, duty, hal, average_interval);

        let (running, zc) = filter_and_duty_max(sched, drive, duty, e_com_time, average_interval);
        bemf_timeout_resets(drive, duty, zc);
        bemf_timeout_rekick(sched, drive, duty, zct, hal, running);

        telemetry_drain(bench, zct, tx_writer);
        handle_requests(sched, drive, duty, bench, zct, tx_writer);
    }
}

/// RX drain band — drain USART2 RX ring → uart_duty parser dispatch
/// (uart_duty_poll main.c:1367).
#[inline]
fn rx_drain(rx: &RxRing<RX_N>, uart: &mut UartDuty, duty: &Duty, bench: &Bench) {
    while let Some(c) = rx.pop() {
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
fn handle_requests(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace<ZCT_N>, tx: &mut UartTxWriter) {
    if bench.info_req.swap(false, Ordering::Relaxed) {
        print_info(sched, drive, duty, bench, zct, tx);
    }
    if bench.dump_req.swap(false, Ordering::Relaxed) {
        dump_bb(tx);
    }
    if duty.killed.swap(false, Ordering::Relaxed) {
        let reason = duty.kill_reason.load(Ordering::Relaxed);
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

fn print_info(sched: &Sched, drive: &Drive, duty: &Duty, bench: &Bench, zct: &ZctTrace<ZCT_N>, tx: &mut UartTxWriter) {
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
        zct.ring.drop.load(Ordering::Relaxed),
        drive.zcfr_guard_hits.load(Ordering::Relaxed),
    );
}

fn dump_bb(tx: &mut UartTxWriter) {
    BB.dump(|b| {
        for &byte in b {
            let _ = tx.push(byte);
        }
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
    comp2::am32_change_comp_input(1);
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
// THE VECTOR TABLE — every #[interrupt] trampoline, together at the
// file's end. Trampolines are the ONLY functions that name the
// static instances; each servicing body (minz_core::am32_isr) takes
// its world as parameters.
// ===============================================================

/// COMP (priority 0) — the ZC chain.
#[interrupt]
fn COMP() {
    let mut hal = hal();
    comp_isr(&SCHED, &DRIVE, &mut hal)
}

/// TIM16 wrap on the shared vector (priority 0) — the COM tick.
#[interrupt]
fn TIM1_UP_TIM16() {
    let mut hal = hal();
    tim1_up_tim16_isr(&SCHED, &DRIVE, &ZCT, &DUTY, &mut hal)
}

/// TIM6 19.6 kHz (priority 3) — tenKhzRoutine.
#[interrupt]
fn TIM6_DACUNDER() {
    let mut hal = hal();
    tim6_dacunder_isr(&SCHED, &DRIVE, &DUTY, &BENCH, &ZCT, &mut hal)
}

/// USART2 RX (priority 2) — enqueue bytes; parser runs in main.
/// Body lives with its peripheral: minz::usart2_rx::service_rx
/// (drain RXNE + error clears, AM32 main.c:1417-1419).
#[interrupt]
fn USART2() {
    minz::usart2_rx::service_rx(&RX)
}
