//! Six-step BEMF sensing validation — the foundation for closed-loop.
//!
//! This is NOT closed-loop yet. It drives the motor with FORCED six-step
//! commutation (open-loop, ramped rate) and captures the FLOATING phase's
//! voltage each step, so we can answer the one question that gates all
//! sensorless work on this board: **is a clean BEMF zero-cross (crossing
//! the VM/2 virtual neutral) visible on the VPH ADC dividers?**
//!
//! If yes, closed-loop BEMF commutation is viable (minimal, or via the
//! minz-core brain per REUSE_PLAN). If no, sensing must be fixed first.
//!
//! Drive (TIM1, raw PAC, per-phase roles rewritten each commutation):
//!   HIGH  phase: OCxM=PWM1, CCxE=1, CCxNE=0  -> INH=PWM, INL=0
//!   LOW   phase: OCxM=force-inactive, CCxE=1, CCxNE=1 -> INH=0, INL=on
//!   FLOAT phase: CCxE=0, CCxNE=0 (OSSR drives INH=INL=0) -> phase floats
//! Dead-time (BDTR DTG) protects every INH/INL pair from shoot-through.
//!
//! SAFETY: low fixed duty, tight current guard, short run, all the
//! `binz::harvest` guards + `stage::force_safe` on every exit.
//!
//! Run: `cargo run --release --example sixstep-sense`

#![no_std]
#![no_main]

use binz::{blackbox, harvest, stage, telem};
use core::fmt::Write;
use core::sync::atomic::Ordering::Relaxed;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::FullConfig;
use stm32g0xx_hal::stm32;

const CARRIER_HZ: u32 = 20_000;
const ARR: u32 = 64_000_000 / CARRIER_HZ - 1; // 3199
const DTG: u8 = 26; // ~406 ns dead time
const DUTY_PCT: u32 = 5; // in the measured synced band (graybeard #3)

// Phase channel indices: A=CH1(0), B=CH2(1), C=CH3(2).
// Canonical forward six-step: (high, low, float) per step. The floating
// phase is what we sense.
const STEPS: [(u8, u8, u8); 6] = [
    (0, 1, 2), // A+ B- , C float
    (0, 2, 1), // A+ C- , B float
    (1, 2, 0), // B+ C- , A float
    (1, 0, 2), // B+ A- , C float
    (2, 0, 1), // C+ A- , B float
    (2, 1, 0), // C+ B- , A float
];

/// VPH ADC channel for each phase (A=PB1/IN9, B=PB0/IN8, C=PB2/IN10).
/// harvest publishes them as VPH1_MV(A), VPH2_MV(B), VPH3_MV(C).
fn floating_vph_mv(float_phase: u8) -> u16 {
    match float_phase {
        0 => harvest::VPH1_MV.load(Relaxed),
        1 => harvest::VPH2_MV.load(Relaxed),
        _ => harvest::VPH3_MV.load(Relaxed),
    }
}

/// Compute (CCMR1, CCMR2, CCER) for a step. OCxPE=1 (preload, matches ARPE).
fn step_regs(high: u8, low: u8, float: u8) -> (u32, u32, u32) {
    // Per channel: (ocm 3-bit, ce, cne)
    let role = |ch: u8| -> (u32, u32, u32) {
        if ch == high {
            (0b110, 1, 0) // PWM1, INH=PWM, INL off
        } else if ch == low {
            (0b100, 1, 1) // force-inactive OCx(INH=0), OCxN(INL)=on
        } else {
            (0b100, 0, 0) // float: channel disabled -> INH=INL=0
        }
    };
    let (m1, e1, n1) = role(0);
    let (m2, e2, n2) = role(1);
    let (m3, e3, n3) = role(2);
    let _ = float;
    let ccmr1 = (1 << 3) | (m1 << 4) | (1 << 11) | (m2 << 12);
    let ccmr2 = (1 << 3) | (m3 << 4);
    let ccer = e1 | (n1 << 2) | (e2 << 4) | (n2 << 6) | (e3 << 8) | (n3 << 10);
    (ccmr1, ccmr2, ccer)
}

