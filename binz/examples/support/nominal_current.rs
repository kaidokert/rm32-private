//! Nominal exploration setting, NOT measured calibration or a certified bound.
//! Board documentation: 7 mOhm low-side shunts, nominal CSA gain 10.
//! ADC offset/sampling uncertainty is not converted into fictitious accuracy.
// Exploration ceiling follows the operator's existing PSU limit, not a
// threshold chosen to pass a failed sample. Nominal conversion is not calibration.
// Higher settings are explicit in build provenance and remain below or equal
// to the operator's independently set PSU ceiling.
#[cfg(any(
    all(feature = "bench-current-1500", feature = "bench-current-2000"),
    all(feature = "bench-current-1500", feature = "bench-current-2500"),
    all(feature = "bench-current-1500", feature = "bench-current-3500"),
    all(feature = "bench-current-1500", feature = "bench-current-4000"),
    all(feature = "bench-current-2000", feature = "bench-current-2500"),
    all(feature = "bench-current-2000", feature = "bench-current-3500"),
    all(feature = "bench-current-2000", feature = "bench-current-4000"),
    all(feature = "bench-current-2500", feature = "bench-current-3500"),
    all(feature = "bench-current-2500", feature = "bench-current-4000"),
    all(feature = "bench-current-3500", feature = "bench-current-4000")
))]
compile_error!("choose exactly one nominal current campaign setting");
pub const TARGET_MA: u32 = if cfg!(feature = "bench-current-4000") {
    4000
} else if cfg!(feature = "bench-current-3500") {
    3500
} else if cfg!(feature = "bench-current-2500") {
    2500
} else if cfg!(feature = "bench-current-2000") {
    2000
} else if cfg!(feature = "bench-current-1500") {
    1500
} else {
    1000
};
pub const VDDA_MV: u32 = 3600;
pub const GAIN: u32 = 10;
pub const SHUNT_MOHM: u32 = 7;
pub const SCANS: u32 = if cfg!(feature = "bench-current-fast-20") {
    20
} else if cfg!(all(
    feature = "bench-startup-adc",
    not(feature = "bench-startup-20k")
)) {
    100
} else {
    50
};
// Keep precision until the final division; this entire expression is const.
// mA*mOhm /1000 is mV; floor cannot increase the positive raw allowance.
// Wide intermediate is evaluated by rustc, never on the M0.
pub const RAW_LIMIT: u32 =
    (TARGET_MA as u64 * SHUNT_MOHM as u64 * GAIN as u64 * 4096 * SCANS as u64
        / (1000 * VDDA_MV) as u64) as u32;
const _: () = assert!(
    RAW_LIMIT
        == if cfg!(feature = "bench-current-4000") {
            if SCANS == 100 {
                31857
            } else if SCANS == 20 {
                6371
            } else {
                15928
            }
        } else if cfg!(feature = "bench-current-3500") {
            if SCANS == 100 {
                27875
            } else if SCANS == 20 {
                5575
            } else {
                13937
            }
        } else if cfg!(feature = "bench-current-2500") {
            if SCANS == 100 {
                19911
            } else if SCANS == 20 {
                3982
            } else {
                9955
            }
        } else if cfg!(feature = "bench-current-2000") {
            if SCANS == 100 {
                15928
            } else if SCANS == 20 {
                3185
            } else {
                7964
            }
        } else if cfg!(feature = "bench-current-1500") {
            if SCANS == 100 {
                11946
            } else if SCANS == 20 {
                2389
            } else {
                5973
            }
        } else if SCANS == 100 {
            7964
        } else if SCANS == 20 {
            1592
        } else {
            3982
        }
);
/// Idle-only conversion from factory-VREF-derived VDDA. CSA gain/shunt remain
/// nominal; this does not certify current calibration or sampling uncertainty.
pub fn raw_for_vdda(vdda_mv: u32) -> Option<u32> {
    if !(2700..=3600).contains(&vdda_mv) {
        return None;
    }
    const NUMERATOR: u32 =
        (TARGET_MA as u64 * SHUNT_MOHM as u64 * GAIN as u64 * 4096 * SCANS as u64 / 1000) as u32;
    Some(NUMERATOR / vdda_mv)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nominal_scale_and_rounding_are_explicit() {
        let numerator = TARGET_MA as u64 * SHUNT_MOHM as u64 * GAIN as u64 * 4096 * SCANS as u64;
        let denominator = (1000 * VDDA_MV) as u64;
        assert!(RAW_LIMIT as u64 * denominator <= numerator);
        assert!(numerator - RAW_LIMIT as u64 * denominator < denominator);
        assert!(RAW_LIMIT > 0 && RAW_LIMIT < 4095 * 3 * SCANS);
    }
    #[test]
    fn factory_vref_scale_keeps_target_not_a_tuned_raw_threshold() {
        for vdda in 2700..=3600 {
            let raw = raw_for_vdda(vdda).unwrap() as u64;
            let numerator =
                TARGET_MA as u64 * SHUNT_MOHM as u64 * GAIN as u64 * 4096 * SCANS as u64;
            let denominator = 1000 * vdda as u64;
            assert!(raw * denominator <= numerator);
            assert!(numerator - raw * denominator < denominator);
        }
        assert_eq!(raw_for_vdda(3600), Some(RAW_LIMIT));
        assert_eq!(raw_for_vdda(2699), None);
        assert_eq!(raw_for_vdda(3601), None);
    }
}
