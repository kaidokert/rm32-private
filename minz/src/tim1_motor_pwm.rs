//! TIM1 triple complementary PWM — register setup matched to AM32 / rm32 L431.

use crate::hal::rcc::{APB2, Enable, Reset};
use crate::hal::stm32::{GPIOA, GPIOB, TIM1};
use crate::{TIM1_AUTORELOAD, TIM1_CCR4_TRGO, TIM1_DEAD_TIME};

/// Configure TIM1: 24 kHz PWM, CH1–3 + CH1N–3N, CH4 TRGO, dead-time, MOE.
pub fn init(tim1: TIM1, apb2: &mut APB2) {
    TIM1::enable(apb2);
    TIM1::reset(apb2);

    unsafe {
        let tim1 = &*TIM1::ptr();
        tim1.psc.write(|w| w.psc().bits(0));
        tim1.arr.write(|w| w.arr().bits(TIM1_AUTORELOAD));
        tim1.ccmr1_output().write(|w| {
            w.oc1m()
                .bits(0b110)
                .oc1pe()
                .set_bit()
                .oc2m()
                .bits(0b110)
                .oc2pe()
                .set_bit()
        });
        tim1.ccmr2_output().write(|w| {
            w.oc3m()
                .bits(0b110)
                .oc3pe()
                .set_bit()
                .oc4m()
                .bits(0b110)
                .oc4pe()
                .set_bit()
        });
        tim1.ccer.write(|w| {
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
                .cc4e()
                .set_bit()
        });
        tim1.ccr4.write(|w| w.ccr().bits(TIM1_CCR4_TRGO));
        tim1.bdtr
            .write(|w| w.moe().set_bit().bkp().set_bit().dtg().bits(TIM1_DEAD_TIME));
        // CR2.MMS = 010 → TRGO fires on TIM1's update event (once per
        // PWM period). Used by TIM15 in slave-reset mode to align its
        // blanking-pulse generator with the PWM cycle wrap.
        tim1.cr2.write(|w| w.mms().bits(0b010));
        tim1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());
    }

    let _ = tim1;
}

/// Set each of the 6 PWM pins to either `ALTERNATE` (TIM1-driven) or
/// general-purpose `OUTPUT`, and drive the OUTPUT pins low via BSRR.
/// This is how AM32 / rm32 firmware "float" a phase — the floating
/// phase's pins go to OUTPUT-LOW so the gate driver IC sees a clean
/// 0 V on both H and L inputs and holds both FETs OFF. Leaving the
/// pins in ALTERNATE mode with `CCxE=0` (`BDTR.OSSR=0` releases them
/// to Hi-Z) is **not** sufficient: the gate driver's input then
/// floats and the FETs end up partially conducting under capacitive
/// pickup from neighboring PWM traces.
///
/// Per AM32 `phaseAFLOAT` (`AM32/Mcu/l431/Src/phaseouts.c:150-158`):
/// MODER goes to OUTPUT first, then the pin is reset low via the
/// BR half of BSRR. TIM1's `CCER` bits stay enabled throughout (the
/// channel keeps toggling internally, just isolated from the pad).
fn set_phase_pin_modes(float_a: bool, float_b: bool, float_c: bool) {
    const AF: u32 = 0b10;
    const OUT: u32 = 0b01;
    let gpioa = unsafe { &*GPIOA::ptr() };
    let gpiob = unsafe { &*GPIOB::ptr() };

    // GPIOA: PA7 (A low), PA8 (A high), PA9 (B high), PA10 (C high).
    let m_al = if float_a { OUT } else { AF };
    let m_ah = if float_a { OUT } else { AF };
    let m_bh = if float_b { OUT } else { AF };
    let m_ch = if float_c { OUT } else { AF };
    let a_mask = (0b11u32 << 14) | (0b11 << 16) | (0b11 << 18) | (0b11 << 20);
    let a_val = (m_al << 14) | (m_ah << 16) | (m_bh << 18) | (m_ch << 20);

    // GPIOB: PB0 (B low), PB1 (C low).
    let m_bl = if float_b { OUT } else { AF };
    let m_cl = if float_c { OUT } else { AF };
    let b_mask = (0b11u32 << 0) | (0b11 << 2);
    let b_val = (m_bl << 0) | (m_cl << 2);

    // MODER first (per AM32 order), then BSRR.BR to drive ODR=0 for
    // the floating phase's pins. BSRR.BR[N] sits at bit (16 + N).
    gpioa
        .moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !a_mask) | a_val) });
    gpiob
        .moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !b_mask) | b_val) });

    let mut bsrr_a = 0u32;
    let mut bsrr_b = 0u32;
    if float_a {
        bsrr_a |= (1 << (16 + 7)) | (1 << (16 + 8));
    }
    if float_b {
        bsrr_a |= 1 << (16 + 9);
        bsrr_b |= 1 << (16 + 0);
    }
    if float_c {
        bsrr_a |= 1 << (16 + 10);
        bsrr_b |= 1 << (16 + 1);
    }
    if bsrr_a != 0 {
        gpioa.bsrr.write(|w| unsafe { w.bits(bsrr_a) });
    }
    if bsrr_b != 0 {
        gpiob.bsrr.write(|w| unsafe { w.bits(bsrr_b) });
    }
}

