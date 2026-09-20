//! Integrated BEMF detector — the payoff of the validated primitives.
//!
//! Six-step forced commutation (spins the rotor, validated) + PWM-OFF-
//! window SYNCHRONIZED ADC reads of the floating phase (validated), long
//! sampling for the high-Z divider. Per step we capture the floating
//! phase across the step and the virtual neutral (mean of all three VPH,
//! sampled the same instant), so the host can SEE whether a clean BEMF
//! zero-cross (floating crossing neutral) exists — the question that gates
//! all sensorless closed-loop work here.
//!
//! Empirics decide the scheme (per bench rule: stress against raw
//! waveforms): we dump the floating-vs-neutral ramp; a real detector shows
//! the crossing ~30 deg before the next commutation, repeatable, tracking
//! speed. Noise handling then comes from minz-core, not hand-rolled.
//!
//! SAFETY: low duty (synced band), current abort, short, stage safed on
//! every exit. Coast-BEMF spin self-check at the end.
//!
//! Run: `cargo run --release --example sixstep-bemf`

#![no_std]
#![no_main]

use binz::stage;
use core::fmt::Write;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::FullConfig;
use stm32g0xx_hal::stm32;

const CARRIER_HZ: u32 = 20_000;
const ARR: u32 = 64_000_000 / CARRIER_HZ - 1; // 3199
const DTG: u8 = 26;
const DUTY_PCT: u32 = 12; // higher torque to actually accelerate the prop
// into a speed where BEMF (∝ speed) rises above the
// ~10 mV noise floor. ON window CCR=384=6us — the
// 19.5-cyc (~1.2 us) ADC sample fits easily.

// (high, low, float) per step; float phase is sensed. A=CH1/IN9,
// B=CH2/IN8, C=CH3/IN10.
const STEPS: [(u8, u8, u8); 6] = [
    (0, 1, 2),
    (0, 2, 1),
    (1, 2, 0),
    (1, 0, 2),
    (2, 0, 1),
    (2, 1, 0),
];
const VPH_CH: [u8; 3] = [9, 8, 10]; // A,B,C ADC channels

fn adc_read(ch: u8) -> u16 {
    unsafe {
        let adc = &*stm32::ADC::ptr();
        adc.chselr0().write(|w| w.bits(1 << ch));
        adc.isr().write(|w| w.bits(1 << 3));
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2)));
        while adc.isr().read().bits() & (1 << 3) == 0 {}
        ((adc.dr().read().bits() as u32 * 3300) >> 12) as u16
    }
}

#[inline]
fn wait_on_window() {
    // Wait until CNT is inside the PWM-ON window (high-side conducting,
    // CNT < CCR), a settle margin past the CNT=0 switching edge. In the ON
    // window the star point sits at VM/2, so the floating phase = VM/2 + BEMF
    // and swings around mid-rail (the OFF/freewheel window pins the star at
    // GND and clips the negative BEMF half). Main context — free to hunt a
    // whole period (unlike the ISR).
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        let ccr = tim.ccr1().read().bits();
        let lo = 48u32; // ~0.75 us past the ON edge
        let hi = ccr.saturating_sub(40); // finish before the CNT=CCR edge
        loop {
            let c = tim.cnt().read().bits();
            if c >= lo && c <= hi {
                return;
            }
        }
    }
}

fn step_regs(high: u8, low: u8) -> (u32, u32, u32) {
    let role = |ch: u8| -> (u32, u32, u32) {
        if ch == high {
            (0b110, 1, 0)
        } else if ch == low {
            (0b100, 1, 1)
        } else {
            (0b100, 0, 0)
        }
    };
    let (m1, e1, n1) = role(0);
    let (m2, e2, n2) = role(1);
    let (m3, e3, n3) = role(2);
    let ccmr1 = (1 << 3) | (m1 << 4) | (1 << 11) | (m2 << 12);
    let ccmr2 = (1 << 3) | (m3 << 4);
    let ccer = e1 | (n1 << 2) | (e2 << 4) | (n2 << 6) | (e3 << 8) | (n3 << 10);
    (ccmr1, ccmr2, ccer)
}

