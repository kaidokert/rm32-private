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

/// One-time init: PA2 → AF7 open-drain + pull-up, USART2 @ 2M 8N1
/// RX-only with RXNE interrupt. Call AFTER the clock tree is at 80 MHz
/// (BRR is computed for PCLK1=80M) and after input-capture GPIO setup so
/// this owns PA2's final mux. Per RM0394, BRR and CR2.SWAP are only
/// writable while UE=0 (true out of reset — call once). The caller owns
/// the NVIC unmask + priority.
pub fn init() {
    unsafe {
        let rcc = &*RCC::ptr();
        rcc.ahb2enr.modify(|_, w| w.gpioaen().set_bit());
        rcc.apb1enr1.modify(|_, w| w.usart2en().set_bit());

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
        usart
            .cr1
            .write(|w| w.re().set_bit().rxneie().set_bit().ue().set_bit());
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

/// USART2 ISR body: drain RXNE into the ring, then clear overrun /
/// framing / noise so the IRQ can't storm (they share the RXNEIE
/// enable). Ring-push-only — no motor state access.
#[inline]
pub fn service_rx() {
    let usart = unsafe { &*USART2::ptr() };
    let rx = ring();
    while usart.isr.read().rxne().bit_is_set() {
        rx.push(usart.rdr.read().bits() as u16);
    }
    let isr = usart.isr.read();
    if isr.ore().bit_is_set() || isr.fe().bit_is_set() || isr.nf().bit_is_set() {
        ORE_N.fetch_add(1, Ordering::Relaxed);
        usart
            .icr
            .write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
    }
}
