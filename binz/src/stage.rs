//! Shared stage-safe primitive: the ONE way every motor example turns the
//! power stage off. Register-only, callable from any context (ISR / main /
//! panic path), correct for both drive styles:
//!  - TIM1 AF mode: MOE off -> OSSI/OSSR drive the pins to their idle-low
//!    state; CCRs zeroed for good measure.
//!  - plain-GPIO mode: BSRR resets on all six pins (ignored in AF mode).
//! Plus EN (PC9) low, which on the EVLDRIVE102H's shared EN/nFLT node also
//! lights the red LED = visible "safed" indicator.

use stm32g0xx_hal::stm32;

/// Force the power stage safe. Idempotent, lock-free, ~1 us.
#[inline]
pub fn force_safe() {
    unsafe {
        let tim = &*stm32::TIM1::ptr();
        tim.bdtr().modify(|r, w| w.bits(r.bits() & !(1 << 15))); // MOE off
        tim.ccr1().write(|w| w.bits(0));
        tim.ccr2().write(|w| w.bits(0));
        tim.ccr3().write(|w| w.bits(0));
        let pa = &*stm32::GPIOA::ptr();
        // INH1/2/3 = PA8/9/10, INL1 = PA7 (resets; no-ops in AF mode)
        pa.bsrr().write(|w| {
            w.bits((1 << (7 + 16)) | (1 << (8 + 16)) | (1 << (9 + 16)) | (1 << (10 + 16)))
        });
        let pd = &*stm32::GPIOD::ptr();
        pd.bsrr()
            .write(|w| w.bits((1 << (3 + 16)) | (1 << (4 + 16)))); // INL2/3
        let pc = &*stm32::GPIOC::ptr();
        pc.bsrr().write(|w| w.br9().set_bit()); // EN low
    }
}
