//! Antiphase LED blink: Nucleo user LED LD4 (PA5, via SB22) and the DRV board
//! LED (PB5 -> J4-15) driven OPPOSITE — one on while the other is off, swapping
//! every 500 ms. Pure GPIO, no motor drive. Confirms both LED wires at once.
//!
//! Run: cargo run --release --example led-alt

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
    let gpiob = dp.GPIOB.split(&mut rcc);
    let mut ld4 = gpioa.pa5.into_push_pull_output(); // Nucleo LD4
    let mut drv_led = gpiob.pb5.into_push_pull_output(); // DRV J4-15 LED

    rprintln!("led-alt: LD4(PA5) vs DRV LED(PB5) antiphase, 500 ms");

    let mut phase = false;
    loop {
        phase = !phase;
        if phase {
            ld4.set_high().ok();
            drv_led.set_low().ok();
        } else {
            ld4.set_low().ok();
            drv_led.set_high().ok();
        }
        delay.delay(500.millis());
    }
}
