//! Real COMP2/EXTI18 adapter in minz polarity. No gate-output access.
use super::*;
use minz_core::am32_hal::{CompExti, Comparator};
use portable_atomic::{AtomicU32, Ordering::Relaxed};
const LINE: u32 = 1 << 18;
static STEP: AtomicU32 = AtomicU32::new(1);
static RISING: AtomicU32 = AtomicU32::new(1);
static COUNT: AtomicU32 = AtomicU32::new(0);
static LAST: AtomicU32 = AtomicU32::new(0);
pub const FILTER_SETTLE_US: u16 = if cfg!(feature = "bench-filter-early-arm") {
    0
} else {
    10
};
#[cfg(feature = "bench-filter-control")]
static FILTERED: AtomicU32 = AtomicU32::new(0);
pub fn filtered() -> bool {
    #[cfg(feature = "bench-filter-control")]
    return FILTERED.load(Relaxed) != 0;
    #[cfg(not(feature = "bench-filter-control"))]
    false
}
#[cfg(feature = "bench-filter-control")]
pub fn prepare_filtered() -> bool {
    cortex_m::interrupt::free(|_| {
        if !filtered_irq_hw::prepare() {
            return false;
        }
        FILTERED.store(1, Relaxed);
        true
    })
}
pub fn hardware_enabled() -> bool {
    #[cfg(feature = "bench-filter-control")]
    if filtered() {
        return filtered_irq_hw::enabled();
    }
    unsafe { (*stm32::EXTI::ptr()).imr1().read().bits() & LINE != 0 }
}
pub struct Input;
impl Comparator for Input {
    fn output_level(&self) -> bool {
        unsafe { core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 }
    }
    fn set_step(&mut self, step: u8, rising: bool) {
        assert!((1..=6).contains(&step));
        STEP.store(step as u32, Relaxed);
        RISING.store(rising as u32, Relaxed);
    }
    fn change_input(&mut self) {
        #[cfg(feature = "bench-filter-control")]
        if filtered() {
            cortex_m::interrupt::free(|_| unsafe {
                let Some(ticket) = filtered_irq_hw::before_mux() else {
                    return;
                };
                let phase = crate::phase_direction::physical_phase(match STEP.load(Relaxed) {
                    1 | 4 => 2,
                    2 | 5 => 0,
                    _ => 1,
                });
                let old = core::ptr::read_volatile(COMP2_CSR);
                core::ptr::write_volatile(
                    COMP2_CSR,
                    (old & !(15 << 4 | 3 << 8 | 1 << 15)) | ((6 + phase as u32) << 4) | (2 << 8),
                );
                // A/B the added capture-disabled wait, not the hardware filter. Earlier
                // arming can include mux transients; core gate/raw persistence still apply.
                if FILTER_SETTLE_US != 0 {
                    let start = t17();
                    while t17().wrapping_sub(start) < FILTER_SETTLE_US {}
                }
                filtered_irq_hw::after_mux(ticket, RISING.load(Relaxed) != 0);
            });
            return;
        }
        self.mask_interrupts();
        #[cfg(feature = "bench-filter-observe")]
        let capture_ticket = filter_observe::before_mux();
        let phase = crate::phase_direction::physical_phase(match STEP.load(Relaxed) {
            1 | 4 => 2,
            2 | 5 => 0,
            _ => 1,
        });
        unsafe {
            let old = core::ptr::read_volatile(COMP2_CSR);
            core::ptr::write_volatile(
                COMP2_CSR,
                (old & !(15 << 4 | 3 << 8 | 1 << 15)) | ((6 + phase as u32) << 4) | (2 << 8),
            );
            let e = &*stm32::EXTI::ptr();
            // minz's rising means raw comparator rises (not rm32 generic inverted HAL).
            e.rtsr1().modify(|r, w| {
                w.bits((r.bits() & !LINE) | if RISING.load(Relaxed) != 0 { LINE } else { 0 })
            });
            e.ftsr1().modify(|r, w| {
                w.bits((r.bits() & !LINE) | if RISING.load(Relaxed) == 0 { LINE } else { 0 })
            });
        }
        // Stock G071 AM32 clears pending when accepting/masking the previous
        // crossing, but does not clear after changeCompInput(). If the newly
        // selected phase is already post-ZC, this mux-latched transition is
        // the only event the half-cycle gate can retain for later service.
        #[cfg(not(feature = "bench-preserve-mux-pending"))]
        self.clear_pending();
        #[cfg(feature = "bench-filter-observe")]
        if let Some(ticket) = capture_ticket {
            filter_observe::after_mux(ticket, RISING.load(Relaxed) != 0, STEP.load(Relaxed));
        }
    }
    fn enable_interrupts(&mut self) {
        unsafe {
            #[cfg(feature = "bench-filter-control")]
            if filtered() {
                filtered_irq_hw::enable_current();
                return;
            }
            (*stm32::EXTI::ptr())
                .imr1()
                .modify(|r, w| w.bits(r.bits() | LINE));
            // Drop stale NVIC state, not EXTI state: a currently pending source will
            // assert again. change_input/core owns clearing the EXTI event itself.
            cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::ADC_COMP);
            cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::ADC_COMP);
        }
    }
    fn mask_interrupts(&mut self) {
        #[cfg(feature = "bench-filter-control")]
        if filtered() {
            filtered_irq_hw::mask();
            return;
        }
        // RM0444 section13.4: IMR controls event latching/wakeup, not cancellation
        // of an already-latched CPU request. Entry094 caught FPR18 set with IMR18=0.
        // This shell exclusively owns ADC_COMP: ADC is polled and COMP1 is off.
        // Do not reuse this vector-wide mask with independent ADC/COMP1 IRQ users.
        cortex_m::peripheral::NVIC::mask(stm32::Interrupt::ADC_COMP);
        unsafe {
            (*stm32::EXTI::ptr())
                .imr1()
                .modify(|r, w| w.bits(r.bits() & !LINE));
        }
    }
}
impl CompExti for Input {
    fn exti_pending(&self) -> bool {
        unsafe {
            #[cfg(feature = "bench-filter-control")]
            if filtered() {
                return filtered_irq_hw::pending();
            }
            let e = &*stm32::EXTI::ptr();
            (e.rpr1().read().bits() | e.fpr1().read().bits()) & LINE != 0
        }
    }
    fn clear_pending(&self) {
        unsafe {
            #[cfg(feature = "bench-filter-control")]
            if filtered() {
                filtered_irq_hw::clear();
                return;
            }
            let e = &*stm32::EXTI::ptr();
            e.rpr1().write(|w| w.bits(LINE));
            e.fpr1().write(|w| w.bits(LINE));
        }
    }
}
pub fn stop() {
    #[cfg(feature = "bench-filter-control")]
    cortex_m::interrupt::free(|_| {
        if filtered() {
            filtered_irq_hw::stop();
            FILTERED.store(0, Relaxed);
        }
    });
    #[cfg(feature = "bench-filter-observe")]
    filter_observe::stop();
    Input.mask_interrupts();
    Input.clear_pending();
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::ADC_COMP);
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::ADC_COMP);
}

