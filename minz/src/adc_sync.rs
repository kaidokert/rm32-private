//! PWM-synchronous continuous current sampling (GECKO, current half).
//!
//! TIM1 already generates OC4REF (PWM mode 1, preloaded CCR4). This
//! module routes OC4REF to TRGO2 (`CR2.MMS2 = 0b0111`) and arms ADC1
//! to convert regular channel 8 (PA3 = INA180 current-sense output)
//! on every **falling** edge of OC4REF — i.e. at `CNT == CCR4` ticks
//! into each 24 kHz PWM cycle, inside the high-side ON window where
//! battery-shunt current equals motor current. No DMA and no new
//! IRQs: the `TIM1_UP_TIM16` ISR (already firing once per PWM cycle)
//! reads `DR` at the cycle wrap, ~35 µs after the conversion landed.
//!
//! Battery voltage (channel 11, PA6) moves to the **injected** group:
//! software-triggered on demand, hardware-inserted between regular
//! conversions, so it never fights the trigger-armed regular channel.
//!
//! After [`start`] runs, the HAL's `OneShot::read` (`SenseAdc::
//! isns_raw` / `vbat_raw`) must NOT be called — it rewrites SQR/CFGR
//! under the armed trigger. `SenseAdc` remains useful for its
//! power-up/calibration side effects and `adc_to_mv`.

use crate::hal::stm32::{ADC1, TIM1};

/// Default sample point within the PWM cycle, TIM1 ticks (80 MHz).
/// 250 ≈ 3.1 µs after the high-side rises — past the INA180's ~1.3 µs
/// settling (350 kHz bandwidth) while still inside the ON window for
/// any duty ≥ 7.5 % of ARR (bench spin floor is 15 %). Below that
/// duty the trigger lands after the ON window and reads recirculation
/// (≈ 0 A through the battery shunt) — degraded but harmless.
pub const SAMPLE_TICKS: u16 = 250;

/// Route OC4REF→TRGO2 and start hardware-triggered conversions of
/// channel 8. Call after TIM1 init and after the HAL `ADC::new`
/// power-up/calibration (`SenseAdc::new`).
pub fn start(sample_ticks: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    let adc = unsafe { &*ADC1::ptr() };

    // TIM1: sample point + OC4REF as TRGO2. CCR4 is preloaded (OC4PE
    // set at init) — takes effect at the next update event.
    tim1.ccr4.write(|w| w.ccr().bits(sample_ticks));
    // CR2.MMS2[23:20] = 0b0111 (OC4REF → TRGO2). The stm32l4 PAC has
    // no `mms2` accessor (SVD hole) — raw bits, preserving the rest.
    tim1.cr2
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 20)) | (0b0111 << 20)) });

    // The HAL powered and calibrated the ADC but its enable state
    // depends on call history — make it enabled, idempotently
    // (RM0394 16.4.9 sequence, same as the HAL's `enable`).
    if adc.cr.read().aden().bit_is_clear() {
        while adc.cr.read().addis().bit_is_set() {}
        adc.isr.write(|w| w.adrdy().set_bit());
        adc.cr.modify(|_, w| w.aden().set_bit());
        while adc.isr.read().adrdy().bit_is_clear() {}
    }
    // CFGR/SQR/SMPR writes below require no conversion in flight.
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        while adc.cr.read().adstart().bit_is_set() {}
    }

    // Sample times: ch8 sees the INA180's buffered output → 47.5 cyc
    // (same as rm32's L431 config); ch11 keeps 640.5 cyc for the
    // ~3.2 kΩ vbat divider.
    adc.smpr1.modify(|_, w| unsafe { w.smp8().bits(0b100) });
    adc.smpr2.modify(|_, w| unsafe { w.smp11().bits(0b111) });

    // Regular group: single conversion of ch8 per trigger.
    adc.sqr1.write(|w| unsafe { w.l().bits(0).sq1().bits(8) });
    // Injected group: single ch11, software trigger (JQDIS below).
    adc.jsqr
        .write(|w| unsafe { w.jl().bits(0).jsq1().bits(11) });

    // EXTSEL 0b1010 = EXT10 = TIM1_TRGO2 (RM0394 ADC1 trigger table),
    // EXTEN 0b10 = falling edge of OC4REF = CNT reaching CCR4.
    // OVRMOD=1: overwrite DR on overrun so a slow reader can never
    // stall the conversion stream. JQDIS=1 (bit 31, no PAC accessor):
    // plain JSQR-register injected mode, no queue semantics.
    adc.cfgr
        .write(|w| unsafe { w.bits((0b1010 << 6) | (0b10 << 10) | (1 << 12) | (1 << 31)) });

    // Arm: with EXTEN != 0 this waits for triggers rather than
    // converting immediately; stays armed until ADSTP.
    adc.cr.modify(|_, w| w.adstart().set_bit());
}

/// Latest completed current conversion (raw 12-bit). Reading DR also
/// clears EOC. Intended caller: the `TIM1_UP_TIM16` ISR, once per PWM
/// cycle.
#[inline]
pub fn last_raw() -> u16 {
    let adc = unsafe { &*ADC1::ptr() };
    (adc.dr.read().bits() & 0x0FFF) as u16
}

/// On-demand vbat read via the injected group (~8.2 µs blocking at
/// the 640.5-cycle sample time). Safe while regular conversions are
/// trigger-armed — the hardware inserts the injected conversion after
/// any in-flight regular one.
pub fn read_vbat_injected() -> u16 {
    let adc = unsafe { &*ADC1::ptr() };
    adc.isr.write(|w| w.jeos().set_bit());
    adc.cr.modify(|_, w| w.jadstart().set_bit());
    while adc.isr.read().jeos().bit_is_clear() {}
    adc.isr.write(|w| w.jeos().set_bit());
    (adc.jdr1.read().bits() & 0x0FFF) as u16
}
