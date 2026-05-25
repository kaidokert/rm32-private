#![no_std]
#![no_main]

use cortex_m_rt::entry;
use hal::prelude::*;
use hal::pwr::PwrExt;
use hal::{rcc, stm32};
use rinz::hal;

#[entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();

    let dp = stm32::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain().freeze();
    let mut rcc = dp.RCC.freeze(rcc::Config::hsi(), pwr);

    let gpioc = dp.GPIOC.split(&mut rcc);
    let mut led = gpioc.pc6.into_push_pull_output();

    let mut n: u32 = 0;
    loop {
        led.toggle();
        rtt_target::rprintln!("blink {}", n);
        n += 1;
        cortex_m::asm::delay(8_000_000); // ~1 s at 8 MHz HSI
    }
}
