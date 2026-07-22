//! TIM6 as the 20 kHz control loop — AM32's `tenKhzRoutine` timer,
//! byte-for-byte the rm32-verified AM32-L431 configuration:
//! PSC = 79 (1 MHz tick), ARR = 50 → 19.6 kHz update, priority 3
//! (lowest — the loop must be preempted by everything
//! motor-critical).
//!
//! Timer-alignment step 2 of the operator's directive (TIM16 COM
//! timer was step 1): control-loop work migrates here from the
//! 6 kHz TIM7 tick incrementally — throttle slew first, then the
//! watchdog/amnesty polls, then the drive stepper (which needs its
//! poll-count constants re-derived for the rate), until TIM7
//! retires.

use crate::hal::stm32;

pub fn init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.tim6en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM6::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(79));
    tim.arr.write(|w| w.arr().bits(50));
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.write(|w| w.uie().set_bit());
    tim.cr1.modify(|_, w| w.cen().set_bit());
}

/// Ack the update flag — first line of the `TIM6_DACUNDER` ISR.
#[inline]
pub fn clear_flag() {
    let tim = unsafe { &*stm32::TIM6::ptr() };
    tim.sr.write(|w| unsafe { w.bits(0) });
}

/// The `minz_core::am32_hal::LoopTimer` register impl over TIM6 —
/// zero-sized, static dispatch; delegates to [`clear_flag`].
pub struct Tim6Loop;

impl minz_core::am32_hal::LoopTimer for Tim6Loop {
    #[inline(always)]
    fn clear_flag(&self) {
        clear_flag()
    }
}
