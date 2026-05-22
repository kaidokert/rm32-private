//! TIM15 blanking-pulse generator, slaved to TIM1.
//!
//! TIM15 is configured in slave-reset mode driven by TIM1's TRGO
//! (= TIM1 update event), so its counter wraps to 0 at every PWM
//! period boundary. CH1 runs in PWM-output mode 1 with `CCR1 = N`,
//! producing an OC1 pulse that's HIGH for the first `N` timer ticks
//! after each slave reset and LOW for the rest of the period.
//!
//! That OC1 signal is wired internally to COMP2's `BLANKING` input
//! (COMP2 CSR.BLANKING = `0b100`). While OC1 is HIGH, the
//! comparator's VALUE bit is held — every EXTI edge that would have
//! been triggered by PWM-coupled ringing during dead-time / FET
//! slew is swallowed by hardware.
//!
//! The pulse width `N` is live-tunable via [`set_blank_ticks`]. At
//! 80 MHz, 1 timer tick = 12.5 ns; pick `N` to cover the noisy window
//! (typically dead-time `≈ 562.5 ns` + slew `≈ 200-300 ns` → `N ≈ 64`).
//!
//! ## Why a raw-pointer SMCR write?
//!
//! `stm32l4`'s SVD for TIM15 doesn't expose the slave-mode-control
//! register (SMCR) at offset `0x08` — it's marked `_reserved2` in the
//! PAC's `RegisterBlock`. The PAC otherwise covers everything else
//! we need (CR1, CR2, CCMR1, CCER, CCR1, CNT, PSC, ARR, BDTR, DIER,
//! SR, EGR), so the workaround is exactly one volatile write to
//! `0x4001_4008`.

use crate::hal::rcc::{APB2, Enable, Reset};
use crate::hal::stm32::TIM15;

/// Absolute address of `TIM15_SMCR`. Base = `0x4001_4000` + offset `0x08`.
const TIM15_SMCR_ADDR: *mut u32 = 0x4001_4008 as *mut u32;

/// SMCR field encoding:
///   SMS\[2:0\] = 0b100 (Reset mode — counter reinitialised on TRGI)
///   TS\[6:4\]  = 0b000 (ITR0 = TIM1, per RM0394 Table 146)
///   All other bits = 0
const SMCR_RESET_MODE_TS_TIM1: u32 = 0b0000_0000_0000_0000_0000_0000_0000_0100;

/// Configure TIM15 to generate a slave-reset-aligned blanking pulse
/// on OC1, wired internally to COMP2 BLANKING. `initial_ticks` is
/// the starting pulse width in 12.5 ns timer ticks; `0` means
/// blanking effectively off (OC1 never goes HIGH).
///
/// Caller must have configured TIM1 to emit TRGO on its update event
/// (TIM1.CR2.MMS = `0b010`) — see `tim1_motor_pwm::init`.
///
/// `ARR` is set to `u16::MAX` so the natural counter overflow never
/// fires within a PWM period; the only thing that resets TIM15.CNT
/// is the slave-mode trigger from TIM1.
///
/// No interrupt is enabled — OC1 drives COMP2's blanking input
/// entirely in hardware once init returns.
pub fn init(_tim15: TIM15, apb2: &mut APB2, initial_ticks: u16) {
    TIM15::enable(apb2);
    TIM15::reset(apb2);

    unsafe {
        let tim15 = &*TIM15::ptr();
        tim15.psc.write(|w| w.psc().bits(0));
        tim15.arr.write(|w| w.arr().bits(u16::MAX));

        // SMCR: slave-reset mode, ITR0 = TIM1.
        core::ptr::write_volatile(TIM15_SMCR_ADDR, SMCR_RESET_MODE_TS_TIM1);

        // CH1 in PWM mode 1 with preload. OC1 active (= HIGH) while
        // CNT < CCR1, inactive otherwise. With CCR1 = N and ARR =
        // u16::MAX, OC1 is HIGH for the first N ticks after each
        // TIM1-driven reset, LOW for the rest of the PWM period.
        tim15
            .ccmr1_output()
            .write(|w| w.oc1m().bits(0b110).oc1pe().set_bit());
        tim15.ccr1.write(|w| w.ccr().bits(initial_ticks));

        // CC1E + main output enable so OC1 actually drives the
        // internal blanking line. TIM15 has BDTR.MOE just like TIM1
        // (it's an "advanced-lite" timer with complementary outputs
        // on CH1) — without MOE set, OC1 stays gated to 0 even with
        // CCMR1/CCER configured.
        //
        // Polarity: CC1P=0 (active-high). RM0394 19.3.7 says the
        // **complement** of the blanking signal is ANDed with the
        // comparator output, so OC1 HIGH = comparator gated. With
        // PWM mode 1 + CC1P=0, OC1 is HIGH during the first CCR1
        // ticks after slave-reset — exactly the post-PWM-edge
        // window we want to gate.
        tim15.ccer.write(|w| w.cc1e().set_bit());
        tim15.bdtr.write(|w| w.moe().set_bit());

        // UIE + CC1IE: prove TIM15 is being slave-reset by TIM1
        // (UIF every PWM cycle) AND that OC1's compare circuitry is
        // firing (CC1IF when CNT reaches CCR1). The `i` key reports
        // both separately so we can tell whether a missing blanking
        // effect is due to CC1 not firing (= OC1 stuck low) vs
        // something downstream.
        tim15.dier.write(|w| w.uie().set_bit().cc1ie().set_bit());
        tim15.cr1.write(|w| w.cen().set_bit());
    }
}

/// Ack TIM15's update + CC1 flags. Must be called at the top of the
/// `TIM1_BRK_TIM15` ISR or the IRQ re-fires immediately on return.
/// Returns `(uif_was_set, cc1if_was_set)` so the caller can count
/// each source separately.
#[inline]
pub fn clear_flags() -> (bool, bool) {
    let tim15 = unsafe { &*TIM15::ptr() };
    let sr = tim15.sr.read();
    let uif = sr.uif().bit_is_set();
    let cc1if = sr.cc1if().bit_is_set();
    tim15
        .sr
        .modify(|_, w| w.uif().clear_bit().cc1if().clear_bit());
    (uif, cc1if)
}

/// Read the current TIM15 CCR1 (= active blanking pulse width in
/// ticks). Useful for the `i` key diagnostic to confirm the write
/// from [`set_blank_ticks`] actually landed.
#[inline]
pub fn ccr1() -> u16 {
    let tim15 = unsafe { &*TIM15::ptr() };
    tim15.ccr1.read().ccr().bits()
}

/// Live-set the blanking pulse width in 12.5 ns timer ticks. `0`
/// disables blanking (OC1 stays LOW); positive values keep OC1 HIGH
/// for the first `ticks` 12.5 ns intervals after each TIM1 update.
/// Single-register write, race-safe with the TIM15 hardware.
#[inline]
pub fn set_blank_ticks(ticks: u16) {
    let tim15 = unsafe { &*TIM15::ptr() };
    tim15.ccr1.write(|w| unsafe { w.ccr().bits(ticks) });
}