fn commutate(step: usize, duty: u32) {
    let (high, low, _f) = STEPS[step];
    let (ccmr1, ccmr2, ccer) = step_regs(high, low);
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.ccr1().write(|w| w.bits(duty));
        tim.ccr2().write(|w| w.bits(duty));
        tim.ccr3().write(|w| w.bits(duty));
        tim.ccmr1_output().write(|w| w.bits(ccmr1));
        tim.ccmr2_output().write(|w| w.bits(ccmr2));
        tim.ccer().write(|w| w.bits(ccer));
        tim.egr().write(|w| w.bits(1));
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

    let _v1 = gpiob.pb1.into_analog();
    let _v2 = gpiob.pb0.into_analog();
    let _v3 = gpiob.pb2.into_analog();
    let _bus = gpioa.pa1.into_analog();
    let _is = gpiob.pb11.into_analog();

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

    // Minimal ADC, long sampling (160.5 cyc) for the high-Z floating phase.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20) | (1 << 11)));
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30));
        adc.cr().write(|w| w.bits(1 << 28));
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31)));
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b011)); // 19.5 cyc (~1.2 us) — fits the ON window
        adc.cfgr1().write(|w| w.bits(0));
        adc.isr().write(|w| w.bits(1));
        adc.cr().modify(|r, w| w.bits(r.bits() | 1));
        while adc.isr().read().bits() & 1 == 0 {}
    }

    let duty = (ARR + 1) * DUTY_PCT / 100;
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(ARR));
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32));
        tim.egr().write(|w| w.bits(1));
        tim.cr1().write(|w| w.bits(0x81));
    }
    commutate(0, 0);
    en.set_high().ok();
    delay.delay(2.millis());
    unsafe {
        (&*stm32::TIM1::ptr())
            .bdtr()
            .modify(|r, w| w.bits(r.bits() | (1 << 15)));
    }
    delay.delay(20.millis());

    let is_base = adc_read(15);
    let bus_base = adc_read(1); // boot bus (pin mV); sag floor = 75%
    let bus_floor = bus_base * 3 / 4;
    rprintln!(
        "sixstep-bemf: duty {}%, IS base {} mV, bus {} floor {}; ON-window BEMF",
        DUTY_PCT,
        is_base,
        bus_base,
        bus_floor
    );
    let _ = writeln!(serial, "BEMF,start,{},{}", DUTY_PCT, is_base);

    // Forced six-step ramp into the synced band. Per step, take N OFF-window
    // samples of the floating phase + the neutral (mean of 3 VPH), and
    // stream floating-vs-neutral so the host sees the BEMF ramp/ZC.
    let mut step: usize = 0;
    let mut step_us: u32 = 20_000;
    let total_steps: u32 = 900;
    let samples_per_step = 12usize;

    for n in 0..total_steps {
        commutate(step, duty);
        let float_ph = STEPS[step].2 as usize;
        let slot_us = step_us / samples_per_step as u32;
        for s in 0..samples_per_step {
            // ON-window synchronized read of the floating phase.
            wait_on_window();
            let vf = adc_read(VPH_CH[float_ph]);
            // Two neutral candidates, both ON-window: (1) 3-phase mean, and
            // (2) VM/2 mapped through the divider (the mzhal scheme). A clean
            // ZC = vf crossing the neutral ~30 deg before commutation.
            wait_on_window();
            let va = adc_read(9);
            wait_on_window();
            let vb = adc_read(8);
            wait_on_window();
            let vc = adc_read(10);
            let mean = ((va as u32 + vb as u32 + vc as u32) / 3) as u16;
            let vm_neu = (adc_read(1) as u32 * 561 / 1000) as u16; // bus/2 in VPH-pin domain
            // Current abort (ON-window IS = active PULSE current, high; the
            // PSU 1.5 A limit is the real backstop, this catches a cap-dump
            // transient only). ~300 mV delta ~ 5 A.
            let is = adc_read(15);
            if is.abs_diff(is_base) > 300 {
                stage::force_safe();
                let _ = writeln!(serial, "BEMF,abort,is,{}", is);
                rprintln!("BEMF abort: IS {} vs base {}", is, is_base);
                loop {
                    cortex_m::asm::nop();
                }
            }
            // Bus-sag abort: a stalled/desynced rotor on the current-limited
            // PSU drops into CC mode and sags the bus (the real envelope edge
            // here, not the IS peak — see the supply-wall scar).
            let bus = adc_read(1);
            if bus < bus_floor {
                stage::force_safe();
                let _ = writeln!(serial, "BEMF,abort,sag,{}", bus);
                rprintln!("BEMF abort: bus {} < floor {}", bus, bus_floor);
                loop {
                    cortex_m::asm::nop();
                }
            }
            // Stream a subset (every few steps) to keep the log readable.
            if n % 6 == 0 {
                let _ = writeln!(
                    serial,
                    "BW,{},{},{},{},{},{},{}",
                    n, step, float_ph, s, vf, mean, vm_neu
                );
                while serial.flush().is_err() {}
            }
            // consume the rest of the slot
            if slot_us > 60 {
                delay.delay((slot_us - 60).micros());
            }
        }
        step = (step + 1) % 6;
        if step_us > 2500 {
            step_us -= 50;
        }
    }

    stage::force_safe();
    let _ = writeln!(serial, "BEMF,done,{}", total_steps);
    rprintln!("sixstep-bemf done: {} steps, stage safed", total_steps);
    loop {
        cortex_m::asm::nop();
    }
}
