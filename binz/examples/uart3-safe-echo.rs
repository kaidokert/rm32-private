//! Motor-inert USART3 diagnostic for the DRV8304/G071 bench.
//!
//! Before enabling USART3, this image makes all six gate-command GPIOs and
//! DRV ENABLE (PD1) push-pull outputs driven low. It never configures PWM,
//! timers, ADC, comparator, or any motor-control interrupt. Received lowercase
//! ASCII is echoed uppercased, proving the MCU processed the byte rather than
//! an adapter loopback returning it unchanged.
//!
//! Wiring: USB-TTL TX -> PC11, USB-TTL RX <- PC10, common GND; 115200 8N1.

#![no_std]
#![no_main]

use binz as _;
use core::fmt::Write;
use cortex_m_rt::entry;
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::serial::BasicConfig;
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    let dp = stm32::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    // Configure the command pins as GPIO outputs, never timer alternate
    // functions. Clear them atomically immediately after mode selection.
    let _gate_pins = (
        gpioa.pa7.into_push_pull_output(),
        gpioa.pa8.into_push_pull_output(),
        gpioa.pa9.into_push_pull_output(),
        gpioa.pa10.into_push_pull_output(),
        gpiob.pb0.into_push_pull_output(),
        gpiob.pb1.into_push_pull_output(),
        gpiod.pd1.into_push_pull_output(),
    );
    let mut nfault = gpiob.pb14.into_floating_input();
    unsafe {
        (*stm32::GPIOA::ptr())
            .bsrr()
            .write(|w| w.bits(((1 << 7) | (1 << 8) | (1 << 9) | (1 << 10)) << 16));
        (*stm32::GPIOB::ptr())
            .bsrr()
            .write(|w| w.bits(((1 << 0) | (1 << 1)) << 16));
        (*stm32::GPIOD::ptr())
            .bsrr()
            .write(|w| w.bits((1 << 1) << 16));
    }

    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    let _ = writeln!(
        serial,
        "SAFE_UART_READY gates=0 en=0 pwm=absent nflt={}",
        if nfault.is_high().unwrap_or(false) {
            1
        } else {
            0
        }
    );
    while serial.flush().is_err() {}

    loop {
        if let Ok(byte) = serial.read() {
            let echo = if byte.is_ascii_lowercase() {
                byte - (b'a' - b'A')
            } else {
                byte
            };
            while serial.write(echo).is_err() {}
        }
    }
}
