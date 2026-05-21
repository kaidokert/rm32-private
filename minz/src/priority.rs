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
//! ## Logical levels for this bench (0 = highest)
//!
//! | Lvl | Handler        | Why                                                |
//! |-----|----------------|----------------------------------------------------|
//! | 0   | SysTick        | 10 µs wall-clock tick; must never be preempted     |
//! | 1   | COMP (EXTI22)  | BEMF zero-cross — short, latency-critical          |
//! | 2   | TIM7           | Motor commutation heartbeat — drives the FETs      |
//! | 3   | TIM1_UP_TIM16  | Per-PWM-period COMP2 sampler                       |
//! | 4   | LPTIM1         | Soft-UART RX sample tick (least time-critical)     |
//!
//! `EXTI0` (soft-UART RX start-edge) is **not** assigned here — its
//! reset value is logical `0`, which would preempt SysTick. Either set
//! it explicitly via [`set_irq_prio`] or wire it into [`set_irq_prios`]
//! at the same level as `LPTIM1`.

use crate::hal::pac::Interrupt;
use cortex_m::interrupt::InterruptNumber;
use cortex_m::peripheral::scb::SystemHandler;
use cortex_m::peripheral::{NVIC, SCB};
use rtt_target::rprintln;

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

// Public constants so callers can reference the same numbers when
// dumping or asserting expected values, and so future tweaks live in
// one place.
pub const PRIO_SYSTICK: u8 = 0;
pub const PRIO_COMP: u8 = 1;
pub const PRIO_TIM7: u8 = 2;
pub const PRIO_TIM1: u8 = 3;
pub const PRIO_LPTIM1: u8 = 4;

/// Program PRIGROUP and the full priority table for the motor-tester
/// bench in one shot. Call once from `main` *before* unmasking NVIC
/// interrupts.
///
/// # Safety
/// - Must be called with interrupts disabled (or before any of the
///   affected vectors are unmasked).
/// - Steals the cortex-m `Peripherals` to reach SCB for the SysTick
///   priority write — don't also call `Peripherals::take()` afterwards
///   in the same context.
pub unsafe fn set_irq_prios() {
    unsafe {
        set_prigroup_preempt4_sub0();

        // SysTick lives in SCB SHPR, not NVIC IPR. cortex-m's static
        // `SCB::get_priority` is exposed, but `set_priority` needs
        // `&mut self`, so we steal Peripherals once to reach it.
        let mut cp = cortex_m::Peripherals::steal();
        cp.SCB
            .set_priority(SystemHandler::SysTick, encode(PRIO_SYSTICK));

        set_irq_prio(Interrupt::COMP, PRIO_COMP);
        set_irq_prio(Interrupt::TIM7, PRIO_TIM7);
        set_irq_prio(Interrupt::TIM1_UP_TIM16, PRIO_TIM1);
        set_irq_prio(Interrupt::LPTIM1, PRIO_LPTIM1);
    }
}

/// Read AIRCR back and print PRIGROUP. Expect `PRIGROUP=3` after
/// [`set_prigroup_preempt4_sub0`].
pub fn dump_prigroup() {
    let aircr = unsafe { (*SCB::PTR).aircr.read() };
    let prigroup = (aircr >> 8) & 0x7;
    rprintln!("AIRCR=0x{:08x}  PRIGROUP={}", aircr, prigroup);
}

/// Read back every priority byte we programmed and print it. Expected
/// hex values reflect the `<<4` encoding (logical 0/1/2/3/4 → 0x00 /
/// 0x10 / 0x20 / 0x30 / 0x40).
pub fn dump_irq_prios() {
    let comp = NVIC::get_priority(Interrupt::COMP);
    let tim7 = NVIC::get_priority(Interrupt::TIM7);
    let tim1 = NVIC::get_priority(Interrupt::TIM1_UP_TIM16);
    let lptim1 = NVIC::get_priority(Interrupt::LPTIM1);
    let syst = SCB::get_priority(SystemHandler::SysTick);
    rprintln!("SHPR SysTick     = 0x{:02X} (expect 0x00)", syst);
    rprintln!("NVIC COMP        = 0x{:02X} (expect 0x10)", comp);
    rprintln!("NVIC TIM7        = 0x{:02X} (expect 0x20)", tim7);
    rprintln!("NVIC TIM1_UP_T16 = 0x{:02X} (expect 0x30)", tim1);
    rprintln!("NVIC LPTIM1      = 0x{:02X} (expect 0x40)", lptim1);
}
