//! Bench UART RX — USART2 receiver on PA2 (the J3 `S` pin) via CR2.SWAP.
//!
//! Ported from `minz/src/usart2_rx.rs`. PA2's AF7 function is USART2_TX;
//! SWAP routes the receiver onto it. TE stays 0: receive-only, we never
//! drive the pin, and the GPIO is open-drain so a mistake can't fight the
//! host adapter. Kernel clock is the CCIPR reset default (PCLK1 = 80 MHz).
//!
//! Under `benchuart` the DShot/PWM input-capture path is NOT armed (see
//! `bin/main.rs`) — PA2 belongs to this receiver. The USART2 vector is
//! **ring-push-only**: it never touches `ISR_LOCAL` (see
//! notes/ISR_STATE_INVARIANT.md), so its NVIC priority is unconstrained
//! by the IsrCell aliasing rule. Level 2 matches the clone.

#![cfg(all(feature = "benchuart", feature = "stm32l431"))]

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};

use crate::pac::{GPIOA, RCC, USART2};
use rm32::bench_input::RxRing;

const BAUD: u32 = 2_000_000;
const PCLK1_HZ: u32 = 80_000_000;

const RING_N: usize = 256;
static RING: [AtomicU16; RING_N] = [const { AtomicU16::new(0) }; RING_N];
static HEAD: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);

/// The shared RX ring. Producer = USART2 ISR, consumer = main loop.
#[inline]
pub fn ring() -> RxRing<'static, RING_N> {
    RxRing {
        ring: &RING,
        head: &HEAD,
        tail: &TAIL,
    }
}

/// DMA circular RX buffer. DMA1_CH6 (CSELR C6S=0b0010 = USART2_RX)
/// writes bytes here with zero ISR-latency dependence; main drains by
/// NDTR position. This replaced RXNE-interrupt service: at 2 Mbaud a
/// byte is 5 µs, and prio-0/1 ISR bursts delayed the USART2 vector past
/// that often enough to corrupt ~20% of host sends (dropped digits ->
/// phantom throttle values; see the two-frame-confirmation notes in
/// bin/main.rs). DMA makes overrun structurally impossible.
const DMA_N: usize = 256;
static mut DMA_BUF: [u8; DMA_N] = [0; DMA_N];
static DMA_TAIL: AtomicUsize = AtomicUsize::new(0);

/// One-time init: PA2 → AF7 open-drain + pull-up, USART2 @ 2M 8N1
/// RX-only, received bytes moved by DMA1_CH6 (circular). Call AFTER the
/// clock tree is at 80 MHz (BRR is computed for PCLK1=80M) and after
/// input-capture GPIO setup so this owns PA2's final mux. Per RM0394,
/// BRR and CR2.SWAP are only writable while UE=0 (true out of reset —
/// call once). No NVIC involvement — main polls via drain_dma().
pub fn init() {
    unsafe {
        let rcc = &*RCC::ptr();
        rcc.ahb2enr.modify(|_, w| w.gpioaen().set_bit());
        rcc.apb1enr1.modify(|_, w| w.usart2en().set_bit());
        rcc.ahb1enr.modify(|_, w| w.dma1en().set_bit());

        let gpioa = &*GPIOA::ptr();
        // PA2: alternate mode, AF7 (USART2_TX pad — SWAP makes it RX),
        // open-drain (we never drive it anyway; belt and suspenders),
        // pull-up so the line idles high when the adapter is unplugged.
        gpioa.moder.modify(|_, w| w.moder2().bits(0b10));
        gpioa.afrl.modify(|_, w| w.afrl2().bits(7));
        gpioa.otyper.modify(|_, w| w.ot2().set_bit());
        gpioa.pupdr.modify(|_, w| w.pupdr2().bits(0b01));

        let usart = &*USART2::ptr();
        usart.cr2.write(|w| w.swap().set_bit());
        usart.brr.write(|w| w.bits((PCLK1_HZ + BAUD / 2) / BAUD));
        usart.cr3.write(|w| w.dmar().set_bit());
        usart.cr1.write(|w| w.re().set_bit().ue().set_bit());

        // DMA1_CH6 <- USART2_RX: peripheral->memory, byte, MINC, circular.
        let dma = &*crate::pac::DMA1::ptr();
        dma.cselr
            .modify(|r, w| w.bits((r.bits() & !(0xF << 20)) | (0b0010 << 20)));
        dma.ccr6.modify(|r, w| w.bits(r.bits() & !1)); // EN=0 before config
        dma.cpar6.write(|w| w.bits(usart.rdr.as_ptr() as u32));
        dma.cmar6
            .write(|w| w.bits(core::ptr::addr_of_mut!(DMA_BUF) as u32));
        dma.cndtr6.write(|w| w.bits(DMA_N as u32));
        // MINC | CIRC | EN (MSIZE=PSIZE=8-bit, DIR=periph->mem)
        dma.ccr6.write(|w| w.bits((1 << 7) | (1 << 5) | 1));
    }
}

/// Move newly DMA'd bytes into the parser ring. Main-loop context, every
/// pass. Also counts sticky USART error flags (framing/noise — overrun
/// can no longer occur) so corruption stays observable.
pub fn drain_dma() {
    let rx = ring();
    unsafe {
        let dma = &*crate::pac::DMA1::ptr();
        let usart = &*USART2::ptr();
        let head = DMA_N - dma.cndtr6.read().bits() as usize;
        let mut tail = DMA_TAIL.load(core::sync::atomic::Ordering::Relaxed);
        while tail != head {
            let b = core::ptr::read_volatile(core::ptr::addr_of!(DMA_BUF[tail]));
            rx.push(b as u16);
            tail = (tail + 1) % DMA_N;
        }
        DMA_TAIL.store(tail, core::sync::atomic::Ordering::Relaxed);
        let isr = usart.isr.read();
        if isr.ore().bit_is_set() || isr.fe().bit_is_set() || isr.nf().bit_is_set() {
            ORE_N.fetch_add(1, Ordering::Relaxed);
            usart
                .icr
                .write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
        }
    }
}

/// RX corruption counter: overrun (byte LOST) + framing/noise (byte
/// garbled). A lost digit turns a throttle line into a shorter number —
/// "50\n" minus the '5' is "0\n" = a commanded STOP at speed. This
/// counter is the decision-side instrument for that failure class.
static ORE_N: AtomicU32 = AtomicU32::new(0);

pub fn ore_count() -> u32 {
    ORE_N.load(Ordering::Relaxed)
}

/// Legacy RXNE ISR body — kept as a no-op safety net in case the vector
/// fires (it is no longer enabled; RX moved to DMA1_CH6, see init()).
#[inline]
pub fn service_rx() {
    let usart = unsafe { &*USART2::ptr() };
    let isr = usart.isr.read();
    if isr.ore().bit_is_set() || isr.fe().bit_is_set() || isr.nf().bit_is_set() {
        ORE_N.fetch_add(1, Ordering::Relaxed);
        usart
            .icr
            .write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
    }
}
