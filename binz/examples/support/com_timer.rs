//! G071 COM timer adapter. Explicit immediate ARR; no motor-output authority.
use super::*;
use cortex_m::peripheral::NVIC;
use minz_core::am32_hal::{ComTimer, ComTimerExt};
use portable_atomic::{AtomicU32, Ordering::Relaxed};

static IRQ_TIME: AtomicU32 = AtomicU32::new(0);
static IRQ_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "bench-prepared-handoff")]
static RECOVERY_PREPARED: portable_atomic::AtomicBool = portable_atomic::AtomicBool::new(false);

#[cfg(feature = "bench-prepared-handoff")]
pub fn prepare_recovery_counter() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        if core_bench::active()
            || !powered_timer::owns()
            || !powered_timer::ready()
            || !powered_timer::outputs_disabled()
        {
            return false;
        }
        // observe_begin initializes PSC/ARR before acquisition. Intermediate
        // stop() revokes activity but deliberately preserves that configuration.
        // Revalidate it; do not repeat RCC/PSC/UG setup during continuous sensing.
        let t = &*stm32::TIM16::ptr();
        if t.cr1().read().bits() != 0
            || t.dier().read().bits() != 0
            || t.sr().read().bits() & 1 != 0
            || t.psc().read().bits() != 31
            || t.arr().read().bits() != 65535
        {
            return false;
        }
        t.cnt().write(|w| w.bits(0));
        t.cr1().write(|w| w.bits(1));
        RECOVERY_PREPARED.store(true, Relaxed);
        true
    })
}
/// Called under the final handoff critical section. Publish once; caller still
/// owns controller metadata and the final unmask. Existing32us admission stays.
#[cfg(feature = "bench-prepared-handoff")]
pub fn publish_recovery_deadline(origin: u16, deadline: u32) -> bool {
    if !RECOVERY_PREPARED.swap(false, Relaxed)
        || !powered_timer::owns()
        || !powered_timer::ready()
        || !powered_timer::outputs_disabled()
    {
        Timer::stop();
        return false;
    }
    // observe_irq_start may have unmasked this vector; no DIER has been armed.
    NVIC::mask(stm32::Interrupt::TIM16);
    let before = t17().wrapping_sub(origin) as u32 * 2;
    let count = unsafe { (*stm32::TIM16::ptr()).cnt().read().bits() as u16 };
    let after = t17().wrapping_sub(origin) as u32 * 2;
    let Ok(mapped) = prepared_policy::map_deadline(before, after, count, deadline, 16) else {
        Timer::stop();
        return false;
    };
    if publish_prepared(mapped.arr) != 0 {
        return false;
    }
    unsafe {
        cortex_m::Peripherals::steal()
            .NVIC
            .set_priority(stm32::Interrupt::TIM16, 0);
    }
    true
}

