//! USART3 "hello" heartbeat — validates the RELOCATED VCOM after the IHM08M1
//! rework moved the bench serial off PA2/PA3 (now COMP2 BEMF) to USART3.
//!
//! Wiring: MCU PC10 = USART3_TX (AF0) -> USB-TTL RX; GND<->GND. (PC11 = RX,
//! not used here.) 1 Hz "hello N", 115200 8N1.
//!
//! Cross-check: RTT prints "sent hello N" too. If RTT shows the count climbing
//! but your terminal is silent, it's the wire/adapter/baud, not the firmware.
//!
//! Run: cargo run --release --example uart3-hello

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

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll()); // 64 MHz
    let mut delay = cp.SYST.delay(&mut rcc);

    let gpioc = dp.GPIOC.split(&mut rcc);

    // USART3 on PC10 (TX) / PC11 (RX), 115200 8N1. USART3 is a "basic" UART in
    // this HAL -> BasicConfig (no FIFO needed at 1 Hz).
    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    rprintln!("uart3-hello: USART3 TX on PC10 @115200 — 1 Hz");
    let mut n: u32 = 0;
    loop {
        let _ = writeln!(serial, "hello {}", n);
        while serial.flush().is_err() {}
        rprintln!("sent hello {}", n);
        n = n.wrapping_add(1);
        delay.delay(1000.millis());
    }
}
