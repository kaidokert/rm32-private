//! Boot-only pad witness of role preloading; DRV ENABLE stays low throughout.
use super::{latch, regs};
use crate::hw::gpio;

fn pads() -> u8 {
    // SAFETY: read-only pad samples on the six configured TIM1 pins.
    let a = unsafe { &*stm32g0xx_hal::stm32::GPIOA::ptr() }.idr().read().bits();
    // SAFETY: same read-only pad access for PB0/PB1.
    let b = unsafe { &*stm32g0xx_hal::stm32::GPIOB::ptr() }.idr().read().bits();
    (((a >> 8) & 1) | (((a >> 7) & 1) << 1) | (((a >> 9) & 1) << 2)
        | ((b & 1) << 3) | (((a >> 10) & 1) << 4) | (((b >> 1) & 1) << 5)) as u8
}

/// No bridge power request: ENABLE pad must be low before MOE is used to
/// inspect MCU timer-output pads. All compares stay zero. Forced output modes
/// distinguish source/sink roles independently of carrier phase.
pub fn run() -> Option<(u8, u8, u8, u8)> {
    if gpio::enable_is_high() || super::moe_is_set() || super::compares() != (0, 0, 0) {
        return None;
    }
    let mut first = crate::sixstep::plan(crate::commutation::Step::new_clamped(1), 0, 6400, 800)?;
    first.ccmr1 = 0x4848;
    first.ccmr2 = 0x58; // CH3 forced active; CH2 complementary sink.
    let mut next = crate::sixstep::plan(crate::commutation::Step::new_clamped(2), 0, 6400, 800)?;
    next.ccmr1 = 0x4858; // CH1 forced active; CH2 remains sink.
    next.ccmr2 = 0x48;
    gpio::gates_to_timer();
    latch::apply(&first);
    super::moe_on();
    cortex_m::asm::delay(640);
    let before = pads();
    let t = regs();
    t.cr2().modify(|_, w| w.ccpc().set_bit().ccus().clear_bit());
    // SAFETY: forced role images above, no polarity or compare changes.
    t.ccmr1_output().write(|w| unsafe { w.bits(next.ccmr1) });
    // SAFETY: CH3 forced-inactive image declared above.
    t.ccmr2_output().write(|w| unsafe { w.bits(next.ccmr2) });
    // SAFETY: sixstep enable image, no polarity changes.
    t.ccer().write(|w| unsafe { w.bits(next.ccer) });
    cortex_m::asm::delay(640);
    let staged = pads();
    t.egr().write(|w| w.comg().set_bit());
    cortex_m::asm::delay(640);
    let after = pads();
    super::moe_off();
    t.cr2().modify(|_, w| w.ccpc().clear_bit());
    super::float_all();
    gpio::gates_low_now();
    gpio::enable_set(false);
    let zero = pads();
    Some((before, staged, after, zero))
}
