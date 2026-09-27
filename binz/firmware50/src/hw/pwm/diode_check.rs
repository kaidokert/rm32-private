//! Boot-only sampled PWM witness; external DRV ENABLE remains low throughout.
use super::{latch, latch_check::pads};
use crate::{commutation::Step, hw::gpio, sixstep};

/// Check each diode sector at the actual carrier with the power bridge disabled.
/// Sampling is not an oscilloscope or a proof of absence of short glitches.
pub fn run() -> bool {
    if gpio::enable_is_high() || super::moe_is_set() || super::compares() != (0, 0, 0) {
        return false;
    }
    super::set_period(1333);
    gpio::gates_to_timer();
    let mut ok = true;
    for step in 1..=6 {
        let Some(plan) = sixstep::plan(Step::new_clamped(step), 250, 1333, 800) else {
            ok = false;
            break;
        };
        let plan = plan.diode();
        latch::apply(&plan);
        super::moe_on();
        cortex_m::asm::delay(4096); // let the complete compare set reach a native UEV
        let source = 1u8 << ((2 - plan.source) * 2);
        let sink = 2u8 << ((2 - plan.sink) * 2);
        let mut high = false;
        let mut low = false;
        for _ in 0..1024 {
            let p = pads();
            if p != sink && p != (sink | source) { ok = false; }
            high |= p == (sink | source);
            low |= p == sink;
        }
        ok &= high && low && !gpio::enable_is_high();
    }
    super::moe_off();
    super::set_phase_compares([0; 3]);
    super::float_all();
    gpio::gates_low_now();
    gpio::enable_set(false);
    super::set_period(crate::duty::STARTUP_TICKS);
    ok && pads() == 0 && super::compares() == (0, 0, 0) && !super::moe_is_set()
}
