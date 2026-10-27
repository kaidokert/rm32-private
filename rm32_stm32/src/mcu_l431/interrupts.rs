//! L431 interrupt vectors — thin wrappers calling shared handlers.
//! L431 uses TIM16 for commutation (shared IRQ with TIM1_UP).

use crate::diag::{DIAG, DiagSink, IsrSite};
use crate::isr_handlers;
use crate::pac;
use stm32l4xx_hal::pac::interrupt;

// Bench UART RX. Ring-push-only — MUST NOT touch ISR_LOCAL (see
// notes/ISR_STATE_INVARIANT.md); any priority is aliasing-safe.
#[cfg(feature = "benchuart")]
#[interrupt]
fn USART2() {
    crate::bench_uart::service_rx();
}

// PA0 soft-UART RX (debuguart bench input): edge qualifier + bit
// sampler, both level 3. Neither touches ISR_LOCAL.
#[cfg(feature = "debuguart")]
#[interrupt]
fn EXTI0() {
    super::softuart_rx::on_exti0();
}

#[cfg(feature = "debuguart")]
#[interrupt]
fn LPTIM1() {
    super::softuart_rx::on_lptim1();
}

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
    // NOTE: never print from this ISR. A 1-in-1024 `[t16]` dprintln that
    // lived here was THE 40-50% fall trigger: ~30 bytes of bounded TXE
    // waits at 2 Mbaud = ~180 µs of priority-0 stall BEFORE the phase
    // switch -> commutation fires ~365 TIM2 ticks late -> the next ZC
    // passes while COMP is still masked -> deaf window -> period-2
    // spiral. Captured end-to-end in spiral1.bin (tl=471 vs la=102).
    unsafe {
        tim16.sr.write(|w| w.bits(0));
    }
    #[cfg(feature = "benchuart")]
    LEAN_COMMS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    isr_handlers::handle_tim14(); // same logic, different timer
}

/// Lean camp-storm probe (clone-style single counters, always on
/// under benchuart): COMP ISR entries + commutations. Lets a build
/// WITHOUT blackbox/zctrace still measure entries/window, to test
/// whether rm32's per-commutation prio-0 instrumentation load is the
/// storm-entry differential.
#[cfg(feature = "benchuart")]
pub static LEAN_COMP_ENTRIES: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "benchuart")]
pub static LEAN_COMMS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[interrupt]
fn COMP() {
    // AM32 stm32l4xx_it.c:276-290: the core's half-average-interval gate
    // with pending-bit camping (parity rung 5b, replacing mask-at-entry,
    // which took the first edge in every window). Storm safety: the gate
    // absorbs early edges, accepts mask the line, and ten_khz_tick masks
    // COMP every tick while !running; the comparator is off in polling.
    isr_handlers::handle_comp_irq();
}

// DMA1 Channel 5: input capture transfer complete
#[interrupt]
fn DMA1_CH5() {
    let cyc_start = DIAG.stamp();
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
    DIAG.bracket(IsrSite::InputDma, cyc_start);
}

#[interrupt]
fn EXTI15_10() {
    let cyc_start = DIAG.stamp();
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
    DIAG.bracket(IsrSite::InputFrame, cyc_start);
}
