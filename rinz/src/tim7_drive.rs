//! TIM7 basic-timer heartbeat — sinewave drive for the rinz bench.
//!
//! TIM7 is a 16-bit basic timer on the G431, clocked from APB1.
//! With SYSCLK=170 MHz and no APB1 prescaler, timer clock = 170 MHz.
//! ARR = SYSCLK / drive_hz − 1; at 6 kHz ARR = 28332.

use crate::hal::stm32::{RCC, TIM7};

/// Init TIM7 at `drive_hz`, UIE enabled, counter starts immediately.
/// Takes ownership of the TIM7 peripheral token.
pub fn init(_tim7: TIM7, drive_hz: u32, sysclk_hz: u32) {
    unsafe {
        (*RCC::ptr()).apb1enr1().modify(|_, w| w.tim7en().set_bit());
        let _ = (*RCC::ptr()).apb1enr1().read(); // propagation delay

        let t = &*TIM7::ptr();
        let arr = (sysclk_hz / drive_hz).saturating_sub(1);
        t.psc().write(|w| w.psc().bits(0));
        t.arr().write(|w| w.arr().bits(arr));
        t.dier().write(|w| w.uie().set_bit());
        t.cr1().write(|w| w.cen().set_bit());
    }
}

/// Clear the TIM7 update interrupt flag. Call at the top of the TIM7 ISR.
#[inline]
pub fn clear_update_flag() {
    unsafe {
        (*TIM7::ptr()).sr().modify(|_, w| w.uif().clear_bit());
    }
}
