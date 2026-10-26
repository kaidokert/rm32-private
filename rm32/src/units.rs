//! Zero-cost newtype wrappers for physical units.
//!
//! Prevents mixing up ADC counts with centivolts, or timer ticks with microseconds.
//! All types are `Copy` and transparent — zero runtime overhead.

/// Battery voltage in centivolts (10 mV): AM32's own unit (1260 = 12.60 V),
/// spoken by LVC, the voltage ramp, KISS and EDT. Bench code that wants mV
/// multiplies by 10 at its own boundary.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(transparent)]
pub struct CentiVolts(pub u16);

impl CentiVolts {
    pub const ZERO: Self = Self(0);

    /// Per-cell check: voltage < cell_count * per_cell_cv
    pub fn below_cell_threshold(self, cell_count: u8, per_cell_cv: u16) -> bool {
        let threshold = cell_count as u16 * per_cell_cv;
        threshold > 0 && self.0 < threshold
    }
}

/// Motor current in milliamps.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(transparent)]
pub struct MilliAmps(pub i16);

impl MilliAmps {
    pub const ZERO: Self = Self(0);

    /// Convert to centiamps for KISS telemetry.
    pub fn to_centiamps(self) -> u16 {
        (self.0 / 10) as u16
    }
}

/// Temperature in degrees Celsius.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(transparent)]
pub struct DegreesCelsius(pub i16);

impl DegreesCelsius {
    /// As i8 for KISS telemetry packet.
    pub fn to_i8(self) -> i8 {
        self.0 as i8
    }
}

/// Timer ticks (0.5µs resolution at 2MHz timer clock).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
#[repr(transparent)]
pub struct TimerTicks(pub u32);

impl TimerTicks {
    pub const ZERO: Self = Self(0);

    /// Convert to eRPM (in units of 100 eRPM).
    /// Formula: 600000 / ticks (when ticks = e_com_time)
    pub fn to_erpm_100(self) -> u16 {
        (600000u32.checked_div(self.0).unwrap_or(0)) as u16
    }
}

/// Raw ADC count (12-bit, 0-4095).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(transparent)]
pub struct AdcCount(pub u16);

use fixed::types::U16F16;

/// Millivolts per ADC count: 3300 / 4095 ≈ 0.8059 mV/count.
/// As fixed-point, this replaces the fragile `* 3300 / 4095` integer chain.
const MV_PER_COUNT: U16F16 = U16F16::from_bits((3300u64 * 65536 / 4095) as u32);

/// Current sense scaling: 3300 / 41 ≈ 80.49 (C uses integer truncation to 80).
/// We match C's integer behavior: 3300/41 = 80 (truncated).
const CURRENT_MV_SCALE: i32 = 3300 / 41; // = 80, matches C

impl AdcCount {
    /// Convert to centivolts given the divider as its true ratio x100
    /// (11:1 = 1100): pin millivolts x ratio / 10. Against AM32's
    /// `* 3300 / 4095 * 110 / 100` the fixed-point MV_PER_COUNT truncates up
    /// to 1 pin-mV lower (<= 2 cV at 11:1; 4095 -> 3628 vs 3630).
    pub fn to_centivolts(self, voltage_divider_x100: u16) -> CentiVolts {
        let mv_pin = MV_PER_COUNT * U16F16::from_num(self.0);
        let scaled = mv_pin.to_num::<u32>() * voltage_divider_x100 as u32 / 1000;
        CentiVolts(scaled as u16)
    }

    /// Convert to milliamps given offset and sensitivity.
    /// Formula: (count * 80 - offset * 100) / mv_per_amp
    pub fn to_milliamps(self, current_offset: i16, mv_per_amp: u16) -> MilliAmps {
        let mv = (self.0 as i32) * CURRENT_MV_SCALE - (current_offset as i32) * 100;
        if mv_per_amp > 0 {
            MilliAmps((mv / mv_per_amp as i32) as i16)
        } else {
            MilliAmps::ZERO
        }
    }
}

