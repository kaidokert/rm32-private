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

/// Flip PB6 (USART1_TX) to push-pull AFTER HAL init. The HAL's
/// half-duplex pin trait insists on open-drain, whose rise through
/// the ~40 kΩ internal pull-up is 2-3 µs and caps the usable baud at
/// about 115200. This line is TX-only (nothing else ever drives it),
/// so actively driving both levels is safe — and is the load-bearing
/// trick that makes ≥921600 work.
pub fn usart1_tx_push_pull_pb6() {
    unsafe {
        (*stm32::GPIOB::ptr())
            .otyper
            .modify(|r, w| w.bits(r.bits() & !(1 << 6)));
    }
}

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
            let tc = dma.isr.read().tcif4().bit_is_set();
            // Lost-completion resync (the silent-TX-death wedge,
            // 2026-07-18): CNDTR==0 means the transfer finished even
            // if the TC flag was consumed elsewhere - without this,
            // service waits forever for a flag that never re-sets and
            // ALL output dies while main runs healthy.
            if tc || dma.cndtr4.read().bits() == 0 {
                if !tc {
                    TX_RESYNCS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                }
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
        // HARDWARE CONTRACT (the black-hole wedge): L4 DMA IGNORES
        // CMAR/CNDTR writes while EN=1. If any path leaves EN set
        // here, the arm below is silently discarded and every
        // subsequent "completion" advances the tail on stale flags -
        // output becomes a black hole while the ring drains normally.
        // Force EN=0 (and count it) before programming.
        if dma.ccr4.read().en().bit_is_set() {
            TX_RESYNCS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            dma.ccr4.modify(|_, w| w.en().clear_bit());
            while dma.ccr4.read().en().bit_is_set() {}
            dma.ifcr.write(|w| w.cgif4().set_bit());
        }
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

    /// Enqueue a byte slice, spinning `service` while the ring is
    /// full — but BOUNDED: if the DMA fails to drain for ~50 ms the
    /// remainder is dropped and counted in `TX_DROPPED`. The old
    /// unbounded spin never refreshed the IWDG, so a stalled or
    /// non-completing DMA transfer became the telemetry-load IWDG
    /// REBOOT class (reviewer audit 2026-07-18). Telemetry loss is
    /// recoverable; a watchdog reset mid-run is not.
    pub fn write_blocking(&mut self, bytes: &[u8]) {
        let wb0 = cortex_m::peripheral::DWT::cycle_count();
        for (i, &b) in bytes.iter().enumerate() {
            if !self.push(b) {
                let t0 = cortex_m::peripheral::DWT::cycle_count();
                loop {
                    self.service();
                    if self.push(b) {
                        break;
                    }
                    if cortex_m::peripheral::DWT::cycle_count().wrapping_sub(t0) > 4_000_000 {
                        TX_DROPPED.fetch_add(
                            (bytes.len() - i) as u32,
                            core::sync::atomic::Ordering::Relaxed,
                        );
                        return;
                    }
                }
            }
        }
        self.service();
        let dur = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(wb0);
        if dur > WB_MAX_CYC.load(core::sync::atomic::Ordering::Relaxed) {
            WB_MAX_CYC.store(dur, core::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Bytes dropped by the bounded `write_blocking` timeout. Nonzero
/// means the DMA stalled ≥50 ms — the condition that used to reboot.
/// Worst single write_blocking duration in cycles (swap-read for max
/// tracking; the mzt_shed 800 ms main-stall hunt).
/// Hardware-state resyncs performed by `service` (lost completion or
/// EN-left-set). Nonzero = the black-hole wedge class fired and was
/// healed in place.
pub static TX_RESYNCS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub static WB_MAX_CYC: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
pub static TX_DROPPED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

impl core::fmt::Write for UartTxWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            // Drop bytes on overflow — status messages aren't critical.
            let _ = self.push(b);
        }
        Ok(())
    }
}