/// ENABLE-low proof that `change_input` preserves the transition generated
/// when it removes comparator polarity inversion. This exercises real
/// COMP2/EXTI18 latching while NVIC and every bridge output remain disabled.
#[cfg(feature = "bench-preserve-mux-pending")]
pub fn mux_pending_check<W: Write>(out: &mut W) {
    if get_idr(3, 1) || powered_timer::owns() || !powered_timer::outputs_disabled() {
        let _ = writeln!(out, "!muxpending disabled_only");
        return;
    }
    gates_off();
    set_pin(3, 1, false);
    stop();
    let saved = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    let e = unsafe { &*stm32::EXTI::ptr() };
    let saved_rising = e.rtsr1().read().bits() & LINE;
    let saved_falling = e.ftsr1().read().bits() & LINE;

    Input.set_step(1, true);
    Input.change_input();
    cortex_m::asm::delay(640);
    let base = Input.output_level();
    Input.set_step(1, base);
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, core::ptr::read_volatile(COMP2_CSR) | (1 << 15));
    }
    cortex_m::asm::delay(640);
    let inverted = Input.output_level();
    Input.clear_pending();
    let before = Input.exti_pending();
    Input.change_input();
    cortex_m::asm::delay(640);
    let after_level = Input.output_level();
    let rising_pending = e.rpr1().read().bits() & LINE != 0;
    let falling_pending = e.fpr1().read().bits() & LINE != 0;
    let pending = Input.exti_pending();

    stop();
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, saved);
        e.rtsr1()
            .modify(|r, w| w.bits((r.bits() & !LINE) | saved_rising));
        e.ftsr1()
            .modify(|r, w| w.bits((r.bits() & !LINE) | saved_falling));
    }
    let safe = !get_idr(3, 1) && powered_timer::outputs_disabled();
    let passed = !before && inverted != base && after_level == base && pending && safe;
    let _ = writeln!(
        out,
        "MUXPENDING passed={} before={} base={} inverted={} after={} rpr={} fpr={} pending={} enable=0 outputs_off={}",
        passed as u8,
        before as u8,
        base as u8,
        inverted as u8,
        after_level as u8,
        rising_pending as u8,
        falling_pending as u8,
        pending as u8,
        safe as u8
    );
}

