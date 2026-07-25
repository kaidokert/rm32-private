//! Bench-debug UART on the existing AM32 KISS telemetry pin.
//!
//! L431: USART1 TX on PB6 (AF7), 115200 8N1, TX-only (no RX, no DMA).
//! Polled — busy-waits on TXE for each byte. Fine for debug log lines, would
//! be terrible for any latency-sensitive ISR path. **Only use under
//! `cfg(feature = "debuguart")`.**
//!
//! When the feature is enabled, normal AM32-style telemetry is unavailable
//! (this module owns USART1). When disabled, this entire module compiles to
//! nothing and the telemetry HAL keeps PB6.
//!
//! Use the `dprintln!` macro to log; it always RTT-prints, and when this
//! feature is on, also pushes the same text out USART1.

#![cfg(feature = "debuguart")]

use core::fmt::{self, Write};

use crate::pac::{GPIOB, RCC, USART1};

/// CPU clock used for BRR computation. Must match the actual SYSCLK once
/// `init()` runs. L431 production clock is 80 MHz.
const CPU_HZ: u32 = 80_000_000;
#[cfg(not(feature = "benchuart"))]
const BAUD: u32 = 115_200;
/// benchuart: 2 Mbaud so the minz bench toolchain reads both directions
/// on one port. Needs push-pull PB6 (set in init below) — the telemetry
/// init's open-drain + pull-up rise time caps the line at ~115200.
#[cfg(feature = "benchuart")]
const BAUD: u32 = 2_000_000;

pub fn init() {
    unsafe {
        let rcc = &*RCC::ptr();
        // GPIOB clock (likely already on, idempotent)
        rcc.ahb2enr.modify(|_, w| w.gpioben().set_bit());
        // USART1 clock (APB2)
        rcc.apb2enr.modify(|_, w| w.usart1en().set_bit());

        let gpiob = &*GPIOB::ptr();
        // PB6 -> AF mode + AF7 (USART1 TX)
        gpiob.moder.modify(|_, w| w.moder6().bits(0b10));
        gpiob.afrl.modify(|_, w| w.afrl6().bits(7));
        // benchuart: force push-pull — telemetry init sets PB6 open-drain
        // (half-duplex KISS pad), whose rise time caps the line ~115200.
        // TX-only line into the adapter's RX, so push-pull is safe.
        #[cfg(feature = "benchuart")]
        gpiob.otyper.modify(|_, w| w.ot6().clear_bit());

        let usart = &*USART1::ptr();
        // Disable while we configure
        usart.cr1.write(|w| w.bits(0));
        // Default oversampling 16, no parity, 8 bits, 1 stop, async mode
        usart.cr2.write(|w| w.bits(0));
        usart.cr3.write(|w| w.bits(0));
        // BRR for OVER8=0: BRR = fck / baud
        let brr = CPU_HZ / BAUD;
        usart.brr.write(|w| w.bits(brr));
        // Enable: TE + UE
        usart.cr1.write(|w| w.te().set_bit().ue().set_bit());

        // Wait for TEACK
        while usart.isr.read().teack().bit_is_clear() {}
    }
}

fn putc(b: u8) {
    let usart = unsafe { &*USART1::ptr() };
    while usart.isr.read().txe().bit_is_clear() {}
    usart.tdr.write(|w| unsafe { w.bits(b as u32) });
}

/// Wait for the transmit shift register to drain. Call before any operation
/// that changes SYSCLK or USART1 config, otherwise the in-flight byte is
/// transmitted at the new (wrong) clock and shows up as garbage.
pub fn flush() {
    let usart = unsafe { &*USART1::ptr() };
    while usart.isr.read().tc().bit_is_clear() {}
}

pub fn write_str(s: &str) {
    for b in s.bytes() {
        putc(b);
    }
}

/// Raw byte out — for binary streams (ZC trace records) that share the
/// bench wire with the text log.
pub fn write_byte(b: u8) {
    putc(b);
}

/// `core::fmt::Write` adapter so we can use `writeln!` against it.
pub struct DebugUart;

impl Write for DebugUart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}