pub struct Timer;
/// Actual guard + DMA feedback interrupts, diagnostic COM ISR only. No gate
/// commit, no synthetic current feedback, and no motor command.
#[cfg(feature = "bench-prepared-load")]
pub fn prepared_load_check<W: Write>(out: &mut W, vcal: u32, priority: u8) {
    if core_bench::active()
        || powered_timer::owns()
        || driven_run::owns()
        || !core_bench::bridge_disabled()
    {
        let _ = writeln!(out, "PREPAREDLOAD refused=1");
        return;
    }
    let mut failures = 0;
    let mut trials = 0;
    let mut events = 0;
    let mut max_late = 0u16;
    let mut max_dma = 0u8;
    let mut min_feedback = u32::MAX;
    let mut refusal = 0u32;
    for offset in 0..32u16 {
        powered_timer::prepare();
        if !powered_timer::wake(&mut || false) {
            refusal = 101;
            break;
        }
        let acquired = t17();
        let Some(sample) = powered_timer::sample_feedback(vcal) else {
            refusal = 102;
            break;
        };
        let age = t17().wrapping_sub(acquired) as u32;
        if !powered_timer::start_prepared(0, 1, sample, age, 20_000) {
            refusal = 103;
            break;
        }
        if !powered_timer::service_feedback(vcal) {
            refusal = 104;
            break;
        }
        Timer::init();
        unsafe {
            cortex_m::Peripherals::steal()
                .NVIC
                .set_priority(stm32::Interrupt::TIM16, priority);
        }
        let t = unsafe { &*stm32::TIM16::ptr() };
        let origin = cortex_m::interrupt::free(|_| unsafe {
            let origin = t17();
            t.cr1().write(|w| w.bits(1));
            origin
        });
        let target_us = 100 + offset * 7; // sweep relative to201us real ADC scan period
        let before_events = IRQ_COUNT.load(Relaxed);
        let mapped = cortex_m::interrupt::free(|_| {
            let before = t17().wrapping_sub(origin) as u32 * 2;
            let count = t.cnt().read().bits() as u16;
            let after = t17().wrapping_sub(origin) as u32 * 2;
            prepared_policy::map_deadline(before, after, count, target_us as u32 * 2, 16)
        });
        let Ok(mapped) = mapped else {
            refusal = 105;
            break;
        };
        let code = publish_prepared(mapped.arr);
        if code != 0 {
            refusal = code;
            break;
        }
        unsafe {
            NVIC::unmask(stm32::Interrupt::TIM16);
        }
        while t17().wrapping_sub(origin) < 600 && powered_timer::owns() {
            if !powered_timer::service_feedback(vcal) {
                break;
            }
        }
        let count = IRQ_COUNT.load(Relaxed).wrapping_sub(before_events);
        let stamp = (IRQ_TIME.load(Relaxed) as u16).wrapping_sub(origin);
        let stats = powered_timer::stopped_snapshot();
        trials += 1;
        events += count;
        if count != 1 || stamp < target_us || stats[0] != 0 || !powered_timer::outputs_disabled() {
            failures += 1;
        }
        max_late = max_late.max(stamp.saturating_sub(target_us));
        max_dma = max_dma.max(adc_stream::max_us());
        min_feedback = min_feedback.min(stats[3]);
        Timer::stop();
        gates_off();
        set_pin(3, 1, false);
        adc_stream::stop();
        if failures != 0 {
            break;
        }
    }
    Timer::stop();
    gates_off();
    set_pin(3, 1, false);
    adc_stream::stop();
    let _ = writeln!(
        out,
        "PREPAREDLOAD trials={} events={} failures={} refusal={} max_late_us={} max_dma_us={} min_feedback={} disabled={} diagnostic_com=1 actual_guard_dma=1 priority={}",
        trials,
        events,
        failures,
        refusal,
        max_late,
        max_dma,
        min_feedback,
        core_bench::bridge_disabled() as u8,
        priority
    );
}
#[cfg(any(feature = "bench-prepared-timer", feature = "bench-prepared-handoff"))]
#[path = "prepared_handoff.rs"]
mod prepared_policy;
/// Prototype primitive: timer is already running, NVIC remains masked.
/// No gate authority and no use in the powered handoff yet.
#[cfg(any(feature = "bench-prepared-timer", feature = "bench-prepared-handoff"))]
fn publish_prepared(arr: u16) -> u32 {
    let accepted = cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM16::ptr();
        let target = arr as u32 + 1;
        if NVIC::is_enabled(stm32::Interrupt::TIM16) {
            return 1;
        }
        if NVIC::is_pending(stm32::Interrupt::TIM16) {
            return 2;
        }
        if t.cr1().read().bits() & 0x81 != 1 {
            return 3;
        }
        if t.dier().read().bits() != 0 {
            return 4;
        }
        if t.sr().read().bits() & 1 != 0 {
            return 5;
        }
        let count = t.cnt().read().bits();
        // Disabled-test allocation only. Live full-arm floor remains32us.
        if target.checked_sub(count).is_none_or(|left| left < 16) {
            return 6;
        }
        t.arr().write(|w| w.bits(arr as u32));
        if t.cnt().read().bits() >= target || t.sr().read().bits() & 1 != 0 {
            return 7;
        }
        t.dier().write(|w| w.bits(1));
        0
    });
    if accepted != 0 {
        Timer::stop();
    }
    accepted
}

