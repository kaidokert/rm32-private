//! Disabled NVIC-only nesting probe; no timer events or output authority.
use super::*;
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static SEQ: AtomicU32 = AtomicU32::new(0);
fn mark(n: u32) {
    SEQ.store(SEQ.load(Relaxed) * 10 + n, Relaxed);
}
pub fn comp() -> bool {
    if !ACTIVE.load(Relaxed) {
        return false;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM2);
    mark(2);
    true
}
pub fn com() -> bool {
    if !ACTIVE.load(Relaxed) {
        return false;
    }
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM16);
    mark(1);
    cortex_m::peripheral::NVIC::pend(stm32::Interrupt::TIM2);
    let start = t17();
    while t17().wrapping_sub(start) < 20 {}
    mark(3);
    true
}
pub fn check<W: Write>(out: &mut W) {
    use cortex_m::peripheral::NVIC;
    use stm32::Interrupt::{TIM2, TIM16};
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || powered_timer::owns()
        || core_bench::active()
        || driven_run::owns()
    {
        let _ = writeln!(out, "PRIORITYCHECK refused=1");
        return;
    }
    comp_input::stop();
    com_timer::Timer::stop();
    unsafe {
        if (*stm32::TIM2::ptr()).dier().read().bits() != 0
            || (*stm32::TIM16::ptr()).dier().read().bits() != 0
        {
            let _ = writeln!(out, "PRIORITYCHECK refused=2");
            return;
        }
    }
    let old = [NVIC::get_priority(TIM2), NVIC::get_priority(TIM16)];
    let enabled = [NVIC::is_enabled(TIM2), NVIC::is_enabled(TIM16)];
    for (priority, expected) in [(0x80, 123), (0x40, 132)] {
        cortex_m::interrupt::free(|_| unsafe {
            NVIC::mask(TIM2);
            NVIC::mask(TIM16);
            NVIC::unpend(TIM2);
            NVIC::unpend(TIM16);
            let mut n = cortex_m::Peripherals::steal().NVIC;
            n.set_priority(TIM2, 0x40);
            n.set_priority(TIM16, priority);
            SEQ.store(0, Relaxed);
            ACTIVE.store(true, Relaxed);
            NVIC::unmask(TIM2);
            NVIC::unmask(TIM16);
            NVIC::pend(TIM16);
        });
        let start = t17();
        while SEQ.load(Relaxed) < 100 && t17().wrapping_sub(start) < 1000 {}
        cortex_m::interrupt::free(|_| {
            NVIC::mask(TIM2);
            NVIC::mask(TIM16);
            ACTIVE.store(false, Relaxed);
            NVIC::unpend(TIM2);
            NVIC::unpend(TIM16);
        });
        let _ = writeln!(
            out,
            "PRIORITYCASE comp={} com={} sequence={} expected={} disabled={}",
            NVIC::get_priority(TIM2),
            NVIC::get_priority(TIM16),
            SEQ.load(Relaxed),
            expected,
            (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
        );
    }
    unsafe {
        let mut n = cortex_m::Peripherals::steal().NVIC;
        n.set_priority(TIM2, old[0]);
        n.set_priority(TIM16, old[1]);
        if enabled[0] {
            NVIC::unmask(TIM2);
        }
        if enabled[1] {
            NVIC::unmask(TIM16);
        }
    }
    let restored = NVIC::get_priority(TIM2) == old[0]
        && NVIC::get_priority(TIM16) == old[1]
        && NVIC::is_enabled(TIM2) == enabled[0]
        && NVIC::is_enabled(TIM16) == enabled[1];
    let _ = writeln!(
        out,
        "PRIORITYCHECK restored={} gate_authority=0",
        restored as u8
    );
}
