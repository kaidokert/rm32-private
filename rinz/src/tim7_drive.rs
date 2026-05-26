//! TIM7 basic-timer heartbeat — sinewave drive for the rinz bench.

use crate::hal::{
    rcc::Clocks,
    stm32::TIM7,
    time::ExtU32,
    timer::{Event, Timer},
};

/// Init TIM7 at `drive_hz`, UIE enabled, counter starts immediately.
/// The returned `CountDownTimer` can be dropped — no `Drop` impl, HW keeps running.
pub fn init(tim7: TIM7, drive_hz: u32, clocks: &Clocks) {
    let timer = Timer::new(tim7, clocks);
    let mut cd = timer.start_count_down((1_000_000u32 / drive_hz).micros());
    cd.listen(Event::TimeOut);
}

/// Clear the TIM7 update interrupt flag. Call at the top of the TIM7 ISR.
/// Uses PAC directly — the `CountDownTimer` is not accessible from ISR context.
#[inline]
pub fn clear_update_flag() {
    unsafe {
        (*TIM7::ptr()).sr().write(|w| w.uif().clear_bit());
    }
}
