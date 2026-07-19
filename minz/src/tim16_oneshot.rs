//! TIM16 as the commutation timer — LITERALLY AM32's COM_TIMER
//! recipe on this chip (operator directive 2026-07-19: same timer,
//! same semantics, no ifs or buts).
//!
//! AM32-L431 / production-rm32 semantics (verified in the rm32
//! register-parity notes): TIM16 FREE-RUNNING from boot
//! (CR1 = ARPE|CEN), 0.5 µs tick (PSC=39 at 80 MHz). Scheduling a
//! commutation = write ARR (shadowed via ARPE) then EGR.UG to
//! force-load and zero the counter; the update interrupt at the
//! wrap IS the commutation. No one-shot enable/disable, no
//! cross-clock-domain sync, nothing to swallow — the free-run is
//! what gives the smooth motor, and the dropped-shot class cannot
//! exist.
//!
//! WHY the move (the dropped-shot autopsy): LPTIM2's async kernel
//! domain silently swallowed armed shots (ARM_RING caught a 20 µs
//! arm never firing at a clean 111 µs cruise → chain silence → blind
//! kick commutation at 75 % duty → 10-13 A surge → bus fold). Every
//! chain-rescue layer was compensation for that peripheral.
//!
//! TIM16 shares the `TIM1_UP_TIM16` vector with the 24 kHz TIM1
//! wrap: the handler dispatches on TIM16.SR.UIF FIRST (commutation
//! before telemetry), exactly like AM32's own shared handler.

use crate::hal::stm32;

/// 0.5 µs tick — AM32's COM_TIMER grain (PSC = 39 at 80 MHz).
const TICKS_PER_US: u32 = 2;

pub fn init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb2enr
            .modify(|_, w| w.tim16en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(39));
    // Park ARR at max so the free-run wraps rarely until the first
    // real schedule; URS: EGR.UG must not fire the update interrupt
    // (only a genuine overflow = the commutation instant does).
    tim.arr.write(|w| w.arr().bits(0xFFFF));
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.write(|w| w.uie().set_bit());
    // AM32: ARPE + CEN, free-running from boot, never stopped.
    tim.cr1
        .modify(|_, w| w.arpe().set_bit().urs().set_bit().cen().set_bit());
}

/// Schedule the next commutation `us` microseconds from NOW —
/// AM32's `set_and_enable`: ARR write (ARPE-shadowed) + EGR.UG
/// (force-load + counter zero). The timer keeps free-running; the
/// update IRQ at the wrap is the commutation. Safe from any
/// context; a later call replaces the pending shot.
#[inline]
pub fn schedule_us(us: u32) {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    let ticks = (us * TICKS_PER_US).clamp(4, 0xFFFE) as u16;
    tim.arr.write(|w| w.arr().bits(ticks));
    tim.egr.write(|w| w.ug().set_bit());
}

/// Same operation — the AM32 recipe has no warm/cold split. Alias so
/// existing call sites read unchanged.
#[inline]
pub fn reschedule_light(us: u32) {
    schedule_us(us);
}

/// Shared-vector dispatch test: true + UIF acknowledged when TIM16's
/// wrap (the commutation) fired.
#[inline]
pub fn fired_and_clear() -> bool {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    if tim.sr.read().uif().bit_is_set() {
        tim.sr.write(|w| unsafe { w.bits(0) });
        true
    } else {
        false
    }
}

/// Kill path: park the wrap far away and clear any pending flag
/// (the free-run itself never stops — AM32 masks/parks likewise).
#[inline]
pub fn cancel() {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.arr.write(|w| w.arr().bits(0xFFFF));
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.write(|w| unsafe { w.bits(0) });
}
