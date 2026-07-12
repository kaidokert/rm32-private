//! Sense-channel conversions for the Vimdrones L431 board. One
//! authoritative calibration: the sag-kill report used to carry its
//! own hardcoded `raw × 7507 / 1000` while the `i` readout went
//! through `adc_to_mv` × divider — two calibrations for the same
//! quantity that silently diverge if the ADC cal changes.
//!
//! The ADC-count → mV step stays in firmware (`SenseAdc::adc_to_mv`,
//! it uses the chip's VREFINT calibration); everything after the mV
//! is here.

/// 3.6 k / 30 k vbat divider: ratio (30+3.6)/3.6 ≈ 9.33×.
pub const VBAT_DIVIDER_X100: u32 = 933;

/// INA180A1 (20 V/V) × 1.5 mΩ shunt → 30 mV per amp.
pub const ISNS_MV_PER_AMP: u32 = 30;

/// Supply voltage in mV from the ADC pin voltage in mV.
pub fn vbat_mv(adc_mv: u32) -> u32 {
    adc_mv * VBAT_DIVIDER_X100 / 100
}

/// Phase-shunt current in mA from the ADC pin voltage in mV.
pub fn isns_ma(adc_mv: u32) -> u32 {
    adc_mv * 1000 / ISNS_MV_PER_AMP
}

/// Nominal (uncalibrated) ADC count → mV, for host-side anchors and
/// scripts; firmware uses the VREFINT-calibrated `adc_to_mv`.
pub fn adc_mv_nominal(raw: u16) -> u32 {
    raw as u32 * 3300 / 4095
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_from_the_bench() {
        // raw 1080 ≈ 8.11 V — the documented live anchor.
        let v = vbat_mv(adc_mv_nominal(1080));
        assert!((8_050..=8_180).contains(&v), "got {v} mV");
        // Sag absolute floor raw 793 ≈ 5.96 V (guards::VBAT_ABS_FLOOR_RAW).
        let v = vbat_mv(adc_mv_nominal(crate::guards::VBAT_ABS_FLOOR_RAW));
        assert!((5_900..=6_020).contains(&v), "got {v} mV");
        // 30 mV at the pin = 1 A.
        assert_eq!(isns_ma(30), 1_000);
        assert_eq!(isns_ma(0), 0);
    }
}
