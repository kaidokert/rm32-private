//! Driver-disabled TIM16 deadline/preload probe. No pin AF or IRQ is enabled.
use super::*;

pub fn run<W: Write>(out: &mut W) {
    gates_off();
    set_pin(3, 1, false);
    let tim = unsafe { &*stm32::TIM16::ptr() };
    unsafe {
        let rcc = &*stm32::RCC::ptr();
        rcc.apbenr2().modify(|r, w| w.bits(r.bits() | (1 << 17)));
        tim.dier().write(|w| w.bits(0));
        tim.ccer().write(|w| w.bits(0));
        tim.bdtr().write(|w| w.bits(0));
    }
    // Seed active ARR=1999 (1000 us), then request ARR=399 (200 us).
    // 0: immediate ARR; 1: reference-style preload without UG;
    // 2: explicit preload transfer. Tim17 brackets elapsed wall time.
    for mode in 0..3 {
        let mut minimum = u32::MAX;
        let mut maximum = 0;
        let mut failures = 0;
        for _ in 0..16 {
            unsafe {
                tim.cr1().write(|w| w.bits(0));
                tim.psc().write(|w| w.bits(31));
                tim.arr().write(|w| w.bits(1999));
                tim.egr().write(|w| w.bits(1));
                tim.sr().write(|w| w.bits(0));
                tim.cr1().write(|w| w.bits(if mode == 0 { 1 } else { 129 }));
            }
            let start = t17();
            unsafe {
                tim.cnt().write(|w| w.bits(0));
                tim.arr().write(|w| w.bits(399));
                if mode == 2 {
                    tim.egr().write(|w| w.bits(1));
                }
                tim.sr().write(|w| w.bits(0));
            }
            while tim.sr().read().bits() & 1 == 0 {
                if t17().wrapping_sub(start) > 2000 {
                    failures += 1;
                    break;
                }
            }
            let elapsed = t17().wrapping_sub(start) as u32;
            minimum = minimum.min(elapsed);
            maximum = maximum.max(elapsed);
            unsafe {
                tim.cr1().write(|w| w.bits(0));
            }
        }
        let _ = writeln!(
            out,
            "COMTIMING mode={} n=16 old_arr=1999 new_arr=399 psc=31 min_us={} max_us={} timeouts={}",
            mode, minimum, maximum, failures
        );
    }
    unsafe {
        tim.cr1().write(|w| w.bits(0));
        tim.dier().write(|w| w.bits(0));
        tim.sr().write(|w| w.bits(0));
    }
    gates_off();
    set_pin(3, 1, false);
}
