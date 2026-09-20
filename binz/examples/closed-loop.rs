//! Closed-loop sensorless BEMF drive — consumes minz-core's commutation
//! brain through binz's HAL adapters (`binz::mzhal`). NOT a rewrite: the
//! ZC/commutation/desync science is minz-core's; binz provides the G071
//! register impls + a SOFTWARE comparator over the VPH ADC.
//!
//! Two ISRs are the loop (am32_clone pattern): TIM6 @ 20 kHz =
//! `tim6_dacunder_isr` (control/BEMF-poll/ramp/safety); TIM16 =
//! `tim1_up_tim16_isr` (commutation timer). State lives in the
//! Sched/Drive/Duty/Bench/ZctTrace clusters over portable_atomic statics.
//!
//! FIRST BRING-UP — start at a low fixed throttle, guards primary, attended.
//! Run: `cargo run --release --example closed-loop`

#![no_std]
#![no_main]

use binz::mzhal::{self, BinzMotor, BinzObserver};
use binz::{blackbox, harvest, stage};
use core::fmt::Write;
use cortex_m_rt::entry;
use minz_core::am32::{ZCT_REC, ZctRing};
use minz_core::am32_control::{desync_check_band, honor_stop, set_input, variable_pwm_ride};
use minz_core::am32_hal::{Motor, Observer};
use minz_core::am32_isr::{comp_isr, tim1_up_tim16_isr, tim6_dacunder_isr};
use minz_core::am32_loop::{
    Bench, DUTY_FULL, Drive, Duty, INIT_INTERVAL_TICKS, Sched, TARGET_MIN_BEMF_COUNTS,
    bemf_timeout_resets, filter_and_duty_max, min_bemf_schedule, store_average_interval,
};
use minz_core::zct_trace::ZctTrace;
use portable_atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering::Relaxed};
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::{FullConfig, Serial};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::{USART2, interrupt};

// ---- AM32 globals as atomics (0.5 us / 2000 domain, matched via 2 MHz TIM2) ----
static COMMUTATION_INTERVAL: AtomicU32 = AtomicU32::new(INIT_INTERVAL_TICKS);
static COMMUTATION_INTERVALS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
static AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_AVERAGE_INTERVAL: AtomicU32 = AtomicU32::new(0);
static LAST_ZC: AtomicU16 = AtomicU16::new(0);
static THIS_ZC: AtomicU16 = AtomicU16::new(0);
static WAIT_TIME: AtomicU16 = AtomicU16::new(0);
static ZERO_CROSSES: AtomicU32 = AtomicU32::new(0);
static BEMF_COUNTER: AtomicU16 = AtomicU16::new(0);
static BAD_COUNT: AtomicU16 = AtomicU16::new(0);
static CURRENT_STEP: AtomicU16 = AtomicU16::new(1);
static RISING: AtomicBool = AtomicBool::new(true);
static OLD_ROUTINE: AtomicBool = AtomicBool::new(true);
static RUNNING: AtomicBool = AtomicBool::new(false);
static ZCFOUND: AtomicBool = AtomicBool::new(false);
static DESYNC_CHECK: AtomicBool = AtomicBool::new(false);
static DESYNC_HAPPENED: AtomicU32 = AtomicU32::new(0);
static BEMF_TIMEOUT_HAPPENED: AtomicU32 = AtomicU32::new(0);
static FILTER_LEVEL: AtomicU16 = AtomicU16::new(5);
static MIN_BEMF_UP: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);
static MIN_BEMF_DOWN: AtomicU16 = AtomicU16::new(TARGET_MIN_BEMF_COUNTS);
static INPUT: AtomicU16 = AtomicU16::new(0);
static ADJUSTED_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DUTY_INPUT: AtomicU16 = AtomicU16::new(0);
static UART_DEADMAN_TICKS: AtomicU32 = AtomicU32::new(0);
static DUTY_CYCLE_SETPOINT: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static LAST_DUTY_CYCLE: AtomicU16 = AtomicU16::new(0);
static DUTY_CYCLE_MAXIMUM: AtomicU16 = AtomicU16::new(DUTY_FULL);
static TENKHZ_COUNTER: AtomicU16 = AtomicU16::new(0);
static ZCFR_GUARD_HITS: AtomicU32 = AtomicU32::new(0);
static TIM1_ARR_SHADOW: AtomicU16 = AtomicU16::new(mzhal::ARR as u16);
static KILLED: AtomicBool = AtomicBool::new(false);
static KILL_REASON: AtomicU16 = AtomicU16::new(0);
static I_RAW: AtomicU16 = AtomicU16::new(0);
static VBAT_RAW: AtomicU16 = AtomicU16::new(0);
static OC_ACC: AtomicU32 = AtomicU32::new(0);
static OC_CNT: AtomicU32 = AtomicU32::new(0);
static VBAT_LOW_TICKS: AtomicU32 = AtomicU32::new(0);
static VBAT_FLOOR_RAW: AtomicU16 = AtomicU16::new(0);
static STOP_REQ: AtomicBool = AtomicBool::new(false);
static DUMP_REQ: AtomicBool = AtomicBool::new(false);
static INFO_REQ: AtomicBool = AtomicBool::new(false);
static GECKO_REQ: AtomicBool = AtomicBool::new(false);
static WAX_REQ: AtomicBool = AtomicBool::new(false);
static HIST_REQ: AtomicBool = AtomicBool::new(false);
static FREERUN_REQ: AtomicBool = AtomicBool::new(false);
static ZCT_STREAM_ON: AtomicBool = AtomicBool::new(false);
static DELAY_IN_FREE_CYC: AtomicU32 = AtomicU32::new(0);
static DELAY_OUT_FREE_CYC: AtomicU32 = AtomicU32::new(0);

