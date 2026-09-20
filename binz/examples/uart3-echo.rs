//! USART3 echo — validates the BIDIRECTIONAL relocated VCOM (RX on PC11 AND
//! TX on PC10) and that the host can talk TO the board, not just listen.
//!
//! Each received byte is echoed back UPPERCASED (a->A). Uppercasing (vs a raw
//! echo) proves the MCU actually processed the byte — a wiring/adapter loopback
//! would return it unchanged. Send "ping" -> receive "PING".
//!
//! Wiring: USB-TTL TX -> PC11 (USART3_RX); USB-TTL RX <- PC10 (USART3_TX); GND.
//! 115200 8N1. Run: cargo run --release --example uart3-echo

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
    let mut rcc = dp.RCC.freeze(Config::pll()); // 64 MHz

    let gpioc = dp.GPIOC.split(&mut rcc);

    // USART3 on PC10 (TX) / PC11 (RX), 115200 8N1.
    let mut serial = dp
        .USART3
        .usart(
            (gpioc.pc10, gpioc.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .unwrap();

    let _ = writeln!(serial, "uart3-echo ready (send text; it echoes UPPERCASED)");
    while serial.flush().is_err() {}
    rprintln!("uart3-echo: ready on USART3 PC10/PC11 @115200");

    loop {
        // read() clears PE/FE/NE/ORE flags internally and returns WouldBlock
        // when idle, so a bare `if let Ok` is safe and can't lock up.
        if let Ok(b) = serial.read() {
            let out = if b.is_ascii_lowercase() { b - 32 } else { b };
            while serial.write(out).is_err() {}
            rprintln!("rx {:#04x} -> tx {:#04x}", b, out);
        }
    }
}
