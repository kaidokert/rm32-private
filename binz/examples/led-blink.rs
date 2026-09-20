//! First-wire life probe for the BOOSTXL-DRV8304H: blink the board's user LED.
//!
//! Wiring so far (operator, this session): only 3.3 V, GND, and the booster's
//! **LED pin (J4-15)** -> **PD1** (Nucleo morpho CN7-10). No motor power, no gates.
//!
//! This toggles PD1 as a push-pull output on a 1 second interval (HIGH 1 s, LOW
//! 1 s). The DRV8304H EVM LED lights either when the pin is high or low depending
//! on its drive polarity (unconfirmed from the schematic) -- either way it blinks
//! once every 2 s, proving the PD1<->J4-15 wire + GPIOD are good.
//!
//! Run: cargo run --release --example led-blink   (watch RTT for the heartbeat)

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll()); // 64 MHz
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpiod = dp.GPIOD.split(&mut rcc);
    let mut led = gpiod.pd1.into_push_pull_output();

    rprintln!("led-blink: PD1 (CN7-10) -> J4-15 LED, 1 s interval");

    let mut on = false;
    loop {
        on = !on;
        if on {
            led.set_high().ok();
        } else {
            led.set_low().ok();
        }
        rprintln!("PD1 = {}", if on { "HIGH" } else { "LOW" });
        delay.delay(1000.millis());
    }
}
