//! TIM1: the six-output complementary PWM that drives the DRV8304 inputs
//! (step 2b of the `hw/` move, notebook E112).
//!
//! **Why not the HAL** (moved from the binary): its BDTR access is limited to
//! MOE, so DTG/OSSR/OSSI are unreachable (no dead time on a 6x bridge is a
//! shoot-through generator); `PwmPin::enable()` sets MOE in the same call that
//! enables a channel and offers no way to clear it, which the preflight and
//! safe-off design require; and `Pwm::set_freq` writes CR1 wholesale,
//! clobbering ARPE.
//!
//! **The one whole-register write in this crate** is [`apply_plan`]: CCMR1,
//! CCMR2 and CCER take the register images `sixstep::plan` computes and
//! host-tests (geometry, float invariant, equal compares). It runs in the COM
//! root on every commutation; decomposing the images into per-field writes at
//! run time would add work to that root, whose cycle count may not grow.

use stm32g0xx_hal::rcc::{Enable, Rcc, Reset};
use stm32g0xx_hal::stm32;

use crate::sixstep::Plan;

#[inline(always)]
fn regs() -> &'static stm32::tim1::RegisterBlock {
    // SAFETY: TIM1's block from the PAC's pointer constant. Writers are the
    // foreground (setup, carrier, sine compares, preflight) and, while the
    // closed loop runs, only the COM root (plans) and the guard root (MOE off
    // on a trip) -- the same single-writer discipline as before the move.
    unsafe { &*stm32::TIM1::ptr() }
}

/// Configure 6x complementary PWM at `period` ticks with the master output
/// **disabled**. The peripheral is reset first: every later step is a
/// `modify`, and "MOE clear => all six gates driven low" depends on the reset
/// values of CCER polarity and BDTR `OISx`.
pub fn init(rcc: &mut Rcc, period: u32, dead_time: u8) {
    stm32::TIM1::enable(rcc);
    stm32::TIM1::reset(rcc);
    let t = regs();
    t.cr1().reset();
    t.psc().write(|w| w.psc().set(0));
    t.arr().write(|w| w.arr().set((period - 1) as u16));
    t.ccr1().write(|w| w.ccr().set(0));
    t.ccr2().write(|w| w.ccr().set(0));
    t.ccr3().write(|w| w.ccr().set(0));
    // PWM mode 1 with output-compare preload on channels 1-3.
    t.ccmr1_output().modify(|_, w| {
        w.oc1m()
            .pwm_mode1()
            .oc1pe()
            .set_bit()
            .oc2m()
            .pwm_mode1()
            .oc2pe()
            .set_bit()
    });
    t.ccmr2_output().modify(|_, w| w.oc3m().pwm_mode1().oc3pe().set_bit());
    // Both the output and its complement on all three channels (the line the
    // HAL's PWM macro gets wrong).
    t.ccer().modify(|_, w| {
        w.cc1e()
            .set_bit()
            .cc1ne()
            .set_bit()
            .cc2e()
            .set_bit()
            .cc2ne()
            .set_bit()
            .cc3e()
            .set_bit()
            .cc3ne()
            .set_bit()
    });
    // OSSI/OSSR keep the outputs *driven* to their idle level when MOE is
    // clear. MOE is deliberately NOT set here.
    t.bdtr()
        .modify(|_, w| w.dtg().set(dead_time).ossr().set_bit().ossi().set_bit());
    t.egr().write(|w| w.ug().set_bit());
    t.cr1().modify(|_, w| w.arpe().set_bit().cen().set_bit());
}

