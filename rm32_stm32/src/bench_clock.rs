//! Wrapping CPU-cycle timestamp for main-loop bench machinery (deadman,
//! bench guard debounce, report pacing).
//!
//! M4 (L431/G431): DWT.CYCCNT. M0+ (G071) has no cycle counter: derive the
//! timestamp from the TIM6 control-tick count (`dbg_isr_tick`, 20 kHz) as
//! ticks * cycles-per-tick. 50 µs resolution, and strictly monotonic — the
//! timer's CNT is deliberately NOT folded in: a CNT wrap observed before
//! the ISR bumps the tick count would step time backwards, and every
//! `wrapping_sub` consumer would see a ~2^32 elapsed time (the
//! false-trip-on-underflow class).

#[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
#[inline]
pub fn now_cyc() -> u32 {
    // SAFETY: read-only access to the free-running DWT cycle counter.
    unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() }
}

/// TIM6 runs PSC=0, ARR=3199 at 64 MHz on G071 (mcu_g071/init.rs).
#[cfg(feature = "stm32g071")]
pub const CYC_PER_TICK: u32 = 3200;

#[cfg(feature = "stm32g071")]
#[inline]
pub fn now_cyc() -> u32 {
    crate::isr::shared()
        .dbg_isr_tick()
        .wrapping_mul(CYC_PER_TICK)
}