#[inline]
fn commutate(step: usize, duty: u32) {
    let (high, low, float) = STEPS[step];
    let (ccmr1, ccmr2, ccer) = step_regs(high, low, float);
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.ccr1().write(|w| w.bits(duty));
        tim.ccr2().write(|w| w.bits(duty));
        tim.ccr3().write(|w| w.bits(duty));
        tim.ccmr1_output().write(|w| w.bits(ccmr1));
        tim.ccmr2_output().write(|w| w.bits(ccmr2));
        tim.ccer().write(|w| w.bits(ccer));
        tim.egr().write(|w| w.bits(1)); // COM/UG: latch preloaded regs
    }
}

fn kill(reason: &str) -> ! {
    stage::force_safe();
    rprintln!("!! sixstep stop: {}", reason);
    loop {
        cortex_m::asm::nop();
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

    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _t = gpioc.pc4.into_analog();
    let _v1 = gpiob.pb1.into_analog();
    let _v2 = gpiob.pb0.into_analog();
    let _v3 = gpiob.pb2.into_analog();
    let _nflt = gpioa.pa6.into_floating_input();

    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

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

    // TIM1 base: PWM carrier, dead-time, OSSR/OSSI (disabled channels ->
    // inactive-low so a floating phase is INH=INL=0), ARPE. Channel modes
    // and enables are written per commutation by commutate().
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 11)));
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(ARR));
        tim.rcr().write(|w| w.bits(0));
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32)); // OSSR|OSSI|DTG, MOE off
        tim.egr().write(|w| w.bits(1));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }

    harvest::init();
    delay.delay(50.millis());
    let vm0 = harvest::VM_MV.load(Relaxed) as u32 * 1759 / 100;
    rprintln!(
        "sixstep-sense: VM~{}.{:02} V, duty {}%",
        vm0 / 1000,
        (vm0 % 1000) / 10,
        DUTY_PCT
    );
    if vm0 < 8000 {
        kill("VM too low");
    }
    // Tight current guard for validation: this is forced (maybe stalling)
    // six-step, so cap sustained average low.
    harvest::IS_KILL_DELTA_MV.store(120, Relaxed);
    harvest::IS_PEAK_CEIL_MV.store(600, Relaxed);

    let duty = (ARR + 1) * DUTY_PCT / 100;

    // Enable driver, precharge on step 0.
    commutate(0, 0);
    en.set_high().ok();
    delay.delay(2.millis());
    unsafe {
        (&*stm32::TIM1::ptr())
            .bdtr()
            .modify(|r, w| w.bits(r.bits() | (1 << 15)));
    }
    delay.delay(30.millis());
    harvest::arm();
    if harvest::KILL.load(Relaxed) != 0 {
        kill("guard tripped at enable");
    }

    // DIAGNOSTIC: hold each of the 6 steps 150 ms and read all three VPH.
    // If the drive works, the HIGH phase reads a small PWM-average voltage,
    // the LOW phase ~0, and the FLOAT phase sits at the star potential.
    // Near-identical/zero across all three = phases NOT driving (bug).
    for s in 0..6usize {
        commutate(s, duty);
        delay.delay(150.millis());
        let (h, l, f) = STEPS[s];
        rprintln!(
            "DIAG step{} H{}={} L{}={} F{}={} mV(pin) is={}",
            s,
            h,
            floating_vph_mv(h),
            l,
            floating_vph_mv(l),
            f,
            floating_vph_mv(f),
            harvest::IS_MV.load(Relaxed)
        );
        let _ = writeln!(
            serial,
            "DIAG,{},{},{},{},{}",
            s,
            floating_vph_mv(h),
            floating_vph_mv(l),
            floating_vph_mv(f),
            harvest::IS_MV.load(Relaxed)
        );
        if harvest::KILL.load(Relaxed) != 0 {
            kill("guard during diag");
        }
    }

    rprintln!("forced six-step, ramping commutation rate; capturing floating-phase BEMF");
    let _ = writeln!(serial, "SIXSTEP,start,duty{}", DUTY_PCT);

    // Forced commutation: ramp the electrical rate up. Per step, sample the
    // floating phase several times across the step and stream them so the
    // host can see the BEMF ramp + zero-cross vs the VM/2 neutral.
    // Start VERY slow so the rotor catches from rest (6 steps/erev, so
    // 20 ms/step = 8.3 Hz-e), then ramp gently. The rotor must actually
    // spin for BEMF to exist on the floating phase.
    let mut step: usize = 0;
    let mut step_us: u32 = 20_000; // 20 ms/step = 8.3 Hz-e from rest
    let total_steps: u32 = 900; // ~8 s: ramp up, hold, then coast-check
    let mut txring = telem::TxRing::new();

    for n in 0..total_steps {
        commutate(step, duty);
        // Sample the floating phase across the step (skip a short blank).
        let float_ph = STEPS[step].2;
        let vm_half = harvest::VM_MV.load(Relaxed) / 2; // pin-mV neutral proxy
        let sub = 8u32;
        let mut crossed = false;
        let mut first_mv = 0u16;
        let mut last_mv = 0u16;
        for s in 0..sub {
            delay.delay((step_us / sub).micros());
            if harvest::KILL.load(Relaxed) != 0 {
                let _ = writeln!(serial, "SIXSTEP,kill,{}", harvest::KILL.load(Relaxed));
                blackbox::dump(&mut serial);
                kill("ISR guard during six-step");
            }
            let v = floating_vph_mv(float_ph);
            if s == 0 {
                first_mv = v;
            }
            last_mv = v;
            // crossing of the VM/2 neutral within the step = BEMF ZC
            if (first_mv < vm_half) != (v < vm_half) {
                crossed = true;
            }
        }
        // Stream a compact per-step record every few steps.
        if n % 3 == 0 {
            let is = harvest::IS_MV.load(Relaxed);
            let vm = harvest::VM_MV.load(Relaxed);
            let _ = writeln!(
                serial,
                "ST,{},{},{},{},{},{},{}",
                n, step, float_ph, first_mv, last_mv, crossed as u8, is
            );
            let _ = vm;
            txring.drain(8, |b| serial.write(b).is_ok());
        }
        step = (step + 1) % 6;
        // Ramp the rate up to spin faster (20 ms -> 2.5 ms/step over ~350
        // steps = ~67 Hz-e, in the measured synced band). Then hold.
        if step_us > 2500 {
            step_us -= 50;
        }
    }

    // SPIN SELF-CHECK (no operator eyes): cut the gates and read the coast
    // BEMF. All phases float, no PWM noise -> the harvest sampler reads
    // clean BEMF here (the ONE regime where it's valid). Non-zero BEMF pk-pk
    // = the rotor was turning under six-step; ~0 = it wasn't.
    harvest::COAST_IDX.store(0, Relaxed);
    harvest::COAST_MODE.store(1, Relaxed);
    unsafe {
        (&*stm32::TIM1::ptr())
            .bdtr()
            .modify(|r, w| w.bits(r.bits() & !(1 << 15)));
    }
    let mut spins = 0u32;
    while harvest::COAST_MODE.load(Relaxed) == 1 && spins < 5_000_000 {
        spins += 1;
    }
    // Analyze coast: peak-to-min of the phase-A (VPH1) coast trace.
    let buf = harvest::coast_samples();
    let (mut mx, mut mn) = (0u16, u16::MAX);
    for &v in &buf[10..] {
        if v > mx {
            mx = v;
        }
        if v < mn {
            mn = v;
        }
    }
    let bemf_pp_term = (mx.saturating_sub(mn)) as u32 * 1567 / 100;
    stage::force_safe();
    harvest::disarm();

    // Dump the coast waveform so the shape is inspectable too.
    for (i, &v) in buf.iter().enumerate() {
        let _ = writeln!(serial, "SPINCD,{},{}", i, v);
        while serial.flush().is_err() {}
    }
    let spun = bemf_pp_term > 120; // >~120 mV terminal pk-pk = real rotation
    let _ = writeln!(serial, "SPINCHECK,{},{}", bemf_pp_term, spun as u8);
    rprintln!(
        "SPIN SELF-CHECK: coast BEMF pk-pk = {} mV term -> rotor {}",
        bemf_pp_term,
        if spun {
            "WAS SPINNING"
        } else {
            "was NOT spinning"
        }
    );
    rprintln!(
        "sixstep-sense done: {} commutations, stage safed",
        total_steps
    );
    loop {
        cortex_m::asm::nop();
    }
}