/// Apply one six-step sector: modes and compares (preloaded), then CCER
/// (immediate), so a channel is never enabled while still carrying the
/// previous sector's role.
#[inline(always)]
pub fn apply_plan(plan: &Plan) {
    let t = regs();
    // The images come from `sixstep::plan`, whose host tests pin every bit
    // (modes, preload, enables, no polarity inversion); see module docs.
    t.ccmr1_output().write(|w| {
        // SAFETY: a `sixstep::plan` CCMR1 image (above).
        unsafe { w.bits(plan.ccmr1) }
    });
    t.ccmr2_output().write(|w| {
        // SAFETY: a `sixstep::plan` CCMR2 image (above).
        unsafe { w.bits(plan.ccmr2) }
    });
    t.ccr1().write(|w| w.ccr().set(plan.ccr[0] as u16));
    t.ccr2().write(|w| w.ccr().set(plan.ccr[1] as u16));
    t.ccr3().write(|w| w.ccr().set(plan.ccr[2] as u16));
    // SAFETY: as above.
    t.ccer().write(|w| unsafe { w.bits(plan.ccer) });
}

/// Program the period and load it now (ARPE is set, so without UG it would
/// wait for the next update).
pub fn set_period(period: u32) {
    let t = regs();
    t.arr().write(|w| w.arr().set((period - 1) as u16));
    t.egr().write(|w| w.ug().set_bit());
}

/// Write one channel's compare (1..=3; anything else is ignored).
#[inline(always)]
pub fn set_channel_compare(ch: u8, v: u16) {
    let t = regs();
    // Statements, not match arms: each write returns its register's own type.
    if ch == 1 {
        t.ccr1().write(|w| w.ccr().set(v));
    } else if ch == 2 {
        t.ccr2().write(|w| w.ccr().set(v));
    } else if ch == 3 {
        t.ccr3().write(|w| w.ccr().set(v));
    }
}

/// Write the three phase compares. Phase A -> CCR3, B -> CCR2, C -> CCR1.
#[inline(always)]
pub fn set_phase_compares(c: [u32; 3]) {
    let t = regs();
    t.ccr3().write(|w| w.ccr().set(c[0] as u16));
    t.ccr2().write(|w| w.ccr().set(c[1] as u16));
    t.ccr1().write(|w| w.ccr().set(c[2] as u16));
}

/// The three channel compares, CCR1..CCR3.
#[must_use]
pub fn compares() -> (u32, u32, u32) {
    let t = regs();
    (
        t.ccr1().read().ccr().bits() as u32,
        t.ccr2().read().ccr().bits() as u32,
        t.ccr3().read().ccr().bits() as u32,
    )
}

/// Float the bridge without touching MOE: every output disabled (driven to
/// its inactive level by OSSR), compares zero.
pub fn float_all() {
    let t = regs();
    t.ccer().reset();
    t.ccr1().write(|w| w.ccr().set(0));
    t.ccr2().write(|w| w.ccr().set(0));
    t.ccr3().write(|w| w.ccr().set(0));
}

/// All three channels back to PWM mode 1 with preload, both outputs enabled
/// (the sine drive chops every phase; a six-step plan leaves them otherwise).
pub fn all_phases_pwm() {
    let t = regs();
    t.ccmr1_output().write(|w| {
        w.oc1m()
            .pwm_mode1()
            .oc1pe()
            .set_bit()
            .oc2m()
            .pwm_mode1()
            .oc2pe()
            .set_bit()
    });
    t.ccmr2_output().write(|w| w.oc3m().pwm_mode1().oc3pe().set_bit());
    t.ccer().write(|w| {
        w.cc1e()
            .set_bit()
            .cc1ne()
            .set_bit()
            .cc2e()
            .set_bit()
            .cc2ne()
            .set_bit()
            .cc3e()
            .set_bit()
            .cc3ne()
            .set_bit()
    });
}

/// Master output enable.
#[inline(always)]
pub fn moe_on() {
    regs().bdtr().modify(|_, w| w.moe().set_bit());
}

/// Master output disable: with OSSI set, all six gate pins go to their idle
/// low in hardware.
#[inline(always)]
pub fn moe_off() {
    regs().bdtr().modify(|_, w| w.moe().clear_bit());
}

#[inline(always)]
#[must_use]
pub fn moe_is_set() -> bool {
    regs().bdtr().read().moe().bit_is_set()
}

/// The PWM counter (the position inside the carrier period).
#[inline(always)]
#[must_use]
pub fn counter() -> u32 {
    regs().cnt().read().cnt().bits() as u32
}
