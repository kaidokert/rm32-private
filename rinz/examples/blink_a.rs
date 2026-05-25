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
    rtt_target::rprintln!("blink_a start");

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain().freeze();
    let mut rcc = dp.RCC.freeze(rcc::Config::hsi(), pwr);
    let mut delay = cp.SYST.delay(&rcc.clocks);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);

    // Phase A: high-side = PA10 (TIM1_CH3), low-side = PB1 (TIM1_CH3N)
    // Low-sides are PA7/PB0/PB1 — NOT PC13/PA12/PB15 (those aren't routed to L6387)
    // LIN is active-high: PB1 HIGH = low-side MOSFET on
    let mut phase_a_hi = gpioa.pa10.into_push_pull_output();
    let mut phase_a_lo = gpiob.pb1.into_push_pull_output();

    // High-side stays off the whole time — just pulsing the low-side
    phase_a_hi.set_low();

    let mut n: u32 = 0;
    loop {
        phase_a_lo.set_high();
        delay.delay_ms(50u32); // 5 % on
        phase_a_lo.set_low();
        delay.delay_ms(950u32); // 95 % off
        rtt_target::rprintln!("pulse {}", n);
        n += 1;
    }
}