/// Phase A/B/C duty (CCR1/2/3) for continuous 3-phase drive (sine).
/// All six PWM pins are restored to ALTERNATE so TIM1 drives them;
/// `CCER` stays at its init value (all channels enabled).
#[inline]
pub fn set_duties(ch1: u16, ch2: u16, ch3: u16) {
    cortex_m::interrupt::free(|_| {
        let tim1 = unsafe { &*TIM1::ptr() };
        tim1.ccr1.write(|w| w.ccr().bits(ch1));
        tim1.ccr2.write(|w| w.ccr().bits(ch2));
        tim1.ccr3.write(|w| w.ccr().bits(ch3));
        set_phase_pin_modes(false, false, false);
    });
}

/// Six-step BLDC commutation: 2 phases driven, 1 floating per sector.
/// `step ∈ 0..5` picks the (Hi, Lo, Float) mapping below. `duty` is
/// the PWM compare value applied to the **high-side** channel; the
/// **low-side** phase gets `CCR=0` (its complementary FET stays on
/// solid → terminal at GND); the **floating** phase has its GPIO pins
/// switched to OUTPUT and driven low (per AM32's behaviour — see
/// [`set_phase_pin_modes`] for why this differs from "let `CCxE=0`
/// release the pad to Hi-Z").
///
/// CCER never changes after `init()` — it stays at all-enabled.
/// TIM1's channels keep toggling internally for the floating phase;
/// the MODER override is what isolates them from the pad.
///
/// | Step | Hi | Lo | Float |
/// |------|----|----|-------|
/// | 0    | A  | B  | C     |
/// | 1    | A  | C  | B     |
/// | 2    | B  | C  | A     |
/// | 3    | B  | A  | C     |
/// | 4    | C  | A  | B     |
/// | 5    | C  | B  | A     |
///
/// Phase A/B/C = TIM1 CH1/2/3 (PA8/PA9/PA10 high, PA7/PB0/PB1 low).
#[inline]
pub fn set_six_step(step: u8, duty: u16) {
    const HIGH: [u8; 6] = [0, 0, 1, 1, 2, 2];
    const LOW: [u8; 6] = [1, 2, 2, 0, 0, 1];
    let s = (step % 6) as usize;
    let hi = HIGH[s] as usize;
    let lo = LOW[s] as usize;

    let mut ccrs = [0u16; 3];
    ccrs[hi] = duty;

    let float_a = hi != 0 && lo != 0;
    let float_b = hi != 1 && lo != 1;
    let float_c = hi != 2 && lo != 2;

    cortex_m::interrupt::free(|_| {
        let tim1 = unsafe { &*TIM1::ptr() };
        tim1.ccr1.write(|w| w.ccr().bits(ccrs[0]));
        tim1.ccr2.write(|w| w.ccr().bits(ccrs[1]));
        tim1.ccr3.write(|w| w.ccr().bits(ccrs[2]));
        set_phase_pin_modes(float_a, float_b, float_c);
    });
}

/// Hard kill: force all six motor-control pins to GPIO OUTPUT-LOW.
/// Each gate-driver's HIN and LIN see a commanded `0` → top FET off,
/// bottom FET off, motor terminal high-Z. This reaches the same
/// "no FETs conducting" outcome the old MOE-clear path did, but by
/// **actively commanding** the gate-driver inputs rather than
/// releasing the pads to Hi-Z. Per the bench rule: the MCU always
/// drives the gate-driver inputs, never leaves them floating.
///
/// MOE stays set (the init value); the TIM1 output stage is never
/// shut down. Pin direction (AF vs OUTPUT) is what selects whether
/// TIM1 or this command holds the line — same mechanism
/// [`set_six_step`] already uses for the per-sector float phase.
#[inline]
pub fn all_off() {
    set_phase_pin_modes(true, true, true);
}

/// Re-arm after [`all_off`]: restore all six pins to ALTERNATE
/// function so TIM1 drives them per the current CCR / CCER state.
/// The next TIM7 commutation tick (within ≤167 µs at 6 kHz) calls
/// [`set_six_step`] which re-sets the correct AF/OUTPUT split for
/// the active sector. MOE was never cleared — see [`all_off`].
#[inline]
pub fn arm_output() {
    set_phase_pin_modes(false, false, false);
}

/// Enable the TIM1 update-event interrupt (DIER.UIE). Fires the
/// `TIM1_UP_TIM16` IRQ once per PWM period (24 kHz @ ARR=3332). The
/// IRQ vector is shared with TIM16, but TIM16 isn't initialised on
/// this bench — so the handler can assume every fire is TIM1.UIF.
///
/// The ISR is responsible for acking `TIM1.SR.UIF`.
#[inline]
pub fn enable_update_interrupt() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.dier.modify(|_, w| w.uie().set_bit());
}

/// Ack the TIM1 update-event flag (`SR.UIF`). Call this at the top
/// of the `TIM1_UP_TIM16` ISR before doing anything else, so the
/// IRQ doesn't re-fire on return.
#[inline]
pub fn clear_update_flag() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.sr.modify(|_, w| w.uif().clear_bit());
}

#[inline]
pub const fn max_duty() -> u16 {
    TIM1_AUTORELOAD
}