/// Pure temperature calculation from raw ADC and calibration values
/// (testable on host).
///
/// `vdda_mv` / `cal_vref_mv`: the factory TS_CAL points are measured
/// at a per-family reference supply (L4/G0/G4: 3.0 V; F0: 3.3 V),
/// while the board runs the ADC from VDDA (3.3 V here). The raw
/// reading must be rescaled into the calibration frame first —
/// exactly what ST's `__LL_ADC_CALC_TEMPERATURE(vdda, raw, res)`
/// does, and what AM32 calls (main.c: `(3300, ADC_raw_temp, 12B)`).
/// Omitting the rescale read ~13 C low on the 3.3 V bench
/// (the "7-9 C on a 20 C bench" defect).
pub fn calc_temperature_pure(
    raw: u16,
    ts_cal1: u16,
    ts_cal2: u16,
    cal1_temp: i32,
    cal2_temp: i32,
    vdda_mv: i32,
    cal_vref_mv: i32,
) -> DegreesCelsius {
    let c1 = ts_cal1 as i32;
    let c2 = ts_cal2 as i32;
    if c2 == c1 || cal_vref_mv == 0 {
        return DegreesCelsius(25);
    }
    let raw_scaled = raw as i32 * vdda_mv / cal_vref_mv;
    DegreesCelsius(((cal2_temp - cal1_temp) * (raw_scaled - c1) / (c2 - c1) + cal1_temp) as i16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centivolts_cell_threshold() {
        let v = CentiVolts(1000); // 10V
        assert!(v.below_cell_threshold(3, 370)); // 3 * 3.7V = 11.1V > 10V
        assert!(!v.below_cell_threshold(2, 370)); // 2 * 3.7V = 7.4V < 10V
    }

    #[test]
    fn adc_to_centivolts_tracks_am32_for_11_to_1() {
        // AM32: ADC_raw_volts * 3300 / 4095 * VOLTAGE_DIVIDER(110) / 100.
        // The fixed-point pin-mV truncates at most 1 mV lower (<= 2 cV).
        for raw in 0..=4095u16 {
            let am32 = (raw as u32 * 3300 / 4095 * 110 / 100) as i32;
            let ours = AdcCount(raw).to_centivolts(1100).0 as i32;
            assert!(
                (0..=2).contains(&(am32 - ours)),
                "raw {raw}: {ours} vs {am32}"
            );
        }
    }

    #[test]
    fn adc_to_centivolts_bench_dividers() {
        // L431 9.33:1 and binz 11.94:1: 1.000 V at the pin.
        let raw = 1241; // 3300/4095*1241 = 1000.1 mV
        assert_eq!(AdcCount(raw).to_centivolts(933).0, 933); // 9.33 V
        assert_eq!(AdcCount(raw).to_centivolts(1194).0, 1194); // 11.94 V
    }

    #[test]
    fn adc_to_milliamps() {
        let count = AdcCount(2048);
        let ma = count.to_milliamps(498, 20);
        assert!(ma.0 > 5000 && ma.0 < 6000); // ~5750mA
    }

    #[test]
    fn timer_ticks_to_erpm() {
        assert_eq!(TimerTicks(600).to_erpm_100(), 1000); // 100k eRPM
        assert_eq!(TimerTicks(0).to_erpm_100(), 0);
    }

    // --- Temperature calculation tests (pure math, no hardware) ---

    #[test]
    fn temp_at_cal1_returns_cal1_temp() {
        // raw == ts_cal1 → should return cal1_temp exactly
        assert_eq!(
            calc_temperature_pure(1000, 1000, 1500, 30, 130, 3000, 3000),
            DegreesCelsius(30)
        );
    }

    #[test]
    fn temp_at_cal2_returns_cal2_temp() {
        assert_eq!(
            calc_temperature_pure(1500, 1000, 1500, 30, 130, 3000, 3000),
            DegreesCelsius(130)
        );
    }

    #[test]
    fn temp_midpoint() {
        // Midpoint between cal1 and cal2
        let mid = calc_temperature_pure(1250, 1000, 1500, 30, 130, 3000, 3000);
        assert_eq!(mid, DegreesCelsius(80)); // (30+130)/2 = 80
    }

    #[test]
    fn temp_equal_cals_returns_25() {
        assert_eq!(
            calc_temperature_pure(1234, 1000, 1000, 30, 130, 3300, 3000),
            DegreesCelsius(25)
        );
    }

    #[test]
    fn temp_f051_range() {
        // F051: cal1=30C, cal2=110C. Typical cal values ~700, ~900
        let t = calc_temperature_pure(800, 700, 900, 30, 110, 3300, 3300);
        assert_eq!(t, DegreesCelsius(70)); // 50% of range = (30+110)/2 = 70
    }

    /// Regression for the "7-9 C on a 20 C bench" defect: L431 TS_CAL
    /// is measured at 3.0 V, the bench runs VDDA=3.3 V. With typical
    /// L431 cal values (cal1~1034 @30 C, cal2~1381 @130 C) a ~21 C die
    /// reads raw~912 at 3.3 V. Unscaled that interpolates to ~-5 C;
    /// with the AM32-matching VDDA/3000 rescale it lands ~21 C.
    #[test]
    fn temp_l431_vdda_rescale_matches_am32() {
        let unscaled = calc_temperature_pure(912, 1034, 1381, 30, 130, 3000, 3000);
        let scaled = calc_temperature_pure(912, 1034, 1381, 30, 130, 3300, 3000);
        assert!(unscaled.0 < 5, "unscaled reads cold: {}", unscaled.0);
        assert!(
            (18..=25).contains(&scaled.0),
            "rescaled should read ~21 C, got {}",
            scaled.0
        );
    }

    #[test]
    fn adc_zero_offset_current() {
        // Zero offset, 20 mv/A: mid-range ADC = ~4024 mA
        let ma = AdcCount(2048).to_milliamps(0, 20);
        assert!(
            ma.0 > 8000,
            "expected high mA with zero offset, got {}",
            ma.0
        );
    }

    #[test]
    fn adc_zero_mvperamp_returns_zero() {
        assert_eq!(AdcCount(2048).to_milliamps(498, 0), MilliAmps::ZERO);
    }
}
