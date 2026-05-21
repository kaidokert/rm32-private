//! TIM7 driver — fixed-rate motor-drive heartbeat.
//!
//! Sets up TIM7 to fire its update event at a user-chosen rate
//! (`drive_hz`), with UIE enabled so the `TIM7` IRQ handler runs at
//! that rate. The bench uses this to drive TIM1's CCR / six-step
//! commutation from a hardware-paced ISR instead of the main loop —
//! so the motor's commanded electrical frequency is honoured exactly
//! regardless of what main is doing.
//!
//! TIM7 is a 16-bit basic timer on the L431; its clock comes from
//! APB1. The clock has the standard "if APB1 prescaler is 1, timer
//! clock = PCLK1, else = 2 × PCLK1" gotcha, but on this bench we
//! initialise SYSCLK→HCLK→PCLK1 1:1 so `SYSCLK == timer clock` and
//! the ARR math below is straightforward.

use crate::SYSCLK;
use crate::hal::rcc::{APB1R1, Enable, Reset};
use crate::hal::stm32::TIM7;

/// Configure TIM7 for periodic update events at `drive_hz`, with
/// the update interrupt enabled. Counter starts immediately.
///
/// `drive_hz` should divide `SYSCLK` reasonably (i.e. give an ARR
/// that fits in `u16`). At SYSCLK = 80 MHz, anything ≥ 1.22 kHz is
/// fine (ARR ≤ 65535). For a typical 6 kHz update tick this is
/// trivially satisfied.
pub fn init(_tim7: TIM7, apb1: &mut APB1R1, drive_hz: u32) {
    TIM7::enable(apb1);
    TIM7::reset(apb1);

    let arr = (SYSCLK.raw() / drive_hz).saturating_sub(1);
    debug_assert!(arr <= u16::MAX as u32, "drive_hz too low for 16-bit ARR");

    unsafe {
        let tim7 = &*TIM7::ptr();
        tim7.psc.write(|w| w.psc().bits(0));
        tim7.arr.write(|w| w.arr().bits(arr as u16));
        tim7.dier.write(|w| w.uie().set_bit());
        tim7.cr1.write(|w| w.cen().set_bit());
    }
}

/// Ack the TIM7 update flag. Must be called at the top of the
/// `TIM7` ISR or the IRQ re-fires immediately on return.
#[inline]
pub fn clear_update_flag() {
    let tim7 = unsafe { &*TIM7::ptr() };
    tim7.sr.modify(|_, w| w.uif().clear_bit());
}