pub fn interrupt() {
    if !Input.exti_pending() {
        return;
    }
    let e = unsafe { &*stm32::EXTI::ptr() };
    let flags = u32::from(e.rpr1().read().bits() & LINE != 0)
        | (u32::from(e.fpr1().read().bits() & LINE != 0) << 1)
        | ((Input.output_level() as u32) << 2);
    // Diagnostic capture is one-shot: bounded even on a noisy idle input.
    Input.mask_interrupts();
    Input.clear_pending();
    LAST.store(flags, Relaxed);
    COUNT.store(COUNT.load(Relaxed) + 1, Relaxed);
}
pub fn diagnostic<W: Write>(out: &mut W, internal_ref: bool) {
    gates_off();
    set_pin(3, 1, false);
    stop();
    let saved = unsafe { core::ptr::read_volatile(COMP2_CSR) };
    unsafe {
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::ADC_COMP);
    }
    for step in 1..=6u8 {
        Input.set_step(step, step & 1 != 0);
        Input.change_input();
        if internal_ref {
            unsafe {
                let old = core::ptr::read_volatile(COMP2_CSR);
                // INM3 = full VREFINT; leave external neutral on INP for stable separation.
                core::ptr::write_volatile(COMP2_CSR, (old & !(15 << 4)) | (3 << 4));
            }
        }
        let csr = unsafe { core::ptr::read_volatile(COMP2_CSR) };
        let mut passed = 0;
        let mut timeout = 0;
        for _ in 0..8 {
            Input.mask_interrupts();
            // Polarity inversion forces a real output transition without changing
            // external wiring. This validates EXTI routing, NOT physical BEMF.
            let expected = step & 1 != 0;
            unsafe {
                core::ptr::write_volatile(COMP2_CSR, csr & !(1 << 15));
            }
            cortex_m::asm::delay(640);
            let base = Input.output_level();
            let pre = if base == expected {
                csr ^ (1 << 15)
            } else {
                csr & !(1 << 15)
            };
            unsafe {
                core::ptr::write_volatile(COMP2_CSR, pre);
            }
            cortex_m::asm::delay(640);
            Input.clear_pending();
            let before = COUNT.load(Relaxed);
            Input.enable_interrupts();
            unsafe {
                core::ptr::write_volatile(COMP2_CSR, pre ^ (1 << 15));
            }
            let start = t17();
            while COUNT.load(Relaxed) == before && t17().wrapping_sub(start) < 500 {}
            Input.mask_interrupts();
            if COUNT.load(Relaxed) == before {
                timeout += 1;
            } else if LAST.load(Relaxed) == if expected { 5 } else { 2 } {
                passed += 1;
            }
        }
        let _ = writeln!(
            out,
            "COMPIRQ step={} mux={} rising={} n=8 pass={} timeout={} last={} forced_polarity=1 ref={}",
            step,
            (csr >> 4) & 15,
            step & 1,
            passed,
            timeout,
            LAST.load(Relaxed),
            internal_ref as u8
        );
    }
    stop();
    unsafe {
        core::ptr::write_volatile(COMP2_CSR, saved);
    }
    gates_off();
    set_pin(3, 1, false);
}
