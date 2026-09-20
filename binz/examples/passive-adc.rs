//! Passive ADC vitals for the IHM08M1 + G071RB rig — reads every WIRED channel
//! that is a valid G071 ADC input, no motor drive, dumps over the relocated
//! VCOM (USART3/PC10) once per second.
//!
//! G071 ADC channel map (PC2/PC3/PB13 are NOT ADC inputs on this MCU):
//!   IN0  PA0  = current (shunt amp, ~1.65 V mid-rail @ 0 A)
//!   IN1  PA1  = bus voltage (169K/9.31K divider, x19.15)
//!   IN2  PA2  = BEMF3 / phase-C  (COMP2 INM input; x5.5 divider)
//!   IN3  PA3  = neutral (3x47k star average; COMP2 INP)
//!   IN15 PB11 = BEMF2 / phase-B  (x5.5 divider)
//!   IN13 = Vrefint (internal) -> VDDA
//! GPIO_BEMF (PC9) driven LOW to enable the phase-sense dividers.
//!
//! Run: cargo run --release --example passive-adc

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

/// Factory Vrefint calibration (measured at VDDA = 3.0 V), G0 @ 0x1FFF75AA.
const VREFINT_CAL_ADDR: u32 = 0x1FFF_75AA;

unsafe fn adc_read(ch: u8) -> u16 {
    let adc = &*stm32::ADC::ptr();
    adc.isr().write(|w| w.bits(1 << 13)); // clear CCRDY
    adc.chselr0().write(|w| w.bits(1 << ch)); // bit-mapped: one channel
    while adc.isr().read().bits() & (1 << 13) == 0 {} // CCRDY
    adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 2))); // ADSTART
    while adc.isr().read().bits() & (1 << 2) == 0 {} // EOC
    adc.dr().read().bits() as u16
}

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioc = dp.GPIOC.split(&mut rcc);
    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    unsafe {
        let rcc_raw = &*stm32::RCC::ptr();
        // GPIOA/B/C clocks + ADC clock.
        rcc_raw.iopenr().modify(|r, w| w.bits(r.bits() | 0b111)); // IOPA/B/C EN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN

        // PA0/PA1/PA2/PA3 -> analog (MODER 0b11 each, bits 0..7).
        let ga = &*stm32::GPIOA::ptr();
        ga.moder().modify(|r, w| w.bits(r.bits() | 0xFF));
        // PB11 -> analog (MODER bits 22..23).
        let gb = &*stm32::GPIOB::ptr();
        gb.moder().modify(|r, w| w.bits(r.bits() | (0b11 << 22)));
        // PC9 (GPIO_BEMF) -> output LOW, enables the phase-sense dividers.
        let gc = &*stm32::GPIOC::ptr();
        gc.moder()
            .modify(|r, w| w.bits((r.bits() & !(0b11 << 18)) | (0b01 << 18)));
        gc.bsrr().write(|w| w.bits(1 << (9 + 16))); // BR9 = drive PC9 low

        // ADC: PCLK/4 clock, regulator, calibrate, long sample, Vrefint on.
        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30)); // CKMODE = PCLK/4 (16 MHz)
        adc.cr().write(|w| w.bits(1 << 28)); // ADVREGEN
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31))); // ADCAL
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b111)); // 160.5 cyc (high-Z dividers)
        adc.ccr().modify(|r, w| w.bits(r.bits() | (1 << 22))); // VREFEN (Vrefint)
        adc.isr().write(|w| w.bits(1)); // clear ADRDY
        adc.cr().modify(|r, w| w.bits(r.bits() | 1)); // ADEN
        while adc.isr().read().bits() & 1 == 0 {}
    }

    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;
    rprintln!("passive-adc: ready; VREFINT_CAL={}", vcal);
    let _ = writeln!(
        serial,
        "\r\n=== passive-adc (IHM08M1 + G071) ===  VREFINT_CAL={}",
        vcal
    );
    while serial.flush().is_err() {}

    loop {
        // Vrefint first -> VDDA (mV). VDDA = 3000 * cal / raw  (G0 cal @ 3.0 V).
        let vref = unsafe { adc_read(13) } as u32;
        let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };

        let i_raw = unsafe { adc_read(0) };
        let bus_raw = unsafe { adc_read(1) };
        let b3_raw = unsafe { adc_read(2) }; // PA2 = BEMF3/phaseC
        let neu_raw = unsafe { adc_read(3) }; // PA3 = neutral
        let b2_raw = unsafe { adc_read(15) }; // PB11 = BEMF2/phaseB

        // pin mV using measured VDDA.
        let mv = |raw: u16| -> u32 { raw as u32 * vdda / 4096 };
        let i_mv = mv(i_raw);
        let bus_mv = mv(bus_raw);
        let bus_v10 = bus_mv * 1915 / 100 / 100; // x19.15 -> centi-volts... keep mV*19.15
        let bus_real_mv = bus_mv * 1915 / 100;
        let _ = bus_v10;

        let _ = writeln!(
            serial,
            "VDDA={}mV | I(PA0)={}mV r{} | BUS(PA1)={}mV->{}mV r{} | BEMF3(PA2)={}mV r{} | NEU(PA3)={}mV r{} | BEMF2(PB11)={}mV r{}",
            vdda,
            i_mv,
            i_raw,
            bus_mv,
            bus_real_mv,
            bus_raw,
            mv(b3_raw),
            b3_raw,
            mv(neu_raw),
            neu_raw,
            mv(b2_raw),
            b2_raw,
        );
        while serial.flush().is_err() {}
        rprintln!(
            "VDDA={} I={} BUS={}(->{}) B3={} NEU={} B2={}",
            vdda,
            i_mv,
            bus_mv,
            bus_real_mv,
            mv(b3_raw),
            mv(neu_raw),
            mv(b2_raw)
        );

        delay.delay(1000.millis());
    }
}
