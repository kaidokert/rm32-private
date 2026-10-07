//! Bench UART command input on G071 (binz bench) — same API as the L431
//! `bench_uart` module so the shared main loop is MCU-agnostic. Bytes
//! arrive via `bench_serial`'s USART3 RX DMA ring (PC11, 115200).

#![cfg(feature = "benchuart")]

use core::sync::atomic::{AtomicU16, AtomicUsize};

use rm32::bench_input::RxRing;

const RING_N: usize = 256;
static RING: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);

/// The shared RX ring. Producer = `drain_dma` (main), consumer = main.
#[inline]
pub fn ring() -> RxRing<'static, RING_N> {
    RxRing {
        ring: &RING,
        head: &HEAD,
        tail: &TAIL,
    }
}

/// USART3 is brought up (TX + RX DMA) by `debug_uart::init()`; nothing
/// further to do here.
pub fn init() {}

pub fn drain_dma() {
    let rx = ring();
    super::bench_serial::drain(|b| {
        rx.push(b as u16);
    });
}

/// RX corruption counter (overrun/framing/noise).
pub fn ore_count() -> u32 {
    super::bench_serial::err_count()
}
