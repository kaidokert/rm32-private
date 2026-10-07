//! Bench serial on USART3 (PC10 TX / PC11 RX, AF0), 115200 8N1 — the binz
//! NUCLEO-G071RB + DRV8304H bench wiring (FTDI adapter, not the ST-LINK
//! VCOM: PA2/PA3 are COMP2 inputs on this board).
//!
//! One full-duplex port carries both directions: the `dprintln!` log (TX,
//! polled) and the bench command stream (RX, DMA1 CH4 circular via
//! DMAMUX request 54 = USART3_RX). DMA makes RX overrun structurally
//! impossible while the main loop is busy in a polled print — with the
//! G071's 1-byte RDR (FIFO off) a 100-char log line would otherwise drop
//! command bytes.
//!
//! Stays at 115200: the G071 USART kernel clock is PCLK = 64 MHz and the
//! binz bench has never run this adapter faster.

#![cfg(feature = "debuguart")]

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::pac::{DMA1, DMAMUX, GPIOC, RCC, USART3};

const PCLK_HZ: u32 = 64_000_000;
const BAUD: u32 = 115_200;
/// DMAMUX request line for USART3_RX (RM0444 Table 59; LL_DMAMUX_REQ_USART3_RX).
const DMAREQ_USART3_RX: u32 = 54;
/// DMA1 channel 4 (index 3). CH1 = DShot capture, CH2 = ADC, CH3 = telemetry TX.
const DMA_CH: usize = 3;

const DMA_N: usize = 256;
static mut DMA_BUF: [u8; DMA_N] = [0; DMA_N];
static DMA_TAIL: AtomicUsize = AtomicUsize::new(0);
static ERR_N: AtomicU32 = AtomicU32::new(0);

/// One-time init. Idempotent with respect to clocks; call once at boot
/// after the PLL is running (BRR assumes PCLK = 64 MHz).
pub fn init() {
    // SAFETY: boot-time single-owner configuration of GPIOC, USART3, DMA1
    // CH4 and DMAMUX C3 — no other module touches these on G071.
    unsafe {
        let rcc = &*RCC::ptr();
        rcc.iopenr().modify(|r, w| w.bits(r.bits() | (1 << 2))); // GPIOCEN
        rcc.apbenr1().modify(|r, w| w.bits(r.bits() | (1 << 18))); // USART3EN
        rcc.ahbenr().modify(|r, w| w.bits(r.bits() | 1)); // DMA1EN

        let gpioc = &*GPIOC::ptr();
        // PC10, PC11 -> alternate function (MODER = 0b10), AF0.
        gpioc.moder().modify(|r, w| {
            w.bits((r.bits() & !((0b11 << 20) | (0b11 << 22))) | (0b10 << 20) | (0b10 << 22))
        });
        gpioc
            .afrh()
            .modify(|r, w| w.bits(r.bits() & !((0xF << 8) | (0xF << 12))));
        // PC11 pull-up so an unplugged adapter idles the RX line high.
        gpioc
            .pupdr()
            .modify(|r, w| w.bits((r.bits() & !(0b11 << 22)) | (0b01 << 22)));

        let usart = &*USART3::ptr();
        usart.cr1().write(|w| w.bits(0));
        usart.cr2().write(|w| w.bits(0));
        usart.cr3().write(|w| w.bits(1 << 6)); // DMAR
        usart.brr().write(|w| w.bits((PCLK_HZ + BAUD / 2) / BAUD));
        // TE | RE | UE (FIFO stays disabled, OVER8 = 0).
        usart.cr1().write(|w| w.bits((1 << 3) | (1 << 2) | 1));

        let dma = &*DMA1::ptr();
        let ch = dma.ch(DMA_CH);
        ch.cr().write(|w| w.bits(0));
        let mux = &*DMAMUX::ptr();
        mux.ccr(DMA_CH).write(|w| w.bits(DMAREQ_USART3_RX));
        ch.par().write(|w| w.bits(usart.rdr().as_ptr() as u32));
        ch.mar()
            .write(|w| w.bits(core::ptr::addr_of_mut!(DMA_BUF) as u32));
        ch.ndtr().write(|w| w.bits(DMA_N as u32));
        // MINC | CIRC | EN; 8-bit, peripheral -> memory, no interrupts.
        ch.cr().write(|w| w.bits((1 << 7) | (1 << 5) | 1));

        // Bounded TEACK wait — a wedged USART must not hang boot.
        for _ in 0..100_000u32 {
            if usart.isr().read().bits() & (1 << 21) != 0 {
                break;
            }
        }
    }
}

#[inline]
pub fn tx_ready() -> bool {
    let usart = unsafe { &*USART3::ptr() };
    usart.isr().read().bits() & (1 << 7) != 0 // TXE
}

#[inline]
pub fn tx_write(b: u8) {
    let usart = unsafe { &*USART3::ptr() };
    // SAFETY: TDR accepts any 8-bit value.
    usart.tdr().write(|w| unsafe { w.bits(b as u32) });
}

#[inline]
pub fn tx_done() -> bool {
    let usart = unsafe { &*USART3::ptr() };
    usart.isr().read().bits() & (1 << 6) != 0 // TC
}

/// Move newly DMA'd bytes into `push`. Main-loop context only. Also
/// counts and clears sticky framing/noise/overrun flags.
pub fn drain(mut push: impl FnMut(u8)) {
    let dma = unsafe { &*DMA1::ptr() };
    let head = DMA_N - dma.ch(DMA_CH).ndtr().read().bits() as usize;
    let mut tail = DMA_TAIL.load(Ordering::Relaxed);
    while tail != head {
        // SAFETY: DMA only writes bytes; a racing write to this slot is
        // impossible because head was sampled before the read.
        let b = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(DMA_BUF[tail])) };
        push(b);
        tail = (tail + 1) % DMA_N;
    }
    DMA_TAIL.store(tail, Ordering::Relaxed);
    let usart = unsafe { &*USART3::ptr() };
    let isr = usart.isr().read().bits();
    // ORE (3) | NE (2) | FE (1)
    if isr & 0b1110 != 0 {
        ERR_N.store(
            ERR_N.load(Ordering::Relaxed).wrapping_add(1),
            Ordering::Relaxed,
        );
        // SAFETY: ICR is write-1-to-clear.
        usart.icr().write(|w| unsafe { w.bits(0b1110) });
    }
}

pub fn err_count() -> u32 {
    ERR_N.load(Ordering::Relaxed)
}
