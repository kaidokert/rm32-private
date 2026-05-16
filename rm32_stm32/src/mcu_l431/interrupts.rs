//! L431 interrupt vectors — thin wrappers calling shared handlers.
//! L431 uses TIM16 for commutation (shared IRQ with TIM1_UP).

use crate::isr_handlers;
use crate::pac;
use stm32l4xx_hal::pac::interrupt;

#[interrupt]
fn TIM6_DACUNDER() {
    let tim6 = unsafe { &*pac::TIM6::PTR };
    unsafe {
        tim6.sr.write(|w| w.bits(0));
    }
    isr_handlers::handle_tim6();
}

#[interrupt]
fn TIM1_UP_TIM16() {
    // TIM16 is the commutation timer on L431
    let tim16 = unsafe { &*pac::TIM16::PTR };
    unsafe {
        tim16.sr.write(|w| w.bits(0));
    }
    isr_handlers::handle_tim14(); // same logic, different timer
}

#[interrupt]
fn COMP() {
    // AM32 pattern: COMP fires once per commutation phase, the handler
    // masks itself, and the next commutation_timer_expired re-unmasks for
    // the next BEMF detection window. AM32 enforces this implicitly via
    // maskPhaseInterrupts() being called at every motor-stop site (~15
    // places in main.c) plus inside the inner handler on a successful
    // zero-cross detection. rm32 only masked on the zero-cross-detected
    // path; the noise-filter early-return and the various Stop/Disarm
    // transitions left COMP unmasked. With NVIC priorities applied
    // (COMP=0 preempts TIM6=3), an unmasked comparator bouncing on noise
    // (motor coasting on undriven phases, or armed-idle) storms COMP_IRQ
    // and starves TIM6 → firmware freezes.
    //
    // Fix: clear EXTI.PR1[22] AND mask EXTI.IMR1[22] at every ISR exit.
    // commutation_timer_expired re-unmasks when it's time to expect the
    // next zero-cross. The noise-filter early-return is now safe.
    let exti = unsafe { &*pac::EXTI::PTR };
    unsafe {
        exti.pr1.write(|w| w.bits(1 << 22));
        exti.imr1.modify(|r, w| w.bits(r.bits() & !(1 << 22)));
    }
    isr_handlers::handle_comp();
}

// DMA1 Channel 5: input capture transfer complete
#[interrupt]
fn DMA1_CH5() {
    let cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    let dma = unsafe { &*pac::DMA1::PTR };
    let dma_isr = dma.isr.read().bits();
    // Acknowledge ALL CH5 flags up front (CGIF5 = bit 16 in IFCR clears
    // TCIF5/HTIF5/TEIF5/GIF5 in one shot). Without this, if a transfer
    // error (TEIF, bit 19) fires alone without TC, the ISR would return
    // without clearing anything → NVIC re-fires forever (same class of
    // bug as the COMP ISR pre-fix). TEIE is enabled in our CCR5=0x098B,
    // so this path is reachable in principle.
    unsafe {
        dma.ifcr.write(|w| w.bits(1 << 16));
    }
    // Channel 5 TC flag = bit 17 — only process actual transfer complete
    if dma_isr & (1 << 17) != 0 {
        // Disable DMA CH5
        unsafe {
            dma.ccr5.modify(|r, w| w.bits(r.bits() & !1));
        }
        isr_handlers::handle_dma_tc();
        // Trigger software EXTI15
        let exti = unsafe { &*pac::EXTI::PTR };
        unsafe {
            exti.swier1.write(|w| w.bits(1 << 15));
        }
    }
    let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    crate::isr::shared().dbg_dma_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
}

#[interrupt]
fn EXTI15_10() {
    let cyc_start = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    let exti = unsafe { &*pac::EXTI::PTR };
    unsafe {
        exti.pr1.write(|w| w.bits(1 << 15));
    }
    let next_capture = isr_handlers::handle_exti_frame();

    // Apply prescaler change if requested (protocol detection)
    let tim15 = unsafe { &*pac::TIM15::PTR };
    if let Some(psc) = next_capture.prescaler {
        unsafe {
            tim15.psc.write(|w| w.bits(psc as u32));
            tim15.egr.write(|w| w.bits(1)); // UG — latch new PSC immediately
        }
    }

    // Re-enable DMA CH5 for next frame
    let dma = unsafe { &*pac::DMA1::PTR };
    unsafe {
        dma.cndtr5.write(|w| w.bits(next_capture.ndtr));
        dma.ccr5.modify(|r, w| w.bits(r.bits() | 1)); // Enable CH5
    }
    unsafe {
        tim15.cr1.modify(|r, w| w.bits(r.bits() | 1));
    }
    let cyc_end = unsafe { (*cortex_m::peripheral::DWT::PTR).cyccnt.read() };
    crate::isr::shared().dbg_exti_last_cyc_set(cyc_end.wrapping_sub(cyc_start));
}
