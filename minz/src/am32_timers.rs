//! AM32 INTERVAL_TIMER (TIM2) + COM_TIMER (TIM16) register wrappers,
//! moved verbatim from `examples/am32_clone.rs`.
//!
//! Both timers run at AM32's exact macro semantics
//! (peripherals.c:439-503, peripherals.h:15-26): PSC=39 → 0.5 µs tick.
//! INTERVAL_TIMER is a free-running 16-bit counter reset only on an
//! accepted ZC; COM_TIMER free-runs with ARPE and its update IRQ
//! (TIM1_UP_TIM16 vector) armed one-shot per commutation.
//!
//! Raw register access hits the AM32 macros directly; the HAL has no
//! equivalent for these free-running-with-manual-CNT patterns.

use crate::hal::stm32;

// ===============================================================
// INTERVAL_TIMER = TIM2 (PSC=39 → 0.5 µs).
// ===============================================================

pub fn interval_timer_init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr1
            .modify(|_, w| w.tim2en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(39)); // AM32 peripherals.c:442 TIM2->PSC=39
    tim.arr.write(|w| unsafe { w.bits(0xFFFF) }); // main.c:443 ARR=0xFFFF
    tim.egr.write(|w| w.ug().set_bit());
    tim.cr1.modify(|_, w| w.cen().set_bit());
}

/// INTERVAL_TIMER_COUNT — peripherals.h:15.
#[inline]
pub fn interval_cnt() -> u32 {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.read().bits() & 0xFFFF
}

/// SET_INTERVAL_TIMER_COUNT — peripherals.h:22.
#[inline]
pub fn set_interval_cnt(v: u16) {
    let tim = unsafe { &*stm32::TIM2::ptr() };
    tim.cnt.write(|w| unsafe { w.bits(v as u32) });
}

// ===============================================================
// COM_TIMER = TIM16 (PSC=39).
// ===============================================================

/// MX_TIM16_Init (peripherals.c:485): PSC=39, ARR=0xFFFF, ARPE ON,
/// UIE off at boot. NVIC prio 0 set by the caller. The counter free-runs.
pub fn com_timer_init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb2enr
            .modify(|_, w| w.tim16en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.psc.write(|w| w.psc().bits(39)); // peripherals.c:495 Prescaler=39
    tim.arr.write(|w| unsafe { w.arr().bits(0xFFFF) });
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.modify(|_, w| w.uie().clear_bit()); // DISABLE_COM_TIMER_INT at boot
    // ARPE on (peripherals.c:501 EnableARRPreload), free-run CEN on.
    tim.cr1.modify(|_, w| w.arpe().set_bit().cen().set_bit());
}

/// SET_AND_ENABLE_COM_INT(time) — peripherals.h:19-21:
/// CNT=0, ARR=time, SR=0, DIER.UIE=1.
#[inline]
pub fn set_and_enable_com_int(time: u16) {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.cnt.write(|w| unsafe { w.cnt().bits(0) });
    tim.arr.write(|w| unsafe { w.arr().bits(time) });
    tim.sr.write(|w| unsafe { w.bits(0) });
    tim.dier.modify(|_, w| w.uie().set_bit());
}

/// DISABLE_COM_TIMER_INT() — peripherals.h:17.
#[inline]
pub fn disable_com_timer_int() {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.dier.modify(|_, w| w.uie().clear_bit());
}

/// Write COM_TIMER->ARR directly (zcfoundroutine main.c:1884; vestigial
/// in polling — UIE is off there so it never fires).
#[inline]
pub fn com_set_arr(v: u16) {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.arr.write(|w| unsafe { w.arr().bits(v) });
}

#[inline]
pub fn com_clear_flag() {
    let tim = unsafe { &*stm32::TIM16::ptr() };
    tim.sr.write(|w| unsafe { w.bits(0) });
}
