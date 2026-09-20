//! RTT hello on the NUCLEO-G071RB: PLL to 64 MHz, banner + 1 Hz heartbeat.
//!
//! Run: `cargo run --release --example rtt-hello`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    // HSI16 -> PLL (M=1, N=8, R=2) -> 64 MHz sysclk, the G071 maximum.
    let rcc = dp.RCC.freeze(Config::pll());

    rprintln!(
        "rtt-hello: NUCLEO-G071RB up, sys_clk={} Hz ahb={} apb={}",
        rcc.clocks.sys_clk.raw(),
        rcc.clocks.ahb_clk.raw(),
        rcc.clocks.apb_clk.raw()
    );

    let mut n: u32 = 0;
    loop {
        rprintln!("hello #{}", n);
        n += 1;
        // ~1 s busy-wait (no WFI on this bench: it breaks RTT). asm::delay
        // runs ~3 cycles/count here (M0+ flash wait states), hence /3.
        cortex_m::asm::delay(21_000_000);
    }
}
