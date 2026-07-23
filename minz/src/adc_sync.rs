//! Injected-group ADC: PWM-synchronous mid-ON sampling for control
//! (the am32_clone observer + bench-safety kills).
//!
//! Hardware-triggered by TIM1 TRGO2 (OC4REF falling edge at
//! `CNT == CCR4`, i.e. `SAMPLE_TICKS` into each PWM cycle, deep in
//! the high-side ON window). Four channels in one burst: ch9 (PA4,
//! phase A), ch10 (PA5, phase B), ch8 (current), ch11 (PA6, vbat).
//! `inj_read()` returns all four from JDR1-4 — a deterministic
//! mid-ON sample per PWM cycle for telemetry, overcurrent and the
//! vbat-floor kill.
//!
//! After [`start`] runs, the HAL's `OneShot::read` (`SenseAdc`) must
//! NOT be called — it rewrites SQR/CFGR under the armed group.
//! `SenseAdc` remains useful for power-up/calibration.

use crate::hal::stm32::{ADC1, TIM1};

/// Injected-group trigger point within the PWM cycle, TIM1 ticks
/// (80 MHz). 100 ticks = 1.25 µs (AM32's own CCR4): the 4-channel
/// injected sequence (A/B @47.5, current @12.5, vbat @47.5) must
/// finish INSIDE the high-side ON window, or the phase channels
/// sample low-side recirculation (~0 V). Whole burst ends ~3.5 µs;
/// ON = duty ≥ 333 ticks = 4.2 µs at amp 10, so it fits with margin
/// down to amp ≈ 8. Hard FLOOR: below ~1.0 µs the first channel
/// samples during dead-time + FET turn-on and reads 0. vbat is last
/// (phase-insensitive: a battery-divider node, valid in ON or OFF).
pub const SAMPLE_TICKS: u16 = 100;

/// Configure and arm the injected group. Call after TIM1 init and
/// after the HAL `ADC::new` power-up/calibration (`SenseAdc::new`).
pub fn start(sample_ticks: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    let adc = unsafe { &*ADC1::ptr() };

    // TIM1: sample point + OC4REF as TRGO2 (drives the INJECTED
    // trigger now). CCR4 is preloaded (OC4PE at init) — next update.
    tim1.ccr4.write(|w| w.ccr().bits(sample_ticks));
    // CR2.MMS2[23:20] = 0b0111 (OC4REF → TRGO2). No PAC accessor.
    tim1.cr2
        .modify(|r, w| unsafe { w.bits((r.bits() & !(0xF << 20)) | (0b0111 << 20)) });

    // Enable ADC idempotently (RM0394 16.4.9). All flag waits bounded
    // (crate::spin — unbounded hardware-flag spins are banned).
    if adc.cr.read().aden().bit_is_clear() {
        crate::spin::spin_until(100_000, || adc.cr.read().addis().bit_is_clear());
        adc.isr.write(|w| w.adrdy().set_bit());
        adc.cr.modify(|_, w| w.aden().set_bit());
        crate::spin::spin_until(100_000, || adc.isr.read().adrdy().bit_is_set());
    }
    // SQR/JSQR/SMPR/CFGR writes require no conversion in flight.
    if adc.cr.read().adstart().bit_is_set() {
        adc.cr.modify(|_, w| w.adstp().set_bit());
        crate::spin::spin_until(100_000, || adc.cr.read().adstart().bit_is_clear());
    }
    if adc.cr.read().jadstart().bit_is_set() {
        adc.cr.modify(|_, w| w.jadstp().set_bit());
        crate::spin::spin_until(100_000, || adc.cr.read().jadstart().bit_is_clear());
    }

    // Sample times: 47.5 cycles on the phase channels (ch9/ch10) —
    // REQUIRED with the 3.3 kΩ divider sources; shorter blinds the
    // sector-2 confirm (2×A ≈ vbus at the ZC, razor margin), proven
    // by A/B at both carriers. ch8 (current) 12.5 — INA180 output is
    // low-Z, and the short sample is what lets the injected burst fit
    // the ON window. ch11 (vbat) 47.5: in the injected burst
    // (0.75 µs), no longer the 8.2 µs software pump that used to
    // collide mid-ON.
    adc.smpr1
        .modify(|_, w| unsafe { w.smp8().bits(0b010).smp9().bits(0b100) });
    adc.smpr2
        .modify(|_, w| unsafe { w.smp10().bits(0b100).smp11().bits(0b100) });

    // Injected group: 4 conversions ch9(A), ch10(B), ch8(current),
    // ch11(PA6 vbat), hardware-triggered on TIM1_TRGO2 falling edge.
    // JSQR: JL[1:0]=3 | JEXTSEL[5:2]=0b1000 (TIM1_TRGO2, RM0394
    // injected trigger table) | JEXTEN[7:6]=0b10 (falling) |
    // JSQ1[12:8]=9 | JSQ2[18:14]=10 | JSQ3[24:20]=8 | JSQ4[30:26]=11.
    adc.jsqr.write(|w| unsafe {
        w.bits(3 | (0b1000 << 2) | (0b10 << 6) | (9 << 8) | (10 << 14) | (8 << 20) | (11 << 26))
    });

    // CFGR: JQDIS=1 (bit 31 — plain JSQR injected mode, no queue;
    // hardware-triggered injected still launches on JEXTEN). The
    // regular group is unused: no DMA, no CONT, EXTEN=0.
    adc.cfgr.write(|w| unsafe { w.bits(1 << 31) });

    // Arm the INJECTED group. JADSTART arms it — a hardware-triggered
    // injected group (JEXTEN != 0) still needs JADSTART set once,
    // exactly as the regular group needs ADSTART; it then stays armed
    // and converts on each TRGO2 falling edge until JADSTP. (Omitting
    // it left JDR at 0 — vbat/A/B read 0.)
    adc.cr.modify(|_, w| w.jadstart().set_bit());
}

/// Newest completed injected burst: `(phase_a, phase_b, current,
/// vbat)` from JDR1-4, all mid-ON at `SAMPLE_TICKS`. Intended caller:
/// `TIM1_UP_TIM16` at the cycle wrap — this cycle's injected burst
/// (triggered ~1.25 µs in) finished tens of µs earlier. Reproduces
/// the old `last_frame()` `(A, B, current)` deterministically and
/// adds vbat.
#[inline]
pub fn inj_read() -> (u16, u16, u16, u16) {
    let adc = unsafe { &*ADC1::ptr() };
    (
        (adc.jdr1.read().bits() & 0x0FFF) as u16,
        (adc.jdr2.read().bits() & 0x0FFF) as u16,
        (adc.jdr3.read().bits() & 0x0FFF) as u16,
        (adc.jdr4.read().bits() & 0x0FFF) as u16,
    )
}

/// The `minz_core::am32_hal::InjAdc` register impl over the ADC1
/// injected group — zero-sized, static dispatch; delegates to
/// [`inj_read`], unchanged.
pub struct InjAdc1;

impl minz_core::am32_hal::InjAdc for InjAdc1 {
    #[inline(always)]
    fn inj_read(&self) -> (u16, u16, u16, u16) {
        inj_read()
    }
}
