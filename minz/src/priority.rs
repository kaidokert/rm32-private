//! NVIC + SCB priority setup for the minz bench prototypes.
//!
//! Modeled after `ref/usb-servo-rs/nuc412/src/priority.rs`, adapted for
//! STM32L431 (Cortex-M4, `__NVIC_PRIO_BITS = 4` — same 4-bit upper-nibble
//! encoding as F4).
//!
//! Three pitfalls this module navigates around — same ones the rm32
//! firmware was bitten by:
//!
//! 1. `cortex_m::NVIC::set_priority(n)` writes the raw byte `n` to IPR.
//!    STM32 only implements the **upper 4 bits** of each priority byte,
//!    so the lower nibble is ignored. A logical level of `1` would land
//!    as effective priority `0` — everything collapses to the same
//!    level. AM32's CMSIS `NVIC_SetPriority` shifts `level << 4`
//!    automatically; cortex-m does not. We always shift explicitly.
//!
//! 2. Writes to `SCB.AIRCR` need the VECTKEY field set to `0x5FA` in
//!    bits 31..16 or the hardware silently drops the write. Set the key
//!    in the same write that programs PRIGROUP.
//!
//! 3. Same upper-nibble encoding applies to SCB SHPR bytes (the system
//!    handlers — SysTick / PendSV / SVCall). Use the same `<<4` helper.
//!
//! The am32_clone programs its own table (COMP=0, TIM1_UP_TIM16=0,
//! TIM6=3, USART2=2 — AM32 peripherals.c:450,491) via
//! [`set_prigroup_preempt4_sub0`] + [`set_irq_prio`].

use crate::hal::pac::Interrupt;
use cortex_m::interrupt::InterruptNumber;
use cortex_m::peripheral::{NVIC, SCB};

const AIRCR_VECTKEY_WRITE: u32 = 0x5FA << 16;
const AIRCR_VECTKEY_MASK: u32 = 0xFFFF << 16;
const AIRCR_PRIGROUP_MASK: u32 = 0x7 << 8;

/// AIRCR.PRIGROUP=3 → 4 preempt bits, 0 sub-priority bits. Matches
/// the AM32 / rm32 firmware setting; with 4 implemented priority bits
/// it means *every* bit is a preempt bit, so logical levels 0..15 all
/// preempt strictly higher logical numbers.
#[inline(always)]
pub unsafe fn set_prigroup_preempt4_sub0() {
    let scb = unsafe { &*SCB::PTR };
    let r = scb.aircr.read();
    let new = (r & !(AIRCR_PRIGROUP_MASK | AIRCR_VECTKEY_MASK)) | AIRCR_VECTKEY_WRITE | (3 << 8);
    unsafe {
        scb.aircr.write(new);
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();
}

/// Encode a logical 0..=15 priority into the upper nibble of an NVIC
/// IPR byte / SCB SHPR byte. STM32L4 only implements the top 4 bits.
#[inline(always)]
pub const fn encode(logical_prio_0_to_15: u8) -> u8 {
    logical_prio_0_to_15 << 4
}

/// Direct IPR write — bypasses `cortex_m::NVIC::set_priority` because
/// that function writes the raw byte without the `<<4` shift.
///
/// # Safety
/// Caller must ensure no other context is reading/writing the same IPR
/// byte. Boot-time use from `main` before NVIC interrupts are unmasked
/// is fine.
pub unsafe fn set_irq_prio(irq: Interrupt, logical: u8) {
    let irqn = irq.number() as usize;
    unsafe {
        (*NVIC::PTR).ipr[irqn].write(encode(logical));
    }
}
