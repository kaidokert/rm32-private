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

#[inline]
pub const fn max_duty() -> u16 {
    TIM1_AUTORELOAD
}
