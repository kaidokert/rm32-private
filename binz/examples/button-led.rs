//! Human-visible GPIO life-probe: LD4 (PA5) blinks at 2 Hz; user button
//! B1 (PC13) is polled -- pressing it holds the LED on solid and logs
//! edges over RTT.
//!
//! Run: `cargo run --release --example button-led`

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
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let mut led = gpioa.pa5.into_push_pull_output();
    let mut button = gpioc.pc13.into_pull_up_input();

    rprintln!("button-led: LD4 blinking on PA5; press B1 (PC13) to hold solid");
    let mut was_pressed = false;
    let mut phase = false;
    loop {
        let pressed = button.is_low().unwrap();
        if pressed != was_pressed {
            rprintln!("button B1 {}", if pressed { "PRESSED" } else { "released" });
            was_pressed = pressed;
        }
        if pressed {
            led.set_high().ok();
        } else {
            phase = !phase;
            if phase {
                led.set_high().ok();
            } else {
                led.set_low().ok();
            }
        }
        delay.delay(250.millis());
    }
}
