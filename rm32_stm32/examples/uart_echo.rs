//! L431 bench UART wiring check: echo every byte received on the S pad
//! (PA2, USART2 RX via CR2.SWAP) out the TX1 pad (PB6, USART1 TX), at the
//! bench's 2 Mbaud 8N1. After ~1 s with no input it prints a heartbeat line,
//! so TX1->adapter RXD can be confirmed on its own before testing S.
//!
//! Standalone (no rm32_stm32 lib), raw register writes. Runs from reset
//! clocks (MSI 4 MHz core); both USART kernels are fed from HSI16 so
//! 2 Mbaud is exact (OVER8=1, USARTDIV=16). No interrupts, no bridge or
//! comparator setup: the motor phases stay at their reset (input) state.
//!
//! Linked at the app slot (0x08001000). Build + start over SWD:
//!   cargo build --release --example uart_echo --target thumbv7em-none-eabihf \
//!     --no-default-features --features stm32l431
//! then load and force-jump with gdb (the bootloader won't jump unless the
//! EEPROM boot byte is 0x01): set $sp/$pc from the ELF's vector table.

#![no_std]
#![no_main]

use core::panic::PanicInfo;

use cortex_m::peripheral::syst::SystClkSource;
use cortex_m_rt::entry;
use stm32l4xx_hal::pac;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        cortex_m::asm::nop();
    }
}

fn put(u: &pac::usart1::RegisterBlock, b: u8) {
    while u.isr.read().bits() & (1 << 7) == 0 {} // TXE
    u.tdr.write(|w| unsafe { w.bits(b as u32) });
}

fn put_str(u: &pac::usart1::RegisterBlock, s: &[u8]) {
    for &b in s {
        put(u, b);
    }
}

fn put_dec(u: &pac::usart1::RegisterBlock, mut n: u32) {
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    put_str(u, &buf[i..]);
}

#[entry]
fn main() -> ! {
    let dp = unsafe { pac::Peripherals::steal() };
    let mut cp = unsafe { cortex_m::Peripherals::steal() };

    // HSI16 on: USART1/USART2 kernel clock (CCIPR USARTxSEL = 0b10).
    dp.RCC
        .cr
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 8)) });
    while dp.RCC.cr.read().bits() & (1 << 10) == 0 {}
    dp.RCC
        .ccipr
        .modify(|r, w| unsafe { w.bits((r.bits() & !0xF) | 0b10 | (0b10 << 2)) });

    // Clocks: GPIOA, GPIOB; USART1 (APB2 bit 14); USART2 (APB1 bit 17).
    dp.RCC
        .ahb2enr
        .modify(|r, w| unsafe { w.bits(r.bits() | 0b11) });
    dp.RCC
        .apb2enr
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 14)) });
    dp.RCC
        .apb1enr1
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 17)) });

    // PB6: AF7 (USART1_TX), push-pull, pull-up, medium speed.
    let gb = &dp.GPIOB;
    gb.afrl
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 24)) | (7 << 24)) });
    gb.otyper
        .modify(|r, w| unsafe { w.bits(r.bits() & !(1 << 6)) });
    gb.ospeedr
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0b11 << 12)) | (0b01 << 12)) });
    gb.pupdr
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0b11 << 12)) | (0b01 << 12)) });
    gb.moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0b11 << 12)) | (0b10 << 12)) });

    // PA2: AF7 (USART2_TX pin, receiver routed onto it by SWAP), open-drain,
    // pull-up; TE stays 0 so the pin is never driven.
    let ga = &dp.GPIOA;
    ga.afrl
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 8)) | (7 << 8)) });
    ga.otyper
        .modify(|r, w| unsafe { w.bits(r.bits() | (1 << 2)) });
    ga.pupdr
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0b11 << 4)) | (0b01 << 4)) });
    ga.moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0b11 << 4)) | (0b10 << 4)) });

    // 2 Mbaud from 16 MHz, OVER8=1: USARTDIV=16 -> BRR = 0x10.
    let u1 = &dp.USART1;
    u1.cr1.write(|w| unsafe { w.bits(0) });
    u1.brr.write(|w| unsafe { w.bits(0x10) });
    u1.cr1.write(|w| unsafe { w.bits((1 << 15) | (1 << 3)) }); // OVER8, TE
    u1.cr1.modify(|r, w| unsafe { w.bits(r.bits() | 1) }); // UE

    let u2 = &dp.USART2;
    u2.cr1.write(|w| unsafe { w.bits(0) });
    u2.brr.write(|w| unsafe { w.bits(0x10) });
    u2.cr2.write(|w| unsafe { w.bits(1 << 15) }); // SWAP
    u2.cr3.write(|w| unsafe { w.bits(1 << 12) }); // OVRDIS
    u2.cr1.write(|w| unsafe { w.bits((1 << 15) | (1 << 2)) }); // OVER8, RE
    u2.cr1.modify(|r, w| unsafe { w.bits(r.bits() | 1) }); // UE

    // SysTick: 1 ms at the 4 MHz MSI reset clock.
    cp.SYST.set_clock_source(SystClkSource::Core);
    cp.SYST.set_reload(4_000 - 1);
    cp.SYST.clear_current();
    cp.SYST.enable_counter();

    let u1r: &pac::usart1::RegisterBlock = u1;
    put_str(
        u1r,
        b"\r\n[uart_echo] L431 up: TX1=PB6 RX=S/PA2 2000000 8N1\r\n",
    );

    let mut rx_total: u32 = 0;
    let mut errs: u32 = 0;
    let mut idle_ms: u32 = 0;
    let mut beat: u32 = 0;
    loop {
        // IWDG refresh: harmless if it was never started.
        dp.IWDG.kr.write(|w| unsafe { w.bits(0xAAAA) });

        let isr = u2.isr.read().bits();
        if isr & 0b1111 != 0 {
            // PE/FE/NE/ORE: count and clear (ICR bits 0..3).
            errs += 1;
            u2.icr.write(|w| unsafe { w.bits(0b1111) });
        }
        if isr & (1 << 5) != 0 {
            // RXNE
            let b = u2.rdr.read().bits() as u8;
            put(u1r, b);
            rx_total += 1;
            idle_ms = 0;
        }
        if cp.SYST.has_wrapped() {
            idle_ms += 1;
            if idle_ms >= 1000 {
                idle_ms = 0;
                beat += 1;
                put_str(u1r, b"[uart_echo] beat=");
                put_dec(u1r, beat);
                put_str(u1r, b" rx=");
                put_dec(u1r, rx_total);
                put_str(u1r, b" err=");
                put_dec(u1r, errs);
                put_str(u1r, b"\r\n");
            }
        }
    }
}
