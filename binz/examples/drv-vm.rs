//! Passive VM (bus voltage) read for the DRV8304 rig. First feedback channel.
//! VSENVM (J5-3, the board's 82k/7.5k bus divider) -> PA6 = ADC_IN6.
//! VM = pin_mV * 11.94 (7.5k / (82k+7.5k) = 0.0838). No motor drive.
//!
//! Vrefint (IN13, cal @ 0x1FFF75AA) gives true VDDA so the mV are accurate.
//! Reports over USART3 (PC10) @115200. Run: cargo run --release --example drv-vm

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

const VREFINT_CAL_ADDR: u32 = 0x1FFF_75AA; // G0 factory cal @ VDDA=3.0 V

unsafe fn adc_read(ch: u8) -> u16 {
    let adc = &*stm32::ADC::ptr();
    adc.isr().write(|w| w.bits(1 << 13)); // clear CCRDY
    adc.chselr0().write(|w| w.bits(1 << ch)); // one channel
    while adc.isr().read().bits() & (1 << 13) == 0 {}
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
        rcc_raw.iopenr().modify(|r, w| w.bits(r.bits() | 0b1)); // GPIOA EN
        rcc_raw
            .apbenr2()
            .modify(|r, w| w.bits(r.bits() | (1 << 20))); // ADCEN

        // PA6 -> analog (MODER bits 12..13 = 0b11).
        let ga = &*stm32::GPIOA::ptr();
        ga.moder().modify(|r, w| w.bits(r.bits() | (0b11 << 12)));

        let adc = &*stm32::ADC::ptr();
        adc.cfgr2().write(|w| w.bits(0b10 << 30)); // CKMODE = PCLK/4
        adc.cr().write(|w| w.bits(1 << 28)); // ADVREGEN
        cortex_m::asm::delay(64 * 30);
        adc.cr().modify(|r, w| w.bits(r.bits() | (1 << 31))); // ADCAL
        while adc.cr().read().bits() & (1 << 31) != 0 {}
        cortex_m::asm::delay(64 * 5);
        adc.smpr().write(|w| w.bits(0b111)); // 160.5 cyc (high-Z divider)
        adc.ccr().modify(|r, w| w.bits(r.bits() | (1 << 22))); // VREFEN
        adc.isr().write(|w| w.bits(1)); // clear ADRDY
        adc.cr().modify(|r, w| w.bits(r.bits() | 1)); // ADEN
        while adc.isr().read().bits() & 1 == 0 {}
    }

    let vcal = unsafe { core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16) } as u32;
    let _ = writeln!(
        serial,
        "\r\ndrv-vm: VSENVM->PA6 (IN6), x11.94 bus scale. VREFINT_CAL={}",
        vcal
    );
    while serial.flush().is_err() {}
    rprintln!("drv-vm ready");

    loop {
        // Average 8 samples of each.
        let mut vref_acc = 0u32;
        let mut in6_acc = 0u32;
        for _ in 0..8 {
            vref_acc += unsafe { adc_read(13) } as u32;
            in6_acc += unsafe { adc_read(6) } as u32;
        }
        let vref = vref_acc / 8;
        let in6 = in6_acc / 8;
        let vdda = if vref > 0 { 3000 * vcal / vref } else { 0 };
        let pin_mv = in6 * vdda / 4096;
        let vm_mv = pin_mv * 1194 / 100;

        let _ = writeln!(
            serial,
            "VDDA={}mV | PA6={}mV (raw {}) | VM={}.{:02} V",
            vdda,
            pin_mv,
            in6,
            vm_mv / 1000,
            (vm_mv % 1000) / 10
        );
        while serial.flush().is_err() {}
        rprintln!(
            "VM={}.{:02}V PA6={}mV",
            vm_mv / 1000,
            (vm_mv % 1000) / 10,
            pin_mv
        );

        delay.delay(500.millis());
    }
}