const ZCT_N: usize = 8;
static ZCT_RING: [[AtomicU16; ZCT_REC]; ZCT_N] =
    [const { [const { AtomicU16::new(0) }; ZCT_REC] }; ZCT_N];
static ZCT_HEAD: AtomicUsize = AtomicUsize::new(0);
static ZCT_TAIL: AtomicUsize = AtomicUsize::new(0);
static ZCT_DROP: AtomicU32 = AtomicU32::new(0);
static ZCT_COMM_N: AtomicU32 = AtomicU32::new(0);
static ZCT_BATCHING: AtomicBool = AtomicBool::new(false);

static SCHED: Sched = Sched {
    commutation_interval: &COMMUTATION_INTERVAL,
    interval_hist: &COMMUTATION_INTERVALS,
    average_interval: &AVERAGE_INTERVAL,
    last_average_interval: &LAST_AVERAGE_INTERVAL,
    last_zc: &LAST_ZC,
    this_zc: &THIS_ZC,
    wait_time: &WAIT_TIME,
};
static DRIVE: Drive = Drive {
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
static DUTY: Duty = Duty {
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
static RAMP_COUNT: AtomicU16 = AtomicU16::new(0);
static BENCH: Bench = Bench {
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
static ZCT: ZctTrace<ZCT_N> = ZctTrace {
    ring: ZctRing {
        ring: &ZCT_RING,
        head: &ZCT_HEAD,
        tail: &ZCT_TAIL,
        drop: &ZCT_DROP,
    },
    comm_n: &ZCT_COMM_N,
    batching: &ZCT_BATCHING,
};

static REC: mzhal::Rec = mzhal::Rec;
static CS: mzhal::Cs = mzhal::Cs;
static SAFETY: mzhal::SafetyAdc = mzhal::SafetyAdc;
static LOOP: mzhal::Loop = mzhal::Loop;

#[inline(always)]
fn motor() -> BinzMotor {
    Motor {
        pwm: mzhal::Tim1Pwm,
        comp: mzhal::AdcComp,
        phase: mzhal::Tim1Pwm,
        interval: mzhal::Timers,
        com: mzhal::Timers,
    }
}
#[inline(always)]
fn observer() -> BinzObserver<'static> {
    Observer {
        bb: &REC,
        cs: &CS,
        adc: &SAFETY,
        lt: &LOOP,
    }
}

const FORCED_TICKS: u32 = 11000; // 5500 us — MATCH the rotor's natural rate at
// 20% so the OBSERVE forced spin doesn't
// over-commutate (2500 us was 2.3x too fast ->
// the rotor slipped and detection was ~42%)
const FALLBACK_TICKS: u32 = 15000; // 7500 us — slow fallback once ZC-driven
/// Handoff state: false = OBSERVE (force-commutate at the ramp rate, comp
/// detects ZCs but does NOT commutate — so it can't disrupt the clean forced
/// spin and the rotor stays synced), true = ENGAGED (comp drives commutation,
/// forced re-arm is only a slow fallback).
static HANDOFF_ENGAGED: AtomicBool = AtomicBool::new(false);

#[interrupt]
fn TIM16() {
    let mut motor = motor();
    tim1_up_tim16_isr(&SCHED, &DRIVE, &ZCT, &DUTY, &mut motor, &observer());
    // Re-open the BEMF window after each commutation (minz's enable is gated on
    // !old_routine, which commutate() may have just flipped).
    mzhal::comp_enable();
    // Forced com-timer re-arm: at the ramp rate during OBSERVE (holds the rotor
    // synced without comp disruption), a slow fallback once ENGAGED (comp's
    // ZC-driven arm wins; this only catches a missed ZC).
    let rearm = if HANDOFF_ENGAGED.load(Relaxed) {
        FALLBACK_TICKS
    } else {
        FORCED_TICKS
    };
    unsafe {
        let t16 = &*stm32::TIM16::ptr();
        t16.arr().write(|w| w.bits(rearm));
        t16.cnt().write(|w| w.bits(0));
        t16.dier().write(|w| w.bits(1));
        t16.cr1().modify(|r, w| w.bits(r.bits() | 1)); // CEN (OPM)
    }
}

static MAX_BC: AtomicU16 = AtomicU16::new(0);
static COMMUT_N: AtomicU32 = AtomicU32::new(0);

// ---- DEATH-CAPTURE BEMF RING (WAXWING-style; sole writer = TIM6 ISR) ----
// Records, every 20 kHz tick, EXACTLY what the software comparator saw and
// decided, so a host plot shows WHY the lock slips before any knob is touched
// (the graybeard rule: no detector tuning without a plot vs a synced ref).
// Circular; FREEZES on the kill so the last ~51 ms captured is the slip itself.
//   VF  = VF_DIAG  (floating-phase mV the detector compared)
//   NEU = NEUTRAL_DIAG (per-sector self-cal neutral it compared against)
//   POS = TIM2 CNT (0.5 us since the last commutation)
//   FLG = step(0..2) | level(3) | edge_latched(4) | zc_accepted(5)
//         | engaged(6) | (vm_pin_mV>>2)<<7   [coarse bus, ~4 mV pin/LSB]
const RING_N: usize = 1024;
static RG_VF: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_NEU: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_POS: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_FLG: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static RG_HEAD: AtomicUsize = AtomicUsize::new(0);
static RING_FROZEN: AtomicBool = AtomicBool::new(false);

fn dump_ring(serial: &mut Serial<USART2, FullConfig>, tag: u16) {
    // If we entered NOT killed (a mid-run reference/success dump), a guard trip
    // during the ~100 ms dump must SAFE the motor immediately — never keep it
    // driving through the dump after a trip. (For the kill dump k0=true, so the
    // full frozen ring is emitted.)
    let k0 = KILLED.load(Relaxed);
    let head = RG_HEAD.load(Relaxed);
    let _ = writeln!(
        serial,
        "CLBEMF n={} head={} tag={:04x} ci={} dc={} vm={} kr={}",
        RING_N,
        head,
        tag,
        COMMUTATION_INTERVAL.load(Relaxed),
        DUTY_CYCLE.load(Relaxed),
        mzhal::vm_mv(),
        KILL_REASON.load(Relaxed),
    );
    for i in 0..RING_N {
        if !k0 && KILLED.load(Relaxed) {
            stage::force_safe();
            let _ = writeln!(serial, "CLBEMF ABORT-KILL");
            break;
        }
        let _ = writeln!(
            serial,
            "{:04x} {:04x} {:04x} {:04x}",
            RG_VF[i].load(Relaxed),
            RG_NEU[i].load(Relaxed),
            RG_POS[i].load(Relaxed),
            RG_FLG[i].load(Relaxed),
        );
        while serial.flush().is_err() {}
    }
    let _ = writeln!(serial, "CLBEMF END");
    while serial.flush().is_err() {}
}

#[interrupt]
fn TIM6_DAC_LPTIM1() {
    let pend0 = mzhal::COMP_PEND_N.load(Relaxed);
    let arm0 = mzhal::COM_ARM_N.load(Relaxed);
    mzhal::sample_bemf(); // read the DMA scan -> ZC_LEVEL (+ COMP_PENDING edge)
    let prev_step = CURRENT_STEP.load(Relaxed);
    let mut motor = motor();
    // Interrupt-mode emulation: process a latched BEMF crossing like the real
    // COMP EXTI. Only once ENGAGED — during OBSERVE, comp detects ZCs (for the
    // sync check) but must NOT commutate, or its mistimed early accepts disrupt
    // the clean forced spin and desync the rotor.
    if HANDOFF_ENGAGED.load(Relaxed) && !OLD_ROUTINE.load(Relaxed) {
        comp_isr(&SCHED, &DRIVE, &mut motor, &observer());
    }
    tim6_dacunder_isr(&SCHED, &DRIVE, &DUTY, &BENCH, &ZCT, &mut motor, &observer());
    // Instrument: peak bemf_counter (did polling ever reach its threshold?)
    // and count polling-driven commutations (CURRENT_STEP changed in-ISR).
    let bc = BEMF_COUNTER.load(Relaxed);
    if bc > MAX_BC.load(Relaxed) {
        MAX_BC.store(bc, Relaxed);
    }
    if CURRENT_STEP.load(Relaxed) != prev_step {
        COMMUT_N.store(COMMUT_N.load(Relaxed).wrapping_add(1), Relaxed);
    }

    // Death-capture: log what the detector saw + decided THIS tick, then freeze
    // the ring the moment a guard kills (so its tail is the slip, not the
    // post-safe silence).
    if !RING_FROZEN.load(Relaxed) {
        let step = CURRENT_STEP.load(Relaxed) & 0x7;
        let level = mzhal::zc_level() as u16;
        let edge = (mzhal::COMP_PEND_N.load(Relaxed) != pend0) as u16;
        let acc = (mzhal::COM_ARM_N.load(Relaxed) != arm0) as u16;
        let eng = HANDOFF_ENGAGED.load(Relaxed) as u16;
        let vmc = (mzhal::VM_CACHE.load(Relaxed) >> 2) & 0xFF;
        let flg = step | (level << 3) | (edge << 4) | (acc << 5) | (eng << 6) | (vmc << 7);
        let pos = unsafe { (&*stm32::TIM2::ptr()).cnt().read().bits() as u16 };
        let h = RG_HEAD.load(Relaxed);
        RG_VF[h].store(mzhal::VF_DIAG.load(Relaxed), Relaxed);
        RG_NEU[h].store(mzhal::NEUTRAL_DIAG.load(Relaxed), Relaxed);
        RG_POS[h].store(pos, Relaxed);
        RG_FLG[h].store(flg, Relaxed);
        RG_HEAD.store((h + 1) % RING_N, Relaxed);
        if KILLED.load(Relaxed) {
            RING_FROZEN.store(true, Relaxed);
        }
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    // Analog sense pins.
    let _ = (
        gpioa.pa1.into_analog(),
        gpiob.pb0.into_analog(),
        gpiob.pb1.into_analog(),
        gpiob.pb2.into_analog(),
        gpiob.pb11.into_analog(),
        gpioc.pc4.into_analog(),
    );
    let _nflt = gpioa.pa6.into_floating_input();
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    // USART2 (PA2/PA3) @ 2 Mbaud, FIFO on — the lossless path for the
    // death-capture ring dump (RTT is too slow/lossy for 1024 lines).
    let mut serial = dp
        .USART2
        .usart(
            (gpioa.pa2, gpioa.pa3),
            FullConfig::default()
                .baudrate(2_000_000.bps())
                .fifo_enable(),
            &mut rcc,
        )
        .unwrap();

    // Six drive pins to TIM1 AF2.
    let _ = (
        gpioa.pa7, gpioa.pa8, gpioa.pa9, gpioa.pa10, gpiod.pd3, gpiod.pd4,
    );
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| {
            w.moder7()
                .alternate()
                .moder8()
                .alternate()
                .moder9()
                .alternate()
                .moder10()
                .alternate()
        });
        pa.afrl().modify(|_, w| w.afr(7).af2());
        pa.afrh()
            .modify(|_, w| w.afr(0).af2().afr(1).af2().afr(2).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }

    mzhal::tim1_init();
    mzhal::adc_init();
    mzhal::timers_init();
    delay.delay(5.millis()); // let the triggered ADC/DMA fill

    // Baseline safety floor: seed VBAT_FLOOR from the hardware-sampled bus.
    let vm0 = mzhal::vm_mv();
    // Floor at the STDRIVE102H's real operating minimum (~6 V = ~52% of the
    // ~11.8 V no-load rail), not an arbitrary 60/75%. The bus SAGS in PSU
    // constant-current mode as duty climbs — that's a valid operating point
    // down to 6 V, not a collapse; the guard still kills below the driver
    // minimum. (vm0 pin mV; 6 V bus = ~341 mV pin.)
    let bus_floor = (vm0 * 52 / 100).max(341);
    VBAT_FLOOR_RAW.store(bus_floor, Relaxed); // ~75% -> collapse guard
    rprintln!("closed-loop: VM raw {} mV(pin); floor {}", vm0, bus_floor);
    if vm0 < 341 {
        rprintln!("VM too low, aborting");
        loop {
            cortex_m::asm::nop();
        }
    }

    // Enable driver, precharge low sides.
    en.set_high().ok();
    delay.delay(2.millis());
    mzhal::moe(true);
    delay.delay(30.millis());

    // ================= OPEN-LOOP STARTUP (align + accel ramp) =================
    // minz-core has NO blind startup ramp — it re-arms the com timer only on a
    // detected BEMF ZC, so it expects a rotor ALREADY spinning in the
    // BEMF-detectable band. Bench-proven: at ~6% duty BEMF is at the ~10 mV
    // noise floor (rotor won't accelerate); at ~12% the rotor catches and BEMF
    // swings 200-380 mV. So spin it open-loop here, THEN hand off. Inline
    // guards (bus-sag = the real edge on this current-limited PSU; IS peak)
    // stage-safe on any trouble.
    const STARTUP_CCR: u16 = 640; // 20% — ramp AND hand off at the same duty so
    // there's no operating-point jump at handoff
    // (the 12%->20% jump desynced the rotor)
    const ALIGN_CCR: u16 = 160; // 5% — hold current bounded while stationary
    const P0_US: u32 = 16_000; // first blind step (slow)
    const PFLOOR_US: u32 = 2_000; // handoff step period
    const DP_US: u32 = 40; // ramp accel per step
    let abort = |tag: &str, v: u16| -> ! {
        stage::force_safe();
        rprintln!("!! startup abort {} {}", tag, v);
        loop {
            cortex_m::asm::nop();
        }
    };
    // Align: energize step 1, let the rotor snap to it.
    mzhal::drive_step(1, ALIGN_CCR);
    delay.delay(300.millis());
    // Accelerating six-step ramp.
    let mut step: u8 = 1;
    let mut period = P0_US;
    let mut last_step = step;
    let mut hold = 0u32;
    loop {
        step = (step % 6) + 1;
        mzhal::drive_step(step, STARTUP_CCR);
        last_step = step;
        // Inline safety = BUS-SAG only (the real envelope edge on this
        // current-limited PSU; see the supply-wall scar). Single-shunt IS in
        // the ON window is the inrush PULSE peak, not average — it false-trips
        // and is NOT the current guard here. The PSU 1.5 A limit + the
        // STDRIVE102H's own VDS/nFLT are the current backstops.
        let bus = mzhal::vm_mv();
        if bus < bus_floor {
            abort("sag", bus);
        }
        delay.delay(period.micros());
        if period > PFLOOR_US {
            period = period.saturating_sub(DP_US).max(PFLOOR_US);
        } else {
            // hold at the floor a few revs to stabilize, then hand off.
            hold += 1;
            if hold > 90 {
                break;
            }
        }
    }
    rprintln!(
        "startup: ramp done @ step {} period {} us -> handoff",
        last_step,
        period
    );

    // ================= HANDOFF to minz-core closed-loop =================
    // Seed the state so minz continues from where the ramp left off, then let
    // its polling BEMF detection (old_routine) take over commutation timing.
    // ci/this_zc/last_zc are TIM2 ticks (0.5 us); seed lz=tz=ci so
    // blend_interval keeps ci stable until real ZCs arrive.
    let ci_ticks = 11_000u32; // ~5500 us — mid of the ci clamp band (the
    // rotor's observed natural rate at 20%); the ramp-end period differs, but
    // the clamp + comp pull ci to the real rate from here.
    COMMUTATION_INTERVAL.store(ci_ticks, Relaxed);
    // Seed the 6-slot interval history so average_interval (= their mean) is
    // sane immediately — otherwise it's 0, the comp gate (interval >
    // average/2) never blanks, early demag crossings are accepted, ci
    // collapses and TIM16 runs away.
    for slot in COMMUTATION_INTERVALS.iter() {
        slot.store(ci_ticks, Relaxed);
    }
    AVERAGE_INTERVAL.store(ci_ticks, Relaxed);
    LAST_AVERAGE_INTERVAL.store(ci_ticks, Relaxed);
    LAST_ZC.store(ci_ticks as u16, Relaxed);
    THIS_ZC.store(ci_ticks as u16, Relaxed);
    WAIT_TIME.store((ci_ticks / 4) as u16, Relaxed); // ci/2 - advance(ci/4)
    CURRENT_STEP.store(last_step as u16, Relaxed);
    RISING.store(last_step % 2 == 1, Relaxed);
    // INTERRUPT MODE: the software comparator emulates the HW COMP+EXTI, so
    // commutation is driven by detected ZCs through the com timer — NOT the
    // blocking polling path (which has no HW comparator to hand off to and
    // would starve the ISR via zcfr_spin_wait).
    OLD_ROUTINE.store(false, Relaxed);
    RUNNING.store(true, Relaxed);
    ZCFOUND.store(false, Relaxed);
    ZERO_CROSSES.store(0, Relaxed);
    BEMF_COUNTER.store(0, Relaxed);
    BAD_COUNT.store(0, Relaxed);
    // 20% duty (0..2000 domain) — the fixed 4-channel ADC scan needs the ON
    // window (CCR1 >= 640) to hold all four conversions; 20% also gives strong
    // BEMF and is on the path to the 30% goal (current bounded once synced).
    DUTY_CYCLE_SETPOINT.store(400, Relaxed);
    DUTY_CYCLE.store(400, Relaxed);
    LAST_DUTY_CYCLE.store(400, Relaxed);
    mzhal::seed_step(last_step);
    mzhal::comp_enable(); // open the BEMF window for the first ZC edge
    HANDOFF_ENGAGED.store(false, Relaxed); // start in OBSERVE (forced spin,
    // comp watching but not commutating) so the rotor holds sync; the main
    // loop engages comp once it has observed a stable ZC stream.
    // Force the 20% CCR NOW (same last-step roles) so the DMA fills at the
    // wider ON window before sample_bemf first reads it.
    mzhal::drive_step(last_step, 640);
    unsafe {
        // reset the interval timer (TIM2) so this_zc measures from now
        (&*stm32::TIM2::ptr()).cnt().write(|w| w.bits(0));
    }

    // TIM6 @ 20 kHz control loop. No phase-alignment needed now — sample_bemf
    // just reads the DMA buffer that the HARDWARE-triggered ADC keeps fresh at
    // a fixed mid-ON instant, independent of when this ISR runs.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 4))); // TIM6EN
        let t6 = &*stm32::TIM6::ptr();
        t6.psc().write(|w| w.bits(63)); // 1 MHz
        t6.arr().write(|w| w.bits(49)); // 20 kHz
        t6.dier().write(|w| w.bits(1)); // UIE
        t6.egr().write(|w| w.bits(1));
        t6.cr1().write(|w| w.bits(1)); // CEN
        // Arm the com timer once at the ramp interval so commutation continues
        // into the handoff; polling BEMF then re-arms it at wait_time on lock.
        let t16 = &*stm32::TIM16::ptr();
        t16.arr().write(|w| w.bits(ci_ticks));
        t16.cnt().write(|w| w.bits(0));
        t16.dier().write(|w| w.bits(1)); // UIE
        t16.cr1().modify(|r, w| w.bits(r.bits() | 1)); // CEN (OPM)
    }

    rprintln!("closed-loop: handoff seeded, unmasking ISRs");
    unsafe {
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM16);
    }

    // Throttle SOURCE (set_input copies uart_duty_input -> input).
    UART_DUTY_INPUT.store(420, Relaxed);
    rprintln!("closed-loop: handoff complete, interrupt-mode BEMF");

    // The main-loop control bands (am32_clone subset): set_input arms/starts,
    // variable_pwm_ride sets the ARR shadow, the average/filter/desync bands
    // manage the BEMF lock. The two ISRs (TIM6 20 kHz, TIM16 commutation) do
    // the per-tick work. HAL bundles built per-iteration (free ZSTs).
    let base_arr = mzhal::ARR as u16;
    let mut last = 0u32;
    let mut hb = 0u32;
    // Throttle-walk state: after the lock ENGAGES and holds, ramp the input
    // from 20% (420) toward 30% (600) a rung at a time, but only while the
    // lock stays healthy (comp still driving most commutations). The guards
    // (bus-sag / OC -> KILLED) remain primary throughout.
    let mut walk_input = 420u16;
    const WALK_TARGET: u16 = 600; // ~30% throttle setpoint
    let mut eng_hbs = 0u32; // heartbeats since ENGAGED
    let mut dumped_ref = false; // dumped the synced-reference ring at 20%?
    let mut dumped_success = false; // dumped the 30%-reached ring?
    let mut reach_hold = 0u32; // heartbeats held at WALK_TARGET
    loop {
        if KILLED.load(Relaxed) {
            stage::force_safe();
            rprintln!("!! KILLED reason={}", KILL_REASON.load(Relaxed));
            blackbox::push(
                harvest::tim17_now() as u32,
                blackbox::KIND_KILL_HOST,
                KILL_REASON.load(Relaxed),
            );
            // The ISR already FROZE the ring on the kill — its tail is the slip.
            RING_FROZEN.store(true, Relaxed);
            rprintln!("!! dumping death-capture ring (the slip) over VCOM...");
            dump_ring(&mut serial, 0xDEAD);
            rprintln!("!! ring dumped, stage safed");
            loop {
                cortex_m::asm::nop();
            }
        }

        // Keep the throttle fresh: this example provides it programmatically,
        // so reset the UART deadman each pass (the RX path would in am32_clone).
        UART_DUTY_INPUT.store(walk_input, Relaxed);
        UART_DEADMAN_TICKS.store(0, Relaxed);

        let mut motor = motor();
        let obs = observer();
        honor_stop(&DRIVE, &DUTY, &BENCH, &mut motor);
        let e_com_time = SCHED.intervals().e_com_time();
        set_input(&SCHED, &DRIVE, &DUTY, &mut motor, &obs);
        min_bemf_schedule(&DRIVE);
        variable_pwm_ride(&SCHED, &DUTY, base_arr, &mut motor);
        let _real_avg = store_average_interval(&SCHED, e_com_time);
        // AVERAGE_INTERVAL FIX (2026-09-06, death-ring justified): TRACK the
        // real commutation interval instead of pinning 2000. The ZC gate is
        // `interval_count > average_interval/2` (am32_isr.rs:72). A pinned 2000
        // fixed the gate at 1000 ticks (500 us) — so as the rotor sped up toward
        // 30% its ZCs arrived SOONER than 500 us and were GATED OUT; only late
        // ZCs survived, ci inflated (death-ring: 1758 -> 2137 at higher duty),
        // commutation fell behind the rotor -> slip + current surge -> the deep
        // PSU sag. Tracking makes the gate = ci/2 (blank the first half-step)
        // scale with speed. Clamp < POLLING_MODE_CHANGEOVER+500 (=2500) so
        // commutate()/desync_check never flip to POLLING mode (the software
        // comparator has no HW comp to spin-wait on). Floor 400 keeps the gate
        // sane at max speed.
        let average_interval = COMMUTATION_INTERVAL.load(Relaxed).clamp(400, 2400);
        AVERAGE_INTERVAL.store(average_interval, Relaxed);
        LAST_AVERAGE_INTERVAL.store(average_interval, Relaxed);
        // CLAMP commutation_interval to a sane band around the rotor's
        // observed natural rate at 20% (~5700 us/step ~ 11400 ticks). Tracking
        // within the band gives the correct wait_time (ci/4) advance; the clamp
        // stops the runaway inflation (sparse accepts -> ci -> 40k -> wait_time
        // huge -> late commutation) and collapse that both broke the lock.
        let ci_now = COMMUTATION_INTERVAL.load(Relaxed);
        // CLAMP FLOOR FIX (2026-09-06, plot+RTT justified): the synced-ref dump
        // measured the rotor's NATURAL interval at 20% = ci~1861 ticks, already
        // BELOW the old 2200 floor. The floor was forcing ci UP to 2200 =
        // commutating SLOWER than the rotor actually spins; as duty climbs the
        // rotor speeds up (ci wants to drop further) while the clamp pins it at
        // 2200, so commutation falls progressively behind -> the slip. Floor
        // 800 ticks (400 us/step) still bars a pathological collapse (back-to-
        // back TIM16 starving the ISR) but lets ci track the real rate up
        // through 30%+ so wait_time (ci/4 advance) stays correct.
        COMMUTATION_INTERVAL.store(ci_now.clamp(800, 16000), Relaxed);
        desync_check_band(&SCHED, &DRIVE, &DUTY, &obs, average_interval);
        let (running, zc) =
            filter_and_duty_max(&SCHED, &DRIVE, &DUTY, e_com_time, average_interval);
        // Override minz's low-RPM duty ceiling during acquisition: the fixed
        // 4-channel ADC scan needs >=20% duty (CCR1 >= 640) to sample the
        // floating phase inside the ON window (below that the later
        // conversions land in the OFF/freewheel window -> garbage BEMF, which
        // capped the lock at low RPM). Bench-safe: PSU 1.5 A limit + bus-sag
        // kill bound the current. (Once locked and fast, minz's own ceiling
        // would allow this anyway.)
        DUTY_CYCLE_MAXIMUM.store(680, Relaxed); // headroom for the 30% walk (~600)
        bemf_timeout_resets(&DRIVE, &DUTY, zc);
        // NOTE: bemf_timeout_rekick is intentionally NOT called. It flips
        // old_routine=true, and in the race before the main loop forces it
        // back the TIM6 ISR's polling_bemf_check runs and MASKS the software
        // comparator (COMP_ENABLED=false) — which killed every ZC latch
        // (pend/hb=0). Commutation continuity during acquisition comes from
        // the forced com-timer re-arm; comp_isr drives it once locked.
        let _ = &running;
        // Stay in INTERRUPT mode: keep old_routine false so the TIM6 ISR uses
        // comp_isr, never the polling path.
        OLD_ROUTINE.store(false, Relaxed);

        hb = hb.wrapping_add(1);
        if hb % 3000 == 0 {
            let z = ZERO_CROSSES.load(Relaxed);
            rprintln!(
                "run={} old={} zc={} (+{}) ci={} step={} dc={} | vf={} neu={} zct={} bto={} dsy={} bc={}",
                RUNNING.load(Relaxed) as u8,
                OLD_ROUTINE.load(Relaxed) as u8,
                z,
                z.wrapping_sub(last),
                COMMUTATION_INTERVAL.load(Relaxed),
                CURRENT_STEP.load(Relaxed),
                DUTY_CYCLE.load(Relaxed),
                mzhal::VF_DIAG.load(Relaxed),
                mzhal::NEUTRAL_DIAG.load(Relaxed),
                mzhal::ZC_TRANS.load(Relaxed),
                BEMF_TIMEOUT_HAPPENED.load(Relaxed),
                DESYNC_HAPPENED.load(Relaxed),
                BEMF_COUNTER.load(Relaxed),
            );
            rprintln!(
                "    maxbc={} commut/hb={} rising={} steptrans={} hi%={} force={}",
                MAX_BC.swap(0, Relaxed),
                COMMUT_N.swap(0, Relaxed),
                RISING.load(Relaxed) as u8,
                mzhal::STEP_TRANS_LAST.load(Relaxed),
                mzhal::STEP_HI_PCT.load(Relaxed),
                HANDOFF_ENGAGED.load(Relaxed) as u8,
            );
            let (pend, arm) = (
                mzhal::COMP_PEND_N.load(Relaxed),
                mzhal::COM_ARM_N.load(Relaxed),
            );
            mzhal::COMP_PEND_N.store(0, Relaxed);
            mzhal::COM_ARM_N.store(0, Relaxed);
            let eng = HANDOFF_ENGAGED.load(Relaxed);
            rprintln!(
                "    pend/hb={} armed/hb={} eng={} compEn={}",
                pend,
                arm,
                eng as u8,
                mzhal::comp_enabled() as u8
            );
            // ENGAGE comp once OBSERVE shows a dense ZC stream (~one detection
            // per forced commutation => the rotor is synced to the forced
            // spin). Forced rate 2500 us => ~200 commutations per ~500 ms
            // heartbeat; require pend to be a solid fraction of that.
            if !eng && pend > 38 {
                HANDOFF_ENGAGED.store(true, Relaxed);
                rprintln!("    >>> HANDOFF ENGAGED (pend={})", pend);
            }
            // THROTTLE WALK 20% -> 30%: only after the lock has settled a few
            // heartbeats AND while comp is still driving most commutations
            // (arm healthy). If sync weakens (arm drops) the walk holds; the
            // bus-sag / OC guards stay primary and KILL if current runs away.
            if eng {
                eng_hbs = eng_hbs.wrapping_add(1);
                // SYNCED-REFERENCE dump: once locked and settled at 20% (BEFORE
                // any walk), freeze + dump the ring. This is the graybeard
                // rule's "known-synced reference" — the healthy BEMF the slip
                // capture is read against, on the same axes. ~100 ms main-loop
                // pause; the ISRs keep commutating the steady 20% lock.
                if eng_hbs == 6 && !dumped_ref {
                    // ARM THE 10% CAP (operator's minz-qual hard spec: >10% PSU
                    // drop = instant kill). Anchor to the SYNCED-20% mid-ON bus
                    // baseline, not vm0 — the mid-ON sample sits ~18% under the
                    // true no-load PSU (it samples during the loaded ON pulse),
                    // so 0.9*vm0 would false-trip at 20%. From the synced
                    // operating point a further 10% drop is unambiguously a
                    // desync surge; kill there before the rail collapses.
                    let synced_vm = mzhal::vm_mv();
                    let cap = (synced_vm * 90 / 100).max(300);
                    VBAT_FLOOR_RAW.store(cap, Relaxed);
                    rprintln!(
                        "    >>> 10% CAP ARMED: synced_vm={} floor={}",
                        synced_vm,
                        cap
                    );
                    rprintln!("    >>> dumping SYNCED-REFERENCE ring at 20% lock...");
                    RING_FROZEN.store(true, Relaxed);
                    dump_ring(&mut serial, 0xA11E);
                    RG_HEAD.store(0, Relaxed); // refill cleanly for the walk
                    RING_FROZEN.store(false, Relaxed);
                    dumped_ref = true;
                }
                if eng_hbs > 6 && walk_input < WALK_TARGET {
                    // Advance ONLY when the lock is solid (comp driving most
                    // commutations). If detection weakens, HOLD at this rung so
                    // the lock re-tightens before pushing more current — this
                    // is what stops the walk driving into a slip-surge sag.
                    let ok = arm >= 44;
                    if ok {
                        // +8/heartbeat (heartbeats are ~1 s here): reaches 600
                        // in ~23 heartbeats so a ~50 s run gets to 30% + the
                        // success dump. Still gentle — the lock tracked +3 fine
                        // and the ci-floor fix lets it follow the faster spin.
                        walk_input = (walk_input + 8).min(WALK_TARGET);
                    }
                    rprintln!(
                        "    WALK input={} dc={} (~{}%) arm={} vm={} {}",
                        walk_input,
                        DUTY_CYCLE.load(Relaxed),
                        DUTY_CYCLE.load(Relaxed) as u32 * 100 / 2000,
                        arm,
                        mzhal::vm_mv(),
                        if ok { "advance" } else { "HOLD" },
                    );
                }
                // SUCCESS dump: reached 30% and held it synced.
                if walk_input >= WALK_TARGET && !dumped_success {
                    reach_hold = reach_hold.wrapping_add(1);
                    if reach_hold > 6 {
                        rprintln!("    >>> 30% REACHED + held; dumping SUCCESS ring...");
                        RING_FROZEN.store(true, Relaxed);
                        dump_ring(&mut serial, 0x600d);
                        RG_HEAD.store(0, Relaxed);
                        RING_FROZEN.store(false, Relaxed);
                        dumped_success = true;
                    }
                }
            }
            last = z;
        }

        // COAST-BEMF ORACLE (~5 s in): cut drive and watch phase A. A truly
        // SPINNING rotor coasts with a decaying BEMF sinusoid (p2p shrinks
        // window to window); a STALLED/cogging rotor goes flat immediately.
        // TIM1 keeps triggering the ADC with MOE off, so phase_mv still reads.
        if hb == 320_000 {
            // Coast-BEMF confirmation AFTER the walk reaches and holds 30%.
            mzhal::moe(false); // float all legs — coast
            for w in 0..10u32 {
                let mut mn = 4095u16;
                let mut mx = 0u16;
                for _ in 0..250 {
                    let v = mzhal::phase_mv(0); // phase A
                    if v < mn {
                        mn = v;
                    }
                    if v > mx {
                        mx = v;
                    }
                    cortex_m::asm::delay(64 * 20); // ~20 us
                }
                rprintln!("COAST w{} A p2p={} ({}..{})", w, mx - mn, mn, mx);
            }
            rprintln!("COAST done (decaying p2p = was spinning; flat = stalled)");
            loop {
                cortex_m::asm::nop();
            }
        }
    }
}
