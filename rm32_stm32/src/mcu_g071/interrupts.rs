//! G071 interrupt vectors — thin wrappers calling shared handlers.

use crate::isr_handlers;
use stm32g0xx_hal::stm32::interrupt;

#[interrupt]
fn TIM6_DAC_LPTIM1() {
    let tim6 = unsafe { &*stm32g0xx_hal::stm32::TIM6::ptr() };
    tim6.sr().modify(|_, w| w.uif().clear_bit());
    isr_handlers::handle_tim6();
}

#[interrupt]
fn TIM14() {
    isr_handlers::handle_tim14();
}

#[interrupt]
fn ADC_COMP() {
    // AM32 Mcu/g071/Src/stm32g0xx_it.c ADC1_COMP_IRQHandler — the same
    // half-average-interval gate + pending-bit camping as the L431 wrapper
    // (parity rung 5b). The previous ack-at-entry policy evaluated every
    // edge immediately, so ringing early in the window passed the
    // persistence filter: binz bring-up ran closed loop but desynced
    // ~11x/s with the commutation rate ~2x the rotor's.
    //
    //   gate OPEN  (TIM2 CNT > average_interval/2): ack, run acceptance.
    //   gate CLOSED, comparator at PRE-ZC level: noise — ack, stay armed.
    //   gate CLOSED, POST-ZC level: camp — pending stays set, NVIC
    //     re-fires until the gate opens (bounded: TIM2 free-runs).
    let exti = unsafe { &*stm32g0xx_hal::stm32::EXTI::ptr() };
    let line = 1 << 18;
    if (exti.rpr1().read().bits() | exti.fpr1().read().bits()) & line == 0 {
        return;
    }
    let avg = {
        let a = crate::comp_gate::get();
        if a != 0 {
            a
        } else {
            let shared = crate::isr::shared();
            rm32::fast_math::div3_i32(shared.e_com_time()).max(0) as u32
        }
    };
    let cnt = unsafe { (*stm32g0xx_hal::stm32::TIM2::ptr()).cnt().read().bits() };
    #[cfg(feature = "benchuart")]
    {
        use core::sync::atomic::Ordering::Relaxed;
        isr_handlers::COMP_ENTRIES.store(
            isr_handlers::COMP_ENTRIES.load(Relaxed).wrapping_add(1),
            Relaxed,
        );
    }
    let ack = || {
        exti.rpr1().write(|w| unsafe { w.bits(line) });
        exti.fpr1().write(|w| unsafe { w.bits(line) });
    };
    if cnt > (avg >> 1) {
        ack();
        isr_handlers::handle_comp();
    } else if isr_handlers::comp_at_pre_zc_level() {
        ack();
        // Edge-swallow race (L431 fall post-mortem): a real crossing
        // between the level read and the clear is re-raised via SWIER.
        if !isr_handlers::comp_at_pre_zc_level() {
            exti.swier1().write(|w| unsafe { w.bits(line) });
        }
    }
    // else: camp.
    #[cfg(feature = "benchuart")]
    if cnt <= (avg >> 1) && !isr_handlers::comp_at_pre_zc_level() {
        use core::sync::atomic::Ordering::Relaxed;
        isr_handlers::COMP_CAMPS.store(
            isr_handlers::COMP_CAMPS.load(Relaxed).wrapping_add(1),
            Relaxed,
        );
    }
}

#[interrupt]
fn DMA1_CHANNEL1() {
    let dma = unsafe { &*stm32g0xx_hal::stm32::DMA1::ptr() };
    if dma.isr().read().tcif1().bit_is_set() {
        dma.ifcr().write(|w| w.cgif1().set_bit());
        dma.ch(0).cr().modify(|_, w| w.en().clear_bit());
        isr_handlers::handle_dma_tc();
        let exti = unsafe { &*stm32g0xx_hal::stm32::EXTI::ptr() };
        exti.swier1().write(|w| unsafe { w.bits(1 << 15) });
    }
    if dma.isr().read().htif1().bit_is_set() {
        dma.ifcr().write(|w| w.chtif1().set_bit());
    }
}

#[interrupt]
fn EXTI4_15() {
    let exti = unsafe { &*stm32g0xx_hal::stm32::EXTI::ptr() };
    exti.rpr1().write(|w| unsafe { w.bits(1 << 15) });
    exti.fpr1().write(|w| unsafe { w.bits(1 << 15) });
    let next_capture = isr_handlers::handle_exti_frame();

    // Apply prescaler change if requested (protocol detection)
    let tim3 = unsafe { &*stm32g0xx_hal::stm32::TIM3::ptr() };
    if let Some(psc) = next_capture.prescaler {
        tim3.psc().write(|w| unsafe { w.bits(psc as u32) });
        tim3.egr().write(|w| w.ug().set_bit());
    }

    // Re-enable DMA
    let dma = unsafe { &*stm32g0xx_hal::stm32::DMA1::ptr() };
    dma.ch(0)
        .ndtr()
        .write(|w| unsafe { w.bits(next_capture.ndtr) });
    dma.ch(0).cr().modify(|_, w| w.en().set_bit());
    tim3.cr1().modify(|_, w| w.cen().set_bit());
}
