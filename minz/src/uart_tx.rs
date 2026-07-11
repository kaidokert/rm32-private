//! DMA-backed UART TX ring for USART1. `write!` enqueues into a
//! 4 KiB static ring (drop-on-overflow); the caller runs [`UartTxWriter::service`]
//! once per microloop, which hands the longest contiguous unsent run
//! to **DMA1_CH4** (USART1_TX request, CSELR C4S = 0b0010) and lets
//! hardware drain it at wire rate. No DMA interrupt — transfer-
//! complete is polled from `service`, so the whole TX path stays
//! main-context-only.
//!
//! Why: a byte-per-pass polled writer moves ~1 kB/s — fine for key
//! echoes at 9600, useless for streaming at 2 Mbaud (200 kB/s). With
//! 4 KiB chunks kicked per pass the wire stays >95 % utilised while
//! main spends ~0 CPU on TX.
//!
//! Ownership note: `Tx<USART1>` is held only to keep the HAL from
//! handing the peripheral to anyone else — after `CR3.DMAT` is set,
//! data moves ring → TDR entirely by DMA. Never write TDR from the
//! CPU while a chunk is in flight (interleaved garbage); all output
//! must go through this ring, including "blocking" dumps.

use crate::hal::serial::Tx;
use crate::hal::stm32;
use crate::hal::stm32::USART1;
use core::sync::atomic::Ordering;

pub const TX_RING_LEN: usize = 4096; // power of two

pub struct UartTxWriter {
    _tx: Tx<USART1>,
    ring: &'static mut [u8; TX_RING_LEN],
    /// Next byte to write (main only). Ring is full when advancing
    /// head would collide with tail (one slot wasted, classic ring).
    head: usize,
    /// Oldest unsent byte. Advances only on DMA transfer-complete.
    tail: usize,
    /// Bytes handed to the in-flight DMA chunk (0 = DMA idle).
    inflight: usize,
}

impl UartTxWriter {
    pub fn new(tx: Tx<USART1>, ring: &'static mut [u8; TX_RING_LEN]) -> Self {
        // One-time plumbing: DMA1 clock, route channel 4 to USART1_TX,
        // point CPAR at TDR, and let USART1 raise DMA requests.
        unsafe {
            (*stm32::RCC::ptr())
                .ahb1enr
                .modify(|_, w| w.dma1en().set_bit());
            let dma = &*stm32::DMA1::ptr();
            dma.cselr.modify(|_, w| w.c4s().bits(0b0010));
            dma.cpar4
                .write(|w| w.bits(&(*stm32::USART1::ptr()).tdr as *const _ as u32));
            (*stm32::USART1::ptr())
                .cr3
                .modify(|_, w| w.dmat().set_bit());
        }
        Self {
            _tx: tx,
            ring,
            head: 0,
            tail: 0,
            inflight: 0,
        }
    }

    pub fn pending(&self) -> usize {
        self.head.wrapping_sub(self.tail) & (TX_RING_LEN - 1)
    }

    /// Push one byte; `false` (byte dropped) if the ring is full.
    pub fn push(&mut self, b: u8) -> bool {
        let next = (self.head + 1) & (TX_RING_LEN - 1);
        if next == self.tail {
            return false;
        }
        self.ring[self.head] = b;
        self.head = next;
        true
    }

    /// Reap a completed DMA chunk (if any) and kick the next one.
    /// Called once per microloop; also spun directly by the blocking
    /// paths. Worst-case gap between chunk-complete and next kick is
    /// one microloop (1 ms) — with 4 KiB chunks (20 ms of wire time
    /// at 2 M) that keeps the line >95 % utilised.
    pub fn service(&mut self) {
        let dma = unsafe { &*stm32::DMA1::ptr() };
        if self.inflight != 0 {
            if dma.isr.read().tcif4().bit_is_set() {
                dma.ccr4.modify(|_, w| w.en().clear_bit());
                dma.ifcr.write(|w| w.cgif4().set_bit());
                self.tail = (self.tail + self.inflight) & (TX_RING_LEN - 1);
                self.inflight = 0;
            } else {
                return; // chunk still on the wire
            }
        }
        let pending = self.pending();
        if pending == 0 {
            return;
        }
        // Longest contiguous run from tail (a wrap becomes two chunks).
        let contig = pending.min(TX_RING_LEN - self.tail);
        unsafe {
            dma.cmar4
                .write(|w| w.bits(self.ring.as_ptr().add(self.tail) as u32));
            dma.cndtr4.write(|w| w.bits(contig as u32));
            // Ring bytes must be visible to DMA before EN.
            core::sync::atomic::compiler_fence(Ordering::Release);
            dma.ccr4
                .write(|w| w.minc().set_bit().dir().set_bit().en().set_bit());
        }
        self.inflight = contig;
    }

    /// Enqueue a byte slice without dropping: spin `service` whenever
    /// the ring is full. Ordering vs earlier `write!` output is free
    /// (single ring). Returns once everything is *enqueued* — the tail
    /// of the data may still be draining by DMA afterwards, which is
    /// fine because all output goes through the same ring.
    pub fn write_blocking(&mut self, bytes: &[u8]) {
        for &b in bytes {
            while !self.push(b) {
                self.service();
            }
        }
        self.service();
    }
}

impl core::fmt::Write for UartTxWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // Drop bytes on overflow — status messages aren't critical.
            let _ = self.push(b);
        }
        Ok(())
    }
}
