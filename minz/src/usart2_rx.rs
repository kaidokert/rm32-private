//! USART2 receiver on PA2 (the J3 `S` pin) via CR2.SWAP — raw
//! register init because the HAL's `Serial::usart2` insists on PA3
//! for RX (no swap support), and PA3 is the current-sense ADC input.
//!
//! PA2's AF7 function is USART2_TX; SWAP routes the receiver onto
//! it. TE stays 0: receive-only, we never drive the pin (the GPIO
//! should also be open-drain so a mistake can't fight the host
//! adapter). Kernel clock is the CCIPR reset default (PCLK1).
//!
//! RX is **DMA-circular, no ISR** — ported back from
//! `rm32_stm32/src/bench_uart.rs` (parity-dossier campaign: ore=0,
//! structurally lossless). The old RXNE-interrupt-per-byte service
//! dropped bytes whenever prio-0/1 ISR bursts delayed the USART2
//! vector past one byte time (5 µs at 2 Mbaud) — measured ~20 % of
//! multi-byte host sends corrupted on the rm32 side before their DMA
//! fix, single keys eaten against this clone. DMA1_CH6 (CSELR C6S =
//! 0b0010 = USART2_RX) moves each received byte into a circular ring
//! with zero ISR-latency dependence; main drains by CNDTR position
//! via [`pop`]. USART overrun is structurally impossible (DMA drains
//! RDR at bus speed) and the USART2 NVIC vector is gone entirely.
//!
//! Overrun now means **tail lapped**: if main stalls longer than the
//! ring fill time (256 bytes / 2 Mbaud ≈ 1.28 ms) the DMA overwrites
//! unread bytes silently. [`pop`] keeps a heuristic counter
//! ([`lap_risk_count`]): it increments when the ring is observed
//! full (pending == N−1), i.e. one more byte would lap. Limits: a
//! lap that happens entirely *between* polls — including an exact
//! multiple-of-N lap that leaves head == tail — is invisible, and a
//! legitimate burst that just fills the ring also trips it. Good
//! enough for a bench link where main polls every loop pass.

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::hal::stm32;

/// Circular DMA target. Sole writer is DMA1_CH6; main reads behind
/// the CNDTR-derived head. 256 so the index math is a natural u8 wrap.
const RING_N: usize = 256;
static mut RING: [u8; RING_N] = [0; RING_N];
/// Soft consumer tail (main-context only; atomic for static-safety).
static TAIL: AtomicUsize = AtomicUsize::new(0);
/// Ring-observed-full events (see module docs — heuristic only).
static LAP_RISK: AtomicU32 = AtomicU32::new(0);

/// One-time init at `baud` 8N1: RX-only USART2 with received bytes
/// moved by DMA1_CH6 (circular, byte, MINC) — no RXNEIE, no NVIC.
/// Per RM0394, BRR and CR2.SWAP are only writable while UE=0 (true
/// out of reset — call this once, before any other USART2 touch).
/// The caller must have configured PA2 as AF7 (open-drain, pull-up).
pub fn init_pa2_rx(usart2: stm32::USART2, pclk1_hz: u32, baud: u32) {
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apb1enr1.modify(|_, w| w.usart2en().set_bit());
        rcc.ahb1enr.modify(|_, w| w.dma1en().set_bit());
    }
    usart2.cr2.write(|w| w.swap().set_bit());
    usart2
        .brr
        .write(|w| unsafe { w.bits((pclk1_hz + baud / 2) / baud) });
    usart2.cr3.write(|w| w.dmar().set_bit());
    usart2.cr1.write(|w| w.re().set_bit().ue().set_bit());

    // DMA1_CH6 <- USART2_RX: peripheral->memory, byte, MINC, circular.
    unsafe {
        let dma = &*stm32::DMA1::ptr();
        // CSELR.C6S (bits 23:20) = 0b0010 selects USART2_RX.
        dma.cselr
            .modify(|r, w| w.bits((r.bits() & !(0xF << 20)) | (0b0010 << 20)));
        dma.ccr6.modify(|r, w| w.bits(r.bits() & !1)); // EN=0 before config
        dma.cpar6.write(|w| w.bits(usart2.rdr.as_ptr() as u32));
        dma.cmar6
            .write(|w| w.bits(core::ptr::addr_of_mut!(RING) as u32));
        dma.cndtr6.write(|w| w.bits(RING_N as u32));
        // MINC | CIRC | EN (MSIZE=PSIZE=8-bit, DIR=periph->mem)
        dma.ccr6.write(|w| w.bits((1 << 7) | (1 << 5) | 1));
    }
}

/// Dequeue one received byte, or `None` if the ring is drained.
/// Main-context only (single consumer): head = RING_N − CNDTR6 is
/// where DMA will write next; the soft tail chases it. On an empty
/// ring, sticky USART error flags (framing/noise — overrun can no
/// longer occur) are cleared so a line glitch can't wedge the
/// receiver (AM32 main.c:1417-1419 does the same clears).
pub fn pop() -> Option<u8> {
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let head = RING_N - dma.cndtr6.read().bits() as usize;
    let tail = TAIL.load(Ordering::Relaxed);
    if tail == head {
        let usart = unsafe { &*stm32::USART2::ptr() };
        let isr = usart.isr.read();
        if isr.ore().bit_is_set() || isr.fe().bit_is_set() || isr.nf().bit_is_set() {
            usart
                .icr
                .write(|w| w.orecf().set_bit().fecf().set_bit().ncf().set_bit());
        }
        return None;
    }
    if (head + RING_N - tail) % RING_N == RING_N - 1 {
        LAP_RISK.fetch_add(1, Ordering::Relaxed);
    }
    let b = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(RING[tail])) };
    TAIL.store((tail + 1) % RING_N, Ordering::Relaxed);
    Some(b)
}

/// Heuristic lap-risk counter (see module docs for its blind spots).
pub fn lap_risk_count() -> u32 {
    LAP_RISK.load(Ordering::Relaxed)
}
