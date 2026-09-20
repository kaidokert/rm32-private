//! Gate-drive path test for the IHM08M1 + G071RB — drive ONE phase at a time
//! (others floating, so no current path -> zero current, safe), and watch that
//! phase's own voltage on the ADC. If driving a phase HIGH pulls its node toward
//! the bus and LOW pulls it to ~0, the L6398 + FET for that leg switch correctly.
//!
//! Measurable phases: C on PA2 (BEMF3, IN2), B on PB11 (BEMF2, IN15). Phase A
//! (PC3) is not a G071 ADC pin — inferred. Current on PA0 (IN0) should stay ~0
//! (no return path). Reports over USART3/VCOM; read via the serial MCP.
//!
//! Bootstrap: each high-side test first drives that phase LOW ~20 ms to charge
//! the L6398 BOOT cap, then HIGH for a few ms (bootstrap holds) to measure.
//!
//! Run: cargo run --release --example gate-test

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m::asm::delay as cycdelay;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

const INH_BIT: [u32; 3] = [8, 9, 10]; // PA8/PA9/PA10 (all GPIOA)
// low: phase0 PA7 (GPIOA), phase1 PB0, phase2 PB1 (GPIOB)
const VM_DIV: u32 = 1915;

struct Ch(u8);
impl Channel<Adc> for Ch {
    type ID = u8;
    fn channel() -> u8 {
        0
    }
}
// The HAL read() is generic over a PIN: Channel; use per-channel marker types.
macro_rules! chan {
    ($name:ident, $n:literal) => {
        struct $name;
        impl Channel<Adc> for $name {
            type ID = u8;
            fn channel() -> u8 {
                $n
            }
        }
    };
}
chan!(ChI, 0); // PA0 current
chan!(ChBus, 1); // PA1 bus
chan!(ChC, 2); // PA2 phase C (BEMF3)
chan!(ChB, 15); // PB11 phase B (BEMF2)

fn all_off() {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pb = &*stm32::GPIOB::ptr();
        pa.bsrr().write(|w| {
            w.bits((1 << (7 + 16)) | (1 << (8 + 16)) | (1 << (9 + 16)) | (1 << (10 + 16)))
        });
        pb.bsrr()
            .write(|w| w.bits((1 << (0 + 16)) | (1 << (1 + 16))));
    }
}

fn low_bit(k: usize, on: bool) {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let pb = &*stm32::GPIOB::ptr();
        let shift = |bit: u32| if on { bit } else { bit + 16 };
        match k {
            0 => pa.bsrr().write(|w| w.bits(1 << shift(7))),
            1 => pb.bsrr().write(|w| w.bits(1 << shift(0))),
            _ => pb.bsrr().write(|w| w.bits(1 << shift(1))),
        }
    }
}

fn high_bit(k: usize, on: bool) {
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        let bit = INH_BIT[k];
        pa.bsrr()
            .write(|w| w.bits(1 << if on { bit } else { bit + 16 }));
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

    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    let _ = (
        gpioa.pa0.into_analog(),
        gpioa.pa1.into_analog(),
        gpioa.pa2.into_analog(),
        gpiob.pb11.into_analog(),
    );
    let _ = (
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
    );
    all_off();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();
    let _ = Ch(0);

    let vm = adc.read_voltage(&mut ChBus).unwrap() as u32 * VM_DIV / 100;
    let _ = writeln!(
        serial,
        "\r\ngate-test: VM={}mV. Drive each phase HIGH (others float), read its node.",
        vm
    );
    while serial.flush().is_err() {}
    rprintln!("gate-test VM={}", vm);

    let names = ["A(PC3-noADC)", "B(PB11)", "C(PA2)"];
    loop {
        for k in 0..3usize {
            // Charge bootstrap: phase low ~20 ms.
            all_off();
            low_bit(k, true);
            delay.delay(20.millis());
            let c_lo = adc.read_voltage(&mut ChC).unwrap();
            let b_lo = adc.read_voltage(&mut ChB).unwrap();
            // Drive HIGH: low off, 1 us gap, high on. Hold 3 ms, measure.
            low_bit(k, false);
            cycdelay(64 * 2);
            high_bit(k, true);
            delay.delay(3.millis());
            let c_hi = adc.read_voltage(&mut ChC).unwrap();
            let b_hi = adc.read_voltage(&mut ChB).unwrap();
            let i = adc.read_voltage(&mut ChI).unwrap();
            all_off();
            let _ = writeln!(
                serial,
                "phase {}: PA2 {}->{}mV | PB11 {}->{}mV | I={}mV (expect the driven leg's node to jump toward VM/5.5)",
                names[k], c_lo, c_hi, b_lo, b_hi, i
            );
            while serial.flush().is_err() {}
            delay.delay(150.millis());
        }
        let _ = writeln!(serial, "---");
        while serial.flush().is_err() {}
        delay.delay(500.millis());
    }
}
