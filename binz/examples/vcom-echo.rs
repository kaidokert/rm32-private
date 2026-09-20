//! Full-duplex VCOM echo on the NUCLEO-G071RB ST-Link V2-1 virtual COM port.
//!
//! USART2 PA2 (TX) / PA3 (RX) -> on-board ST-Link VCP. 64 MHz sysclk,
//! 2000000 8N1 (64 MHz/32 exact; V2-1 bridge verified lossless at 16 KiB
//! full-duplex blast, ~109 kB/s sustained -- the USB FS bridge saturates
//! around 110 kB/s, so higher baud gains nothing). Sends a hello banner,
//! then echoes every received byte back; activity mirrored to RTT.
//!
//! Run: `cargo run --release --example vcom-echo`

#![no_std]
#![no_main]

use binz as _; // panic handler
use core::fmt::Write;
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::Config;
use stm32g0xx_hal::serial::FullConfig;
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    // HSI16 -> PLL (M=1, N=8, R=2) -> 64 MHz sysclk, the G071 maximum.
    let mut rcc = dp.RCC.freeze(Config::pll());
    let gpioa = dp.GPIOA.split(&mut rcc);

    // 8-deep TX/RX hardware FIFOs: without them the 1-byte RDR overruns
    // during any >11 us stall at 921600 (measured: bursts of RX Overrun).
    let mut serial = dp
        .USART2
        .usart(
            (gpioa.pa2, gpioa.pa3),
            FullConfig::default()
                .baudrate(2_000_000.bps())
                .fifo_enable(),
            &mut rcc,
        )
        .unwrap();

    writeln!(
        serial,
        "hello from binz on NUCLEO-G071RB (64 MHz, 2000000 8N1)"
    )
    .ok();
    rprintln!(
        "vcom-echo: USART2 @ 2000000, sys_clk={} Hz -- echoing",
        rcc.clocks.sys_clk.raw()
    );

    // Lossless full-duplex echo: never block on TX while RX bytes are
    // pending. Software ring bridges RX -> TX; RX is always drained first.
    const N: usize = 1024;
    let mut ring = [0u8; N];
    let mut head: usize = 0;
    let mut tail: usize = 0;
    let mut rx_count: u32 = 0;
    let mut err_count: u32 = 0;
    loop {
        // Drain RX while data and ring space are available.
        while (head + 1) % N != tail {
            match serial.read() {
                Ok(b) => {
                    ring[head] = b;
                    head = (head + 1) % N;
                    rx_count += 1;
                    if rx_count % 65536 == 0 {
                        rprintln!("echoed {} bytes ({} errors)", rx_count, err_count);
                    }
                }
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(e)) => {
                    err_count += 1;
                    if err_count <= 16 || err_count % 256 == 0 {
                        rprintln!("uart error #{}: {:?}", err_count, e);
                    }
                }
            }
        }
        // Pump pending bytes into the TX FIFO without blocking.
        while tail != head {
            match serial.write(ring[tail]) {
                Ok(()) => tail = (tail + 1) % N,
                Err(nb::Error::WouldBlock) => break,
                Err(nb::Error::Other(_)) => break,
            }
        }
    }
}
