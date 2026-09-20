//! Standalone ADC bringup check: prove I can sample a phase voltage
//! SYNCHRONIZED to the PWM cycle (the primitive BEMF sensing needs).
//!
//! No commutation, no harvest DMA — a minimal software-triggered ADC I
//! fully time by polling TIM1 CNT. Drive phase A high at a fixed duty
//! (A=PWM, B=low, C=float), then sample each phase at a controlled point:
//!   - ON  window (CNT well below the compare) — high FET conducting
//!   - OFF window (CNT well above the compare) — high FET off
//! If synchronized sampling works, the DRIVEN phase A reads ~VM in the ON
//! window and ~0 in the OFF window, cleanly and repeatably; the de-cohered
//! sampler could never separate these. The FLOATING phase C in the OFF
//! window is where BEMF will live.
//!
//! Run: `cargo run --release --example adc-sync-check`

#![no_std]
#![no_main]

use binz::stage;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

const CARRIER_HZ: u32 = 20_000;
const ARR: u32 = 64_000_000 / CARRIER_HZ - 1; // 3199
const DTG: u8 = 26;
const DUTY_PCT: u32 = 5; // static energization = stall heater; keep low.
// ON-window sample reads VM regardless of duty.

/// Minimal ADC: single conversion, software trigger, one channel at a time.
/// 160.5-cycle sampling (high-Z VPH dividers). Returns pin mV.
fn adc_read(ch: u8) -> u16 {
    unsafe {
        let adc = &*stm32::ADC::ptr();
        adc.chselr0().write(|w| w.bits(1 << ch));
        adc.isr().write(|w| w.bits(1 << 3)); // clear EOC
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2))); // ADSTART
        while adc.isr().read().bits() & (1 << 3) == 0 {}
        let raw = adc.dr().read().bits() as u32;
        ((raw * 3300) >> 12) as u16
    }
}

/// Busy-wait until TIM1 CNT is in [lo, hi] (up-counting window).
#[inline]
fn wait_cnt_window(lo: u32, hi: u32) {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        loop {
            let c = tim.cnt().read().bits();
            if c >= lo && c <= hi {
                return;
            }
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

    let _v1 = gpiob.pb1.into_analog(); // phase A = IN9
    let _v2 = gpiob.pb0.into_analog(); // phase B = IN8
    let _v3 = gpiob.pb2.into_analog(); // phase C = IN10
    let _bus = gpioa.pa1.into_analog(); // VM = IN1

    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    // Drive pins AF2.
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

    // Minimal ADC: PCLK/4, regulate, calibrate, 160.5-cyc sampling, single
    // conversion, software trigger.
    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20) | (1 << 11))); // ADCEN, TIM1EN
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30));
        adc.cr().write(|w| w.bits(1 << 28)); // ADVREGEN
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31))); // ADCAL
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b010)); // 12.5 cyc (short, for ON-window confirm)
        adc.cfgr1().write(|w| w.bits(0)); // single, software trigger
        adc.isr().write(|w| w.bits(1));
        adc.cr().modify(|r, w| w.bits(r.bits() | 1)); // ADEN
        while adc.isr().read().bits() & 1 == 0 {}
    }

    // TIM1: phase A (CH1) PWM at DUTY, phase B (CH2) low, phase C (CH3)
    // float. Complementary + dead time + OSSR/OSSI.
    let duty = (ARR + 1) * DUTY_PCT / 100;
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.cr1().write(|w| w.bits(0));
        tim.psc().write(|w| w.bits(0));
        tim.arr().write(|w| w.bits(ARR));
        tim.ccr1().write(|w| w.bits(duty)); // A high-side PWM
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        // CH1 = PWM1 (A high), CH2 = force-inactive + CCxNE (B low on),
        // CH3 = disabled (C float).
        tim.ccmr1_output()
            .write(|w| w.bits((1 << 3) | (0b110 << 4) | (1 << 11) | (0b100 << 12)));
        tim.ccmr2_output()
            .write(|w| w.bits((1 << 3) | (0b100 << 4)));
        tim.ccer().write(|w| w.bits(1 | (1 << 4) | (1 << 6))); // CC1E, CC2E, CC2NE
        tim.bdtr()
            .write(|w| w.bits((1 << 11) | (1 << 10) | DTG as u32));
        tim.egr().write(|w| w.bits(1));
        tim.cr1().write(|w| w.bits(0x81)); // ARPE|CEN
    }

    rprintln!(
        "adc-sync-check: A=PWM {}%, B=low, C=float; duty count={}/{}",
        DUTY_PCT,
        duty,
        ARR
    );
    en.set_high().ok();
    delay.delay(2.millis());
    unsafe {
        (&*stm32::TIM1::ptr())
            .bdtr()
            .modify(|r, w| w.bits(r.bits() | (1 << 15))); // MOE
    }
    delay.delay(20.millis());

    // Sample phase A (ch9), B (ch8), C (ch10) in the ON window (CNT small,
    // < duty) vs the OFF window (CNT > duty, before the wrap). Average a
    // few to beat noise. Expected: A_on >> A_off (high FET switching).
    let on_lo = 40;
    let on_hi = (duty / 2).max(60);
    let off_lo = duty + 200;
    let off_hi = ARR - 100;

    // Baseline shunt (IS = ch15) before energizing meaningfully.
    let is_base = adc_read(15);
    for rep in 0..4 {
        // Current abort: static energization is a stall heater.
        let is = adc_read(15);
        if is.abs_diff(is_base) > 60 {
            stage::force_safe();
            rprintln!(
                "adc-sync-check ABORT: IS {} vs base {} (stall current)",
                is,
                is_base
            );
            loop {
                cortex_m::asm::nop();
            }
        }
        let mut a_on = 0u32;
        let mut a_off = 0u32;
        let mut c_off = 0u32;
        let mut b_off = 0u32;
        let n = 64u32;
        for _ in 0..n {
            wait_cnt_window(on_lo, on_hi);
            a_on += adc_read(9) as u32; // A in ON window
            wait_cnt_window(off_lo, off_hi);
            a_off += adc_read(9) as u32; // A in OFF window
            wait_cnt_window(off_lo, off_hi);
            b_off += adc_read(8) as u32; // B (low) in OFF
            wait_cnt_window(off_lo, off_hi);
            c_off += adc_read(10) as u32; // C (float) in OFF
        }
        rprintln!(
            "rep{}: A_on={} A_off={} mV(pin)  B_off={} C_off={}  (A_on>>A_off => sync works)",
            rep,
            a_on / n,
            a_off / n,
            b_off / n,
            c_off / n
        );
        delay.delay(200.millis());
    }

    stage::force_safe();
    rprintln!("adc-sync-check done, stage safed");
    loop {
        cortex_m::asm::nop();
    }
}