/// Disabled-only absolute ARR/deadline/refusal exercise. Never unmask COM.
#[cfg(feature = "bench-prepared-timer")]
pub fn prepared_check<W: Write>(out: &mut W) {
    #[cfg(feature = "bench-driven-power")]
    let driven = driven_run::owns();
    #[cfg(not(feature = "bench-driven-power"))]
    let driven = false;
    if core_bench::active() || powered_timer::owns() || driven || !core_bench::bridge_disabled() {
        let _ = writeln!(out, "PREPAREDCHECK refused=1");
        return;
    }
    gates_off();
    set_pin(3, 1, false);
    let mut failed = 0u32;
    let mut accepted = 0u32;
    let mut refused = 0u32;
    let mut max_publish = 0u16;
    let mut max_origin = 0u16;
    let mut max_error = 0u16;
    let mut max_mapping = 0u16;
    for mode in 0..6 {
        for _ in 0..32 {
            Timer::init();
            let t = unsafe { &*stm32::TIM16::ptr() };
            let (origin, span) = cortex_m::interrupt::free(|_| unsafe {
                let before = t17();
                t.cr1().write(|w| w.bits(1));
                (before, t17().wrapping_sub(before))
            });
            max_origin = max_origin.max(span);
            while t.cnt().read().bits() < 100 && t17().wrapping_sub(origin) < 1000 {}
            let mut arr = 199u16;
            if mode == 5 {
                let mapping_start = t17();
                let mapped = cortex_m::interrupt::free(|_| {
                    let before = t17().wrapping_sub(origin) as u32 * 2;
                    let count = t.cnt().read().bits() as u16;
                    let after = t17().wrapping_sub(origin) as u32 * 2;
                    prepared_policy::map_deadline(before, after, count, 200, 16)
                });
                max_mapping = max_mapping.max(t17().wrapping_sub(mapping_start));
                match mapped {
                    Ok(value) => arr = value.arr,
                    Err(_) => {
                        failed += 1;
                        Timer::stop();
                        continue;
                    }
                }
            }
            unsafe {
                match mode {
                    1 => {
                        arr = t.cnt().read().bits() as u16;
                    } // deadline already too close
                    2 => {
                        NVIC::pend(stm32::Interrupt::TIM16);
                    }
                    3 => {
                        t.egr().write(|w| w.bits(1));
                    } // stale peripheral UIF
                    4 => {
                        t.cr1().write(|w| w.bits(0));
                    }
                    _ => {}
                }
            }
            let start = t17();
            let reason = publish_prepared(arr);
            let ok = reason == 0;
            max_publish = max_publish.max(t17().wrapping_sub(start));
            if mode == 0 && !ok && refused == 0 {
                let _ = writeln!(out, "PREPAREDREFUSAL reason={}", reason);
            }
            if ok {
                accepted += 1;
                if mode != 0 && mode != 5 {
                    failed += 1;
                }
                while t.sr().read().bits() & 1 == 0 && t17().wrapping_sub(origin) < 1000 {}
                let elapsed = t17().wrapping_sub(origin);
                max_error = max_error.max(elapsed.abs_diff(100));
                if t.sr().read().bits() & 1 == 0 || elapsed < 100 || elapsed > 103 {
                    failed += 1;
                }
            } else {
                refused += 1;
                if mode == 0 || mode == 5 {
                    failed += 1;
                }
            }
            Timer::stop();
            if t.cr1().read().bits() & 1 != 0
                || t.dier().read().bits() != 0
                || NVIC::is_pending(stm32::Interrupt::TIM16)
                || NVIC::is_enabled(stm32::Interrupt::TIM16)
            {
                failed += 1;
            }
        }
    }
    gates_off();
    set_pin(3, 1, false);
    let _ = writeln!(
        out,
        "PREPAREDMAP trials=32 max_mapping_us={} quantization_bound_ticks=4",
        max_mapping
    );
    let _ = writeln!(
        out,
        "PREPAREDCHECK trials=192 accepted={} refused={} failed={} max_publish_us={} origin_span_us={} deadline_error_us={} disabled={} irq_masked=1",
        accepted,
        refused,
        failed,
        max_publish,
        max_origin,
        max_error,
        core_bench::bridge_disabled() as u8
    );
}
/// Disabled-only timer register exercise. No COM ISR or output authority.
#[cfg(feature = "bench-com-keep-running")]
pub fn arm_check<W: Write>(out: &mut W) {
    if core_bench::active() || powered_timer::owns() || !core_bench::bridge_disabled() {
        let _ = writeln!(out, "COMARMCHECK refused=1");
        return;
    }
    let mut failed = 0u32;
    let mut trials = 0u32;
    let mut max_arm = 0u16;
    let mut min_slack = u16::MAX;
    for mode in 0..4 {
        for timeout in [63u16, 399] {
            for _ in 0..16 {
                Timer::init(); // also masks NVIC; pending ISR must never run
                let t = unsafe { &*stm32::TIM16::ptr() };
                unsafe {
                    if mode != 0 {
                        t.cr1().write(|w| w.bits(1));
                    }
                    if mode == 2 {
                        t.egr().write(|w| w.bits(1));
                        NVIC::pend(stm32::Interrupt::TIM16);
                    }
                }
                if mode == 3 {
                    Timer::stop();
                }
                let start = t17();
                Timer.set_and_enable(timeout);
                let armed = t17().wrapping_sub(start);
                max_arm = max_arm.max(armed);
                let early =
                    t.sr().read().bits() & 1 != 0 || NVIC::is_pending(stm32::Interrupt::TIM16);
                let registers = t.cr1().read().bits() & 1 == 1
                    && t.dier().read().bits() == 1
                    && t.arr().read().bits() == timeout as u32;
                while t.sr().read().bits() & 1 == 0 && t17().wrapping_sub(start) < 1000 {}
                let elapsed = t17().wrapping_sub(start);
                let expected = (timeout + 1) / 2;
                if early
                    || !registers
                    || armed > 10
                    || elapsed < expected
                    || elapsed > expected + 20
                {
                    failed += 1;
                }
                min_slack = min_slack.min(elapsed.saturating_sub(armed));
                Timer::stop();
                if t.cr1().read().bits() & 1 != 0
                    || t.dier().read().bits() != 0
                    || NVIC::is_pending(stm32::Interrupt::TIM16)
                {
                    failed += 1;
                }
                trials += 1;
            }
        }
    }
    let _ = writeln!(
        out,
        "COMARMCHECK trials={} modes=4 arr_short=63 arr_long=399 failed={} max_arm_us={} min_slack_us={} disabled={} irq_masked=1 gate_authority=0",
        trials,
        failed,
        max_arm,
        min_slack,
        core_bench::bridge_disabled() as u8
    );
}
impl Timer {
    pub fn init() {
        NVIC::mask(stm32::Interrupt::TIM16);
        unsafe {
            let rcc = &*stm32::RCC::ptr();
            rcc.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 17)));
            let t = &*stm32::TIM16::ptr();
            t.cr1().write(|w| w.bits(0)); // ARPE off: ARR writes take effect now
            t.dier().write(|w| w.bits(0));
            t.ccer().write(|w| w.bits(0));
            t.bdtr().write(|w| w.bits(0));
            t.psc().write(|w| w.bits(31));
            t.arr().write(|w| w.bits(65535));
            t.egr().write(|w| w.bits(1));
            t.sr().write(|w| w.bits(0));
        }
        NVIC::unpend(stm32::Interrupt::TIM16);
    }
    pub fn stop() {
        #[cfg(feature = "bench-prepared-handoff")]
        RECOVERY_PREPARED.store(false, Relaxed);
        NVIC::mask(stm32::Interrupt::TIM16);
        unsafe {
            let t = &*stm32::TIM16::ptr();
            t.dier().write(|w| w.bits(0));
            t.cr1().write(|w| w.bits(0));
            t.sr().write(|w| w.bits(0));
        }
        NVIC::unpend(stm32::Interrupt::TIM16);
    }
}
impl ComTimer for Timer {
    fn set_and_enable(&mut self, timeout: u16) {
        cortex_m::interrupt::free(|_| unsafe {
            let t = &*stm32::TIM16::ptr();
            // Experimental running-counter arm. Initial/stopped timer still
            // starts at the final CEN write; shutdown/init remain unchanged.
            // Retain DIER exclusion and stale peripheral/NVIC pending clears.
            #[cfg(not(feature = "bench-com-keep-running"))]
            t.cr1().write(|w| w.bits(0));
            t.dier().write(|w| w.bits(0));
            t.cnt().write(|w| w.bits(0));
            t.arr().write(|w| w.bits(timeout as u32));
            t.sr().write(|w| w.bits(0));
            NVIC::unpend(stm32::Interrupt::TIM16);
            t.dier().write(|w| w.bits(1));
            t.cr1().write(|w| w.bits(1));
        });
    }
    fn disable_interrupt(&mut self) {
        unsafe {
            (*stm32::TIM16::ptr()).dier().write(|w| w.bits(0));
        }
    }
    fn enable_interrupt(&mut self) {
        unsafe {
            (*stm32::TIM16::ptr()).dier().write(|w| w.bits(1));
        }
    }
}
impl ComTimerExt for Timer {
    fn com_set_arr(&mut self, arr: u16) {
        unsafe {
            (*stm32::TIM16::ptr()).arr().write(|w| w.bits(arr as u32));
        }
    }
    fn com_clear_flag(&mut self) {
        unsafe {
            (*stm32::TIM16::ptr()).sr().write(|w| w.bits(0));
        }
    }
}

