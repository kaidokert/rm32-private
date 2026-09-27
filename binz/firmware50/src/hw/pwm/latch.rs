//! Software-triggered TIM1 role latch. No timer rephasing or new control law.
use super::{Plan, regs};

/// Caller owns a masked, stop-checked COM transaction (or disabled preflight).
/// RM0444: CCPC shadows OCxM/CCxE/NE; COMG transfers those bits together.
/// OCxPE is not shadowed: enable it on all channels BEFORE writing compares.
/// UDIS prevents any partial CCR set reaching active shadows at an overflow.
/// The complete set transfers on the next native update, without UG/CNT.
#[inline(always)]
pub fn apply(plan: &Plan) {
    let t = regs();
    let cr1 = t.cr1().read().bits();
    let cr2 = t.cr2().read().bits();
    // SAFETY: preserve configured bits, only UDIS/CCPC/CCUS change temporarily.
    t.cr1().write(|w| unsafe { w.bits(cr1 | 2) });
    // SAFETY: CCPC/CCUS are the only changed CR2 fields.
    t.cr2().write(|w| unsafe { w.bits((cr2 | 1) & !4) });
    // SAFETY: checked sixstep images, plus compare preload for every channel.
    t.ccmr1_output().write(|w| unsafe { w.bits(plan.ccmr1 | 0x0808) });
    // SAFETY: CH3 checked role image and its preload enable.
    t.ccmr2_output().write(|w| unsafe { w.bits(plan.ccmr2 | 0x08) });
    t.ccr1().write(|w| w.ccr().set(plan.ccr[0] as u16));
    t.ccr2().write(|w| w.ccr().set(plan.ccr[1] as u16));
    t.ccr3().write(|w| w.ccr().set(plan.ccr[2] as u16));
    // SAFETY: same role enables as the immediate path; no polarity changes.
    t.ccer().write(|w| unsafe { w.bits(plan.ccer) });
    t.egr().write(|w| w.comg().set_bit());
    // SAFETY: restore saved configuration, never MOE, UG or CNT.
    t.cr2().write(|w| unsafe { w.bits(cr2) });
    // SAFETY: exact saved CR1; caller excludes competing configuration writers.
    t.cr1().write(|w| unsafe { w.bits(cr1) });
}
