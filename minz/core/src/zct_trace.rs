//! ZC_TRACE ring cluster — 15-byte records (AM32 fork main.c:1536-1567,
//! 5B A9 sync) + batch-decimation state. Producer: COM ISR
//! (PeriodElapsedCallback) + polling zcfoundroutine. Consumer: main
//! loop → USART1 DMA writer. Guarded with the [`Cs`] seam because two
//! ISR contexts (prio 0 COM, prio 3 TIM6) can both push.
//!
//! BATCH DECIMATION (operator directive 2026-07-20): above the wire's
//! bandwidth the trace switches to 50-commutations-on / 50-off batch
//! mode instead of losing records to saturation aliasing. Batches
//! (not 1-in-N) preserve CONSECUTIVE records so rolling-mean
//! excursion metrics stay valid inside each batch (~44 usable
//! samples per 50). Budget: 2 Mbaud ≈ 13.3k records/s; full rate
//! fits down to ci ≈ 150 ticks (75 µs); batching engages below
//! ci = 200 ticks (100 µs → 10k/s full → 5k/s batched, comfortable).
//! Mode is re-evaluated only at batch boundaries (no mid-batch
//! flapping); flag bit6 marks batched-mode records so the host can
//! segment (decoders mask bits0-2|7 — bit6 is backward-compatible).
//!
//! The ring storage + statics + the instance stay with the program
//! (am32_clone.rs); this module owns only the behavior. Hardware
//! seams: the critical section arrives as a [`Cs`] impl and the
//! drain sink as a byte closure, so the whole module is host-testable.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::am32::{self, ZCT_REC, ZctRing};
use crate::am32_hal::Cs;
use crate::am32_loop::{Drive, Duty, Sched};

/// ZC_TRACE ring cluster — 15-byte records + batch-decimation state.
pub struct ZctTrace<'a, const N: usize> {
    /// Ring mechanics live host-tested in `crate::am32::ZctRing`;
    /// this composes it with the batch-decimation state.
    pub ring: ZctRing<'a, N>,
    pub comm_n: &'a AtomicU32,
    pub batching: &'a AtomicBool,
}

impl<const N: usize> ZctTrace<'_, N> {
    // zct_write (main.c:1542-1567): one canonical row per commutation.
    #[inline(always)]
    pub fn write(&self, sched: &Sched, drive: &Drive, duty: &Duty, cs: &impl Cs) {
        // Batch-decimation gate (am32::zct_batch_gate). Single CI snapshot
        // used for both the gate and the packed record (the value is stable
        // within the calling ISR — this was two separate loads before).
        let n = self.comm_n.fetch_add(1, Ordering::Relaxed);
        let ci_ticks = sched.commutation_interval.load(Ordering::Relaxed);
        let (record, batching) =
            am32::zct_batch_gate(n, ci_ticks, self.batching.load(Ordering::Relaxed));
        self.batching.store(batching, Ordering::Relaxed);
        if !record {
            return; // the skipped half-duty of the batch cycle
        }
        let rec: [u8; ZCT_REC] = am32::zct_pack(
            drive.current_step.load(Ordering::Relaxed) as u8,
            drive.old_routine.load(Ordering::Relaxed),
            batching,
            sched.this_zc.load(Ordering::Relaxed),
            ci_ticks as u16,
            sched.wait_time.load(Ordering::Relaxed),
            duty.duty_cycle.load(Ordering::Relaxed),
            drive.tenkhz_counter.load(Ordering::Relaxed),
            sched.average_interval.load(Ordering::Relaxed) as u16,
        );
        // Dual-producer guard (prio-0 COM + prio-3 TIM6): the core
        // ring is not self-synchronizing; the critical section arrives
        // through the platform's Cs impl.
        cs.free(|| self.ring.push_rec(&rec));
    }

    /// Drain up to 3 ZC_TRACE records into a byte sink (main.c:2347-2357;
    /// the firmware sink pushes into the USART1 DMA ring).
    #[inline]
    pub fn drain(&self, sink: impl FnMut(u8)) {
        self.ring.drain(3, sink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU16, AtomicUsize};

    // (write() is exercised end-to-end — batch gate, pack, cs-guarded
    // push — by the am32_control tests, which own the cluster
    // fixtures. Here: the drain bound + sink plumbing.)

    #[test]
    fn drain_caps_at_three_records_per_call() {
        static RING: [[AtomicU16; ZCT_REC]; 8] =
            [const { [const { AtomicU16::new(0) }; ZCT_REC] }; 8];
        static HEAD: AtomicUsize = AtomicUsize::new(0);
        static TAIL: AtomicUsize = AtomicUsize::new(0);
        static DROP: AtomicU32 = AtomicU32::new(0);
        static COMM_N: AtomicU32 = AtomicU32::new(0);
        static BATCHING: AtomicBool = AtomicBool::new(false);
        let zct = ZctTrace {
            ring: ZctRing { ring: &RING, head: &HEAD, tail: &TAIL, drop: &DROP },
            comm_n: &COMM_N,
            batching: &BATCHING,
        };
        let mut rec = [0u8; ZCT_REC];
        rec[0] = 0x5B;
        rec[1] = 0xA9;
        for v in 0..5u8 {
            rec[2] = v;
            zct.ring.push_rec(&rec);
        }
        // First drain: exactly 3 records (main.c:2347 drains 3/pass).
        let mut out = std::vec::Vec::new();
        zct.drain(|b| out.push(b));
        assert_eq!(out.len(), 3 * ZCT_REC);
        assert_eq!((out[0], out[1], out[2]), (0x5B, 0xA9, 0));
        assert_eq!(out[ZCT_REC + 2], 1);
        // Second drain: the remaining 2.
        out.clear();
        zct.drain(|b| out.push(b));
        assert_eq!(out.len(), 2 * ZCT_REC);
        assert_eq!(out[2], 3);
    }
}
