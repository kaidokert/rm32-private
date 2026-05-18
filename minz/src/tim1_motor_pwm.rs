//! TIM1 triple complementary PWM — register setup matched to AM32 / rm32 L431.

use crate::hal::rcc::{APB2, Enable, Reset};
use crate::hal::stm32::TIM1;
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
        tim1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());
    }

    let _ = tim1;
}

/// Phase A/B/C duty (CCR1/2/3). Complementary outputs follow in hardware.
#[inline]
pub fn set_duties(ch1: u16, ch2: u16, ch3: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.ccr1.write(|w| w.ccr().bits(ch1));
    tim1.ccr2.write(|w| w.ccr().bits(ch2));
    tim1.ccr3.write(|w| w.ccr().bits(ch3));
}

/// Six-step BLDC commutation: 2 phases driven, 1 floating (Hi-Z) per sector.
///
/// `step ∈ 0..5` picks the (high, low, float) phase mapping. `duty` is
/// the PWM compare value applied to the **high-side** complementary
/// channel; the **low-side** phase gets CCR=0 (low FET held on solid
/// via the complementary output), and the **floating** phase has both
/// CCxE and CCxNE cleared — with `OSSR=0` in BDTR (set by `init()`),
/// the timer releases that pad and AF-mode Hi-Z gives a true float
/// window. CC4E (TRGO) is preserved.
///
/// Sector → (Hi, Lo, Float) mapping:
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
    // ccrs[lo] stays 0 → complementary low FET on solid.

    let cc1_on = hi == 0 || lo == 0;
    let cc2_on = hi == 1 || lo == 1;
    let cc3_on = hi == 2 || lo == 2;

    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.ccr1.write(|w| w.ccr().bits(ccrs[0]));
    tim1.ccr2.write(|w| w.ccr().bits(ccrs[1]));
    tim1.ccr3.write(|w| w.ccr().bits(ccrs[2]));
    tim1.ccer.write(|w| {
        w.cc1e()
            .bit(cc1_on)
            .cc1ne()
            .bit(cc1_on)
            .cc2e()
            .bit(cc2_on)
            .cc2ne()
            .bit(cc2_on)
            .cc3e()
            .bit(cc3_on)
            .cc3ne()
            .bit(cc3_on)
            .cc4e()
            .set_bit()
    });
}

/// Re-enable CCxE + CCxNE for all three motor channels (and CC4E
/// TRGO). Call this when switching back from `set_six_step` to
/// continuous 3-phase drive (`set_duties`) so the previously floating
/// phase isn't left Hi-Z mid-sine.
#[inline]
pub fn enable_all_phases() {
    let tim1 = unsafe { &*TIM1::ptr() };
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
}

/// Hard kill: clear `MOE` in BDTR. All TIM1 outputs immediately stop
/// driving — with `OSSI=0` (the value `init()` programs), the AF
/// block releases every gate-driver pad → no FETs conducting.
/// `CCRx` / `CCER` are left intact, so `arm_output()` resumes from
/// the same waveform state without re-programming anything.
#[inline]
pub fn all_off() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.bdtr.modify(|_, w| w.moe().clear_bit());
}

/// Re-arm after `all_off`: set `MOE` in BDTR. The timer resumes
/// driving whatever waveform `CCRx` / `CCER` currently describe.
#[inline]
pub fn arm_output() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.bdtr.modify(|_, w| w.moe().set_bit());
}

#[inline]
pub const fn max_duty() -> u16 {
    TIM1_AUTORELOAD
}
