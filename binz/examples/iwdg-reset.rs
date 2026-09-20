//! Independent watchdog life-probe, self-verifying across one reset:
//! on boot it decodes RCC.CSR reset-cause flags. If IWDGRSTF is set, the
//! previous run's un-fed watchdog really did reset the chip -> PASS, idle.
//! Otherwise it arms the IWDG (~500 ms), feeds it 4 times to prove feeding
//! holds the dog off, then stops feeding and lets it bite.
//!
//! Run: `cargo run --release --example iwdg-reset` (watch two boots)

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::watchdog::IWDGExt;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    // Reset-cause flags (RM0444 RCC_CSR[31:25]), read before anything else.
    let csr = dp.RCC.csr().read().bits();
    let iwdg_reset = csr & (1 << 29) != 0;
    rprintln!(
        "iwdg-reset: CSR={:#010x} lpwr={} wwdg={} iwdg={} sft={} pwr={} pin={} obl={}",
        csr,
        (csr >> 31) & 1,
        (csr >> 30) & 1,
        (csr >> 29) & 1,
        (csr >> 28) & 1,
        (csr >> 27) & 1,
        (csr >> 26) & 1,
        (csr >> 25) & 1
    );
    dp.RCC.csr().modify(|_, w| w.rmvf().set_bit()); // clear flags

    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    if iwdg_reset {
        rprintln!("IWDG RESET CONFIRMED -- watchdog ALIVE - PASS");
        loop {
            delay.delay(5000.millis());
            rprintln!("idle (watchdog already proven)");
        }
    }

    let mut wdg = dp.IWDG.constrain();
    wdg.start(500.millis());
    rprintln!("IWDG armed @500 ms; feeding 4x to prove feed path...");
    for i in 0..4 {
        delay.delay(300.millis());
        wdg.feed();
        rprintln!("fed #{} (300 ms in, still alive)", i + 1);
    }
    rprintln!("now STARVING the dog -- expect reset in ~500 ms");
    let mut n = 0u32;
    loop {
        delay.delay(100.millis());
        n += 1;
        rprintln!("starving {} ms", n * 100);
    }
}