// Diagnostic ISR only. Replace with qualified core COM service when explicitly
// integrating the observe-only adapter. Never invokes sixstep_apply or MOE.
pub fn interrupt() {
    let stamp = t17();
    let t = unsafe { &*stm32::TIM16::ptr() };
    if t.sr().read().bits() & 1 != 0 && t.dier().read().bits() & 1 != 0 {
        Timer.disable_interrupt();
        Timer.com_clear_flag();
        IRQ_TIME.store(stamp as u32, Relaxed);
        IRQ_COUNT.store(IRQ_COUNT.load(Relaxed) + 1, Relaxed);
    }
}

pub fn diagnostic<W: Write>(out: &mut W) {
    gates_off();
    set_pin(3, 1, false);
    Timer::init();
    unsafe {
        NVIC::unmask(stm32::Interrupt::TIM16);
    }
    for adc_load in [false, true] {
        let mut minimum = u16::MAX;
        let mut maximum = 0;
        let mut failures = 0;
        let before = IRQ_COUNT.load(Relaxed);
        for _ in 0..32 {
            let count = IRQ_COUNT.load(Relaxed);
            let start = t17();
            Timer.set_and_enable(399);
            while IRQ_COUNT.load(Relaxed) == count {
                if t17().wrapping_sub(start) > 2000 {
                    failures += 1;
                    break;
                }
                if adc_load {
                    core::hint::black_box(unsafe { adc_read(6) });
                }
            }
            if IRQ_COUNT.load(Relaxed) != count {
                let elapsed = (IRQ_TIME.load(Relaxed) as u16).wrapping_sub(start);
                minimum = minimum.min(elapsed);
                maximum = maximum.max(elapsed);
            }
            Timer.disable_interrupt();
        }
        let _ = writeln!(
            out,
            "COMIRQ adc={} n=32 irq={} min_us={} max_us={} timeouts={}",
            adc_load as u8,
            IRQ_COUNT.load(Relaxed) - before,
            minimum,
            maximum,
            failures
        );
    }
    Timer::stop();
    gates_off();
    set_pin(3, 1, false);
}
