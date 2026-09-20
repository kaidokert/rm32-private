//! Real TIM1 preload adapter. Only disabled-driver check calls it so far.
//! Future powered caller must own the same guard/write critical section.
use super::*;
#[cfg(feature = "bench-live-duty-check")]
const REQUESTS: [live_duty::Prepared; 4] = [
    live_duty::Prepared::new(phase_role_live::CARRIER.ticks(), 40).unwrap(),
    live_duty::Prepared::new(phase_role_live::CARRIER.ticks(), 80).unwrap(),
    live_duty::Prepared::new(phase_role_live::CARRIER.ticks(), duty_envelope::MAX).unwrap(),
    live_duty::Prepared::new(phase_role_live::CARRIER.ticks(), 70).unwrap(),
];
struct Hardware {
    cr1: u32,
}
impl live_duty::Registers for Hardware {
    fn ready(&self, ticks: u32) -> bool {
        unsafe {
            let t = &*stm32::TIM1::ptr();
            t.cr1().read().bits() & 0xff == 0x81
                && t.arr().read().bits() + 1 == ticks
                && t.psc().read().bits() == 0
                && t.rcr().read().bits() == 0
                && t.smcr().read().bits() & 0x10007 == 0
                && t.dier().read().bits() == 0
                && t.ccmr1_output().read().bits() == 0x6868
                && t.ccmr2_output().read().bits() == 0x68
                && t.ccer().read().bits() == 0x555
        }
    }
    fn suppress_updates(&mut self) {
        unsafe {
            let t = &*stm32::TIM1::ptr();
            self.cr1 = t.cr1().read().bits();
            t.cr1().write(|w| w.bits(self.cr1 | 2));
        }
    }
    fn compare(&mut self, ch: usize, value: u32) {
        unsafe {
            let t = &*stm32::TIM1::ptr();
            match ch {
                0 => t.ccr1().write(|w| w.bits(value)),
                1 => t.ccr2().write(|w| w.bits(value)),
                2 => t.ccr3().write(|w| w.bits(value)),
                _ => unreachable!(),
            };
        }
    }
    fn resume_updates(&mut self) {
        unsafe {
            (*stm32::TIM1::ptr()).cr1().write(|w| w.bits(self.cr1));
        }
    }
}
/// Caller provides exclusive access; this function grants no gate authority.
pub(super) fn update(request: live_duty::Prepared) -> bool {
    live_duty::apply(&mut Hardware { cr1: 0 }, request)
}
#[cfg(feature = "bench-live-duty-check")]
fn roles() -> [u32; 5] {
    unsafe {
        [
            (*stm32::GPIOA::ptr()).moder().read().bits(),
            (*stm32::GPIOB::ptr()).moder().read().bits(),
            (*stm32::GPIOA::ptr()).odr().read().bits() & phase_gpio_plan::A_GATES,
            (*stm32::GPIOB::ptr()).odr().read().bits() & phase_gpio_plan::B_GATES,
            // VALUE bit30 is the live signal, not comparator configuration.
            core::ptr::read_volatile(COMP2_CSR) & !(1 << 30),
        ]
    }
}
#[cfg(feature = "bench-live-duty-check")]
pub fn check<W: Write>(out: &mut W) {
    if get_idr(3, 1)
        || powered_timer::owns()
        || core_bench::active()
        || !powered_timer::outputs_disabled()
    {
        let _ = writeln!(out, "LIVEDUTYCHECK refused=1");
        return;
    }
    prepare_sine();
    phase_role_live::prepare_carrier();
    let carrier = phase_role_live::CARRIER;
    let mut passed = 0;
    let mut maximum = 0;
    for step in 1..=6 {
        cortex_m::interrupt::free(|_| phase_role_live::apply(step, 70));
        for request in REQUESTS {
            let result = cortex_m::interrupt::free(|_| unsafe {
                if get_idr(3, 1) {
                    return false;
                }
                let before_roles = roles();
                let t = &*stm32::TIM1::ptr();
                let before = t.cnt().read().bits();
                let start = t17();
                let ok = update(request);
                let elapsed = t17().wrapping_sub(start) as u32;
                let after = t.cnt().read().bits();
                maximum = maximum.max(elapsed);
                let delta = (after + carrier.ticks() - before) % carrier.ticks();
                ok && carrier.continuous(elapsed, delta)
                    && before_roles == roles()
                    && t.cr1().read().bits() & 0xff == 0x81
                    && t.ccr1().read().bits() == request.compare()
                    && t.ccr2().read().bits() == request.compare()
                    && t.ccr3().read().bits() == request.compare()
                    && !get_idr(3, 1)
            });
            if !result {
                break;
            }
            passed += 1;
        }
        if passed != step as u32 * 4 {
            break;
        }
    }
    gates_off();
    set_pin(3, 1, false);
    let _ = writeln!(
        out,
        "LIVEDUTYCHECK passed={} expected=24 max_us={} disabled={} shadow_readback=0 motor_authority=0",
        passed,
        maximum,
        (!get_idr(3, 1) && powered_timer::outputs_disabled()) as u8
    );
}
