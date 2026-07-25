//! L431 interrupt vectors — thin wrappers calling shared handlers.
//! L431 uses TIM16 for commutation (shared IRQ with TIM1_UP).

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
    // TEMP DIAG (edge-probe rung): 1-in-1024 raw snapshot of the fire —
    // TIM16.ARR / TIM16.CNT / TIM2.CNT at entry, to explain why fires
    // land at TIM2~7 when the arm value was wait+1 (~88).
    #[cfg(feature = "zctrace")]
    {
        use core::sync::atomic::{AtomicU32, Ordering};
        static FIRE_N: AtomicU32 = AtomicU32::new(0);
        let n = FIRE_N.fetch_add(1, Ordering::Relaxed);
        if n & 0x3FF == 0 {
            let arr = tim16.arr.read().bits();
            let cnt16 = tim16.cnt.read().bits();
            let cnt2 = unsafe { (*pac::TIM2::PTR).cnt.read().bits() };
            crate::dprintln!("[t16 arr={} cnt16={} cnt2={}]", arr, cnt16, cnt2);
        }
    }
    unsafe {
        tim16.sr.write(|w| w.bits(0));
    }
    isr_handlers::handle_tim14(); // same logic, different timer
}

#[interrupt]
fn COMP() {
    // AM32 stm32l4xx_it.c:276-290 — the half-average-interval acceptance
    // gate + pending-bit camping (parity rung 5b; this replaces the
    // mask-at-entry policy that made rm32 take the FIRST edge in every
    // window and lose the window on a persistence reject):
    //
    //   gate OPEN  (interval CNT > average_interval/2): ack the line and
    //     run the acceptance path. bemf_zero_cross masks the comparator
    //     itself on accept; a persistence reject stays UNMASKED and armed
    //     for the true crossing later in the window.
    //   gate CLOSED, comparator at PRE-ZC level: a noise blip — ack it
    //     and stay armed.
    //   gate CLOSED, comparator at POST-ZC level: CAMP — leave the
    //     pending bit set so NVIC re-fires this ISR until the gate opens
    //     and the (early) crossing is evaluated. Bounded: TIM2 free-runs,
    //     so CNT crosses avg/2 in at most avg/2 ticks.
    //
    // Storm safety without mask-at-entry: the gate absorbs early edges,
    // accepts mask the line, and ten_khz_tick masks COMP every tick while
    // !running (the Armed-idle storm path). The comparator is also no
    // longer enabled at all during polling mode (exclusivity, isr_logic).
    let exti = unsafe { &*pac::EXTI::PTR };
    if exti.pr1.read().bits() & (1 << 22) == 0 {
        return;
    }
    let shared = crate::isr::shared();
    // Gate avg: 20 kHz-latched (AM32-verbatim staleness; see
    // edge_probe::gate_avg). Fallback to fresh when the latch is cold.
    #[cfg(feature = "zctrace")]
    let avg = {
        let a = crate::edge_probe::gate_avg();
        if a != 0 {
            a
        } else {
            (shared.e_com_time() / 3).max(0) as u32
        }
    };
    #[cfg(not(feature = "zctrace"))]
    let avg = (shared.e_com_time() / 3).max(0) as u32;
    let cnt = unsafe { (*pac::TIM2::PTR).cnt.read().bits() };
    // Edge probe: every confirmed-pending entry counts (camp re-fires
    // included — this is the storm meter), first edge time captured.
    #[cfg(feature = "zctrace")]
    crate::edge_probe::edge_seen(cnt);
    if cnt > (avg >> 1) {
        unsafe { exti.pr1.write(|w| w.bits(1 << 22)) };
        isr_handlers::handle_comp();
    } else if isr_handlers::comp_at_pre_zc_level() {
        unsafe { exti.pr1.write(|w| w.bits(1 << 22)) };
        #[cfg(feature = "zctrace")]
        crate::edge_probe::gated_clear();
    }
    // else: camp — pending stays set, ISR re-fires until the gate opens.
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
