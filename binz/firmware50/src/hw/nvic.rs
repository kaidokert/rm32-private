//! The NVIC operations the firmware uses, as safe functions (notebook E113).
//!
//! `cortex_m`'s `NVIC::unmask` and `set_priority` are `unsafe` because
//! enabling an interrupt can break a critical section built on masking it.
//! This crate never builds critical sections that way (it uses
//! `cortex_m::interrupt::free`, PRIMASK), so every handler here is sound to
//! enable in any state; that one argument is made here, once, instead of at
//! each call site.

use stm32g0xx_hal::stm32::Interrupt;

/// Set a priority byte. The M0+ implements the **top two bits** only, so pass
/// it pre-shifted (0x00, 0x40, 0x80, 0xC0); `cortex_m` writes the raw byte.
pub fn set_priority(irq: Interrupt, prio: u8) {
    // SAFETY: the NVIC is not owned anywhere else in this crate (the HAL's
    // `Peripherals::take` is never called for it).
    let mut nvic = unsafe { cortex_m::Peripherals::steal() }.NVIC;
    // SAFETY: see the module docs; priorities are written only at setup.
    unsafe { nvic.set_priority(irq, prio) };
}

#[inline(always)]
pub fn unmask(irq: Interrupt) {
    // SAFETY: see the module docs.
    unsafe { cortex_m::peripheral::NVIC::unmask(irq) };
}

#[inline(always)]
pub fn mask(irq: Interrupt) {
    cortex_m::peripheral::NVIC::mask(irq);
}

#[inline(always)]
pub fn unpend(irq: Interrupt) {
    cortex_m::peripheral::NVIC::unpend(irq);
}

#[inline(always)]
pub fn pend(irq: Interrupt) {
    cortex_m::peripheral::NVIC::pend(irq);
}
