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
    // Acknowledge EXTI line 22 (COMP2) FIRST. Otherwise if the inner filter
    // logic in `bemf_zero_cross` decides the edge is noise and early-returns
    // *before* calling `comp.mask_interrupts()` (the only path that clears
    // EXTI.PR1), NVIC sees the bit still set and re-enters this ISR forever.
    // Observed on bench: PR1 stuck at 0x400000, NVIC ISPR2 bit 0 set,
    // 100% CPU in COMP+TIM6 tail-chain, main loop starved.
    let exti = unsafe { &*pac::EXTI::PTR };
    unsafe {
        exti.pr1.write(|w| w.bits(1 << 22));
    }
    isr_handlers::handle_comp();
}

// DMA1 Channel 5: input capture transfer complete
#[interrupt]
fn DMA1_CH5() {
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
}

#[interrupt]
fn EXTI15_10() {
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
}
