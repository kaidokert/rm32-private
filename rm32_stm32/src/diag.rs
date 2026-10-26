//! The firmware's [`DiagSink`]: bench counters and cycle brackets in debug
//! builds (`debuguart`), `()` in production, where every hook compiles away.
//!
//! Counters are read by the main-loop `[loop]` and last-words lines.

pub use rm32::diag::{DiagSink, IsrSite};

#[cfg(feature = "debuguart")]
pub type Diag = BenchDiag;
#[cfg(not(feature = "debuguart"))]
pub type Diag = ();

#[cfg(feature = "debuguart")]
pub const DIAG: Diag = BenchDiag;
#[cfg(not(feature = "debuguart"))]
pub const DIAG: Diag = ();

#[cfg(feature = "debuguart")]
pub use bench::*;

// Production: the counters read 0, as the `()` sink keeps none.
#[cfg(not(feature = "debuguart"))]
pub fn crc_pass() -> u32 {
    0
}
#[cfg(not(feature = "debuguart"))]
pub fn crc_fail() -> u32 {
    0
}
#[cfg(not(feature = "debuguart"))]
pub fn bidir_events() -> u16 {
    0
}
#[cfg(not(feature = "debuguart"))]
pub fn high_pin_count() -> u8 {
    0
}
#[cfg(not(feature = "debuguart"))]
pub fn last_cycles(_site: IsrSite) -> u32 {
    0
}

#[cfg(feature = "debuguart")]
mod bench {
    use super::{DiagSink, IsrSite};
    use portable_atomic::{AtomicU8, AtomicU16, AtomicU32, Ordering};
    use rm32::commutation::Commutation;
    use rm32::transfer::{TransferAction, TransferActions};

    const ACQ: Ordering = Ordering::Acquire;
    const REL: Ordering = Ordering::Release;

    struct Counters {
        crc_pass: AtomicU32, // valid DShot frames
        crc_fail: AtomicU32, // BadCrc / InvalidTiming
        // Bidir detections plus DShot command frames (EDT handshake debug).
        bidir_evt: AtomicU16,
        // transfer.high_pin_count after each frame; last command id after one.
        high_pin_n: AtomicU8,
        // Last bracket per site, in cycles: single writer each, plain store
        // keeps the measurement overhead to one STR.
        last_cyc: [AtomicU32; 6],
    }

    // One block, so every hook addresses it from a single base.
    static C: Counters = Counters {
        crc_pass: AtomicU32::new(0),
        crc_fail: AtomicU32::new(0),
        bidir_evt: AtomicU16::new(0),
        high_pin_n: AtomicU8::new(0),
        last_cyc: [const { AtomicU32::new(0) }; 6],
    };

    pub struct BenchDiag;

    impl DiagSink for BenchDiag {
        #[inline(always)]
        fn stamp(&self) -> u32 {
            #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
            {
                unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() }
            }
            // M0/M0+: no cycle counter.
            #[cfg(not(any(feature = "stm32l431", feature = "stm32g431")))]
            {
                0
            }
        }

        #[inline(always)]
        fn bracket(&self, site: IsrSite, start: u32) -> u32 {
            #[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
            {
                let end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
                let cycles = end.wrapping_sub(start);
                C.last_cyc[site as usize].store(cycles, REL);
                cycles
            }
            #[cfg(not(any(feature = "stm32l431", feature = "stm32g431")))]
            {
                let _ = (site, start);
                0
            }
        }

        #[inline(always)]
        fn input_frame(
            &self,
            actions: &TransferActions,
            crc_pass: bool,
            dshot: impl FnOnce() -> bool,
        ) {
            C.high_pin_n.store(actions.high_pin_count, REL);
            let is_dshot_none = matches!(actions.action, TransferAction::None) && dshot();
            if crc_pass {
                C.crc_pass.fetch_add(1, REL);
            } else if is_dshot_none {
                C.crc_fail.fetch_add(1, REL);
            }
        }

        #[inline(always)]
        fn dshot_command(&self, cmd: u8) {
            C.bidir_evt.fetch_add(1, REL);
            C.high_pin_n.store(cmd, REL);
        }

        #[inline(always)]
        fn bidir_detected(&self) {
            C.bidir_evt.fetch_add(1, REL);
        }

        #[inline(always)]
        fn on_com_step(&self, commutation: &mut Commutation) {
            #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
            if DESYNC_INJECT.load(core::sync::atomic::Ordering::Relaxed) {
                DESYNC_INJECT.store(false, core::sync::atomic::Ordering::Relaxed);
                commutation.inject_skip();
                INJECT_N.store(
                    INJECT_N
                        .load(core::sync::atomic::Ordering::Relaxed)
                        .wrapping_add(1),
                    core::sync::atomic::Ordering::Relaxed,
                );
            }
            #[cfg(not(all(feature = "benchuart", feature = "stm32g071")))]
            let _ = commutation;
        }
    }

    /// Bench desync injection (binz `K`): consumed by the next commutation.
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    pub static DESYNC_INJECT: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);
    /// Injections actually consumed by the commutation ISR.
    #[cfg(all(feature = "benchuart", feature = "stm32g071"))]
    pub static INJECT_N: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

    pub fn crc_pass() -> u32 {
        C.crc_pass.load(ACQ)
    }
    pub fn crc_fail() -> u32 {
        C.crc_fail.load(ACQ)
    }
    pub fn bidir_events() -> u16 {
        C.bidir_evt.load(ACQ)
    }
    pub fn high_pin_count() -> u8 {
        C.high_pin_n.load(ACQ)
    }
    /// Last bracket recorded at `site`, in cycles.
    pub fn last_cycles(site: IsrSite) -> u32 {
        C.last_cyc[site as usize].load(ACQ)
    }
}
