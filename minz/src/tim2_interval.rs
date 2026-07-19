//! TIM2 as the interval timer — AM32's INTERVAL_TIMER on this chip
//! (timer-alignment step 3): free-running 1 µs counter, ARR=0xFFFF
//! (16-bit wrap, AM32/rm32-parity), CNT reset ONLY on an accepted
//! ZC. Two things follow, both AM32-verbatim:
//!
//! - The COMP gate becomes `CNT > average_interval/2` — anchored to
//!   the last ACCEPT, not the window open. After a missed window the
//!   gate opens LATER (elapsed keeps growing), a built-in miss brake
//!   our window-anchored gate lacked (fresh re-anchor each window =
//!   permissive after exactly the misses that seed the cascade).
//! - A CNT read is 2 cycles; the `ticks_1us()` it replaces in the
//!   hot COMP path is a software u64 division.

use crate::hal::stm32;

pub fn init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.tim2en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(79)); // 1 MHz = 1 µs ticks
    tim.arr.write(|w| unsafe { w.bits(0xFFFF) }); // 16-bit wrap, AM32-matched
    tim.egr.write(|w| w.ug().set_bit());
    tim.cr1.modify(|_, w| w.cen().set_bit());
}

/// µs since the last [`reset`] (16-bit wrap at 65.5 ms — any
/// legitimate gate comparison is far below it; a stall reads large,
/// which is exactly the conservative direction).
#[inline]
pub fn cnt_us() -> u32 {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.read().bits() & 0xFFFF
}

/// AM32: the interval timer resets ONLY on an accepted ZC.
#[inline]
pub fn reset() {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.write(|w| unsafe { w.bits(0) });
}
