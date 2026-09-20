//! Disabled software-pending test of the actual ADC_COMP and DMA vectors.
use super::*;
use cortex_m::peripheral::NVIC;
use portable_atomic::{AtomicU32, Ordering::Relaxed};
use stm32::Interrupt::{ADC_COMP, DMA1_CHANNEL1};
static MODE: AtomicU32 = AtomicU32::new(0);
static SEQ: AtomicU32 = AtomicU32::new(0);
fn mark(n: u32) {
    SEQ.store(SEQ.load(Relaxed) * 10 + n, Relaxed);
}
pub fn comp() -> bool {
    let mode = MODE.load(Relaxed);
    if mode == 0 {
        return false;
    }
    NVIC::mask(ADC_COMP);
    mark(1);
    if mode == 1 {
        NVIC::pend(DMA1_CHANNEL1);
    }
    let start = t17();
    while t17().wrapping_sub(start) < 20 {}
    mark(3);
    true
}
pub fn dma() -> bool {
    if MODE.load(Relaxed) == 0 {
        return false;
    }
    NVIC::mask(DMA1_CHANNEL1);
    mark(2);
    true
}
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || !powered_timer::outputs_disabled()
        || powered_timer::owns()
        || core_bench::active()
        || driven_run::owns()
    {
        let _ = writeln!(out, "DMAPRIORITYCHECK refused=1");
        return;
    }
    // Refuse live producers, rather than clearing their events to fake a test.
    let idle = unsafe {
        (*stm32::DMA1::ptr()).ch1().cr().read().bits() & 1 == 0
            && (*stm32::ADC::ptr()).ier().read().bits() == 0
            && (*stm32::EXTI::ptr()).imr1().read().bits() & ((1 << 17) | (1 << 18)) == 0
            && (*stm32::TIM6::ptr()).dier().read().bits() == 0
    };
    if !idle || NVIC::is_pending(ADC_COMP) || NVIC::is_pending(DMA1_CHANNEL1) {
        let _ = writeln!(out, "DMAPRIORITYCHECK refused=2");
        return;
    }
    let old = [
        NVIC::get_priority(ADC_COMP),
        NVIC::get_priority(DMA1_CHANNEL1),
    ];
    let enabled = [NVIC::is_enabled(ADC_COMP), NVIC::is_enabled(DMA1_CHANNEL1)];
    for (mode, priority, expected) in [(1, 0, 123), (1, 64, 132), (2, 64, 213)] {
        cortex_m::interrupt::free(|_| unsafe {
            NVIC::mask(ADC_COMP);
            NVIC::mask(DMA1_CHANNEL1);
            let mut n = cortex_m::Peripherals::steal().NVIC;
            n.set_priority(ADC_COMP, 64);
            n.set_priority(DMA1_CHANNEL1, priority);
            SEQ.store(0, Relaxed);
            MODE.store(mode, Relaxed);
            NVIC::unmask(ADC_COMP);
            NVIC::unmask(DMA1_CHANNEL1);
            if mode == 2 {
                NVIC::pend(DMA1_CHANNEL1);
            }
            NVIC::pend(ADC_COMP);
        });
        let start = t17();
        while SEQ.load(Relaxed) < 100 && t17().wrapping_sub(start) < 1000 {}
        cortex_m::interrupt::free(|_| {
            NVIC::mask(ADC_COMP);
            NVIC::mask(DMA1_CHANNEL1);
            MODE.store(0, Relaxed);
            NVIC::unpend(ADC_COMP);
            NVIC::unpend(DMA1_CHANNEL1);
        });
        let _ = writeln!(
            out,
            "DMAPRIORITYCASE mode={} comp={} dma={} guard={} sequence={} expected={} disabled={}",
            mode,
            NVIC::get_priority(ADC_COMP),
            NVIC::get_priority(DMA1_CHANNEL1),
            NVIC::get_priority(stm32::Interrupt::TIM6_DAC_LPTIM1),
            SEQ.load(Relaxed),
            expected,
            (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
        );
    }
    cortex_m::interrupt::free(|_| unsafe {
        let mut n = cortex_m::Peripherals::steal().NVIC;
        n.set_priority(ADC_COMP, old[0]);
        n.set_priority(DMA1_CHANNEL1, old[1]);
        if enabled[0] {
            NVIC::unmask(ADC_COMP);
        }
        if enabled[1] {
            NVIC::unmask(DMA1_CHANNEL1);
        }
    });
    let restored = NVIC::get_priority(ADC_COMP) == old[0]
        && NVIC::get_priority(DMA1_CHANNEL1) == old[1]
        && NVIC::is_enabled(ADC_COMP) == enabled[0]
        && NVIC::is_enabled(DMA1_CHANNEL1) == enabled[1]
        && !NVIC::is_pending(ADC_COMP)
        && !NVIC::is_pending(DMA1_CHANNEL1);
    let _ = writeln!(
        out,
        "DMAPRIORITYCHECK restored={} gate_authority=0",
        restored as u8
    );
}
