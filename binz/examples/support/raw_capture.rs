//! TIM3 CH2 observer configuration; never owns counter, ADC trigger or DMA.
use super::*;
pub fn prepare() -> bool {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM3::ptr();
        if t.psc().read().bits() != 63
            || t.arr().read().bits() != 200
            || t.ccer().read().bits() & (1 << 4) != 0
        {
            return false;
        }
        let before = [
            t.cr1().read().bits(),
            t.cr2().read().bits(),
            t.smcr().read().bits(),
            t.dier().read().bits(),
        ];
        t.ccmr1_input()
            .modify(|r, w| w.bits((r.bits() & !0xff00) | (1 << 8)));
        t.tisel()
            .modify(|r, w| w.bits((r.bits() & !(15 << 8)) | (1 << 8)));
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        before
            == [
                t.cr1().read().bits(),
                t.cr2().read().bits(),
                t.smcr().read().bits(),
                t.dier().read().bits(),
            ]
    })
}
pub fn arm(rising: bool) {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM3::ptr();
        t.ccer().modify(|r, w| w.bits(r.bits() & !(1 << 4)));
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        t.ccer().modify(|r, w| {
            w.bits((r.bits() & !(15 << 4)) | (1 << 4) | if rising { 0 } else { 1 << 5 })
        });
    });
}
pub fn stop() {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM3::ptr();
        t.ccer().modify(|r, w| w.bits(r.bits() & !(1 << 4)));
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
    });
}
/// Snapshot in the same serialized observer epoch; age is modulo201us only.
pub fn snapshot() -> [u32; 5] {
    cortex_m::interrupt::free(|_| unsafe {
        let t = &*stm32::TIM3::ptr();
        t.ccer().modify(|r, w| w.bits(r.bits() & !(1 << 4)));
        let sr = t.sr().read().bits();
        let ccr = t.ccr2().read().bits();
        let now = t.cnt().read().bits();
        t.sr().write(|w| w.bits(!(1 << 2 | 1 << 10)));
        [1, (sr >> 2) & 1, (sr >> 10) & 1, ccr, now]
    })
}
fn delay(us: u16) {
    let start = t17();
    while t17().wrapping_sub(start) < us {}
}
/// Only the disabled coexistence fixture may generate comparator transitions.
pub fn disabled_pulse() -> bool {
    unsafe {
        if get_idr(3, 1) || !powered_timer::outputs_disabled() || core_bench::active() {
            return false;
        }
        let saved = core::ptr::read_volatile(COMP2_CSR);
        let base = (saved & !(15 << 4 | 1 << 15)) | (3 << 4) | 1;
        core::ptr::write_volatile(COMP2_CSR, base);
        delay(40);
        let low = base
            | if core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0 {
                1 << 15
            } else {
                0
            };
        core::ptr::write_volatile(COMP2_CSR, low);
        delay(4);
        arm(true);
        core::ptr::write_volatile(COMP2_CSR, low ^ (1 << 15));
        delay(2);
        let high = core::ptr::read_volatile(COMP2_CSR) & (1 << 30) != 0;
        core::ptr::write_volatile(COMP2_CSR, low);
        delay(2);
        let t = &*stm32::TIM3::ptr();
        let sr = t.sr().read().bits();
        let ccr = t.ccr2().read().bits();
        stop();
        core::ptr::write_volatile(COMP2_CSR, saved);
        high && sr & (1 << 2) != 0
            && sr & (1 << 10) == 0
            && ccr <= 200
            && !get_idr(3, 1)
            && powered_timer::outputs_disabled()
    }
}
