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

use crate::platform::Platform;

/// Raw ADC count (12-bit, 0-4095).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(transparent)]
pub struct AdcCount(pub u16);

/// Pin millivolts per ADC count, 3300 / 4095, as Q16 (truncated): the
/// pin-mV step is `(raw * MV_PER_COUNT_Q16) >> 16`.
const MV_PER_COUNT_Q16: u32 = (3300u64 * 65536 / 4095) as u32;
/// Largest reading the `Adc` trait can return. The conversions are exact
/// over all of `u16`, not only 12-bit counts: a port may encode its reading
/// (binz returns metered mA + 2000, above 4095 past ~2.1 A).
const ADC_MAX: u32 = u16::MAX as u32;
/// Largest pin-mV a reading gives (`ADC_MAX * MV_PER_COUNT_Q16` fits u32).
const MV_PIN_MAX: u32 = (ADC_MAX * MV_PER_COUNT_Q16) >> 16;

/// Current sense scaling: 3300 / 41 ≈ 80.49 (C uses integer truncation to 80).
/// We match C's integer behavior: 3300/41 = 80 (truncated).
const CURRENT_MV_SCALE: i32 = 3300 / 41; // = 80, matches C

/// `floor((a * x + b) / d)` for every `x` in `0..=x_max` with a multiply,
/// add and shift and no divide (cores without hardware divide would
/// otherwise call `__aeabi_*div`). [`AffineDiv::new`] runs at compile time,
/// checks every `x` against the exact quotient and fails the build if no
/// exact form exists.
///
/// Three forms, first exact one wins (`a = A*d + a1`, `a2 = d - a1`):
/// direct `floor((a x + b) / d)`; `A x + floor((a1 x + b) / d)`; and
/// `(A + 1) x - ceil((a2 x - b) / d)`. Peeling off the integer part keeps the
/// multiplier small enough for the precision a large `d` needs. A 32-bit
/// multiply is preferred; only when none of the three has an exact 32-bit
/// form over the whole domain does it take a 64-bit multiply (on M0 a
/// constant-time `__aeabi_lmul`, still no divide): the voltage dividers
/// over all of `u16`'s pin-mV range need one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineDiv {
    lin: i32,
    core: MulShift,
    negate: bool,
    /// The domain [`AffineDiv::new`] proved exact; `apply` beyond it wraps.
    x_max: u32,
}

/// `((x * mul + add) >> shift) - sub` = `floor((alpha x + beta) / d)`, in
/// 32-bit arithmetic, or 64-bit when `wide`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MulShift {
    mul: u64,
    add: u64,
    shift: u32,
    sub: i32,
    wide: bool,
}

impl MulShift {
    const ZERO: Self = Self {
        mul: 0,
        add: 0,
        shift: 0,
        sub: 0,
        wide: false,
    };

    /// Exact form of `floor((alpha x + beta) / d)` for `x <= x_max`, if one
    /// exists with `x * mul + add` inside u32 (u64 when `wide`).
    const fn find(alpha: i32, beta: i32, d: i32, x_max: u32, wide: bool) -> Option<Self> {
        // beta = d * t + r with 0 <= r < d: floor((alpha x + r) / d) + t.
        let t = beta.div_euclid(d);
        let r = beta.rem_euclid(d);
        let limit = if wide {
            u64::MAX as u128
        } else {
            u32::MAX as u128
        };
        let mut shift = if wide { 63 } else { 31 };
        loop {
            let mul_lo = ((alpha as u128) << shift) / d as u128;
            let add_lo = ((r as u128) << shift) / d as u128;
            let mut k = 0;
            while k < 4 {
                let mul = mul_lo + (k & 1);
                let add = add_lo + (k >> 1);
                if x_max as u128 * mul + add <= limit {
                    let c = Self {
                        mul: mul as u64,
                        add: add as u64,
                        shift,
                        sub: -t,
                        wide,
                    };
                    if c.exact(alpha, r, d, x_max) {
                        return Some(c);
                    }
                }
                k += 1;
            }
            if shift == 0 {
                return None;
            }
            shift -= 1;
        }
    }

    const fn exact(self, alpha: i32, r: i32, d: i32, x_max: u32) -> bool {
        let mut x = 0;
        while x <= x_max {
            let want = ((alpha as i64 * x as i64 + r as i64) / d as i64) as i32;
            if self.apply(x) + self.sub != want {
                return false;
            }
            x += 1;
        }
        true
    }

    #[inline(always)]
    const fn apply(self, x: u32) -> i32 {
        let q = if self.wide {
            ((x as u64 * self.mul + self.add) >> self.shift) as u32
        } else {
            (x * self.mul as u32 + self.add as u32) >> self.shift
        };
        q as i32 - self.sub
    }
}

impl AffineDiv {
    /// Always 0 (a board without the sensor).
    pub const ZERO: Self = Self {
        lin: 0,
        core: MulShift::ZERO,
        negate: false,
        x_max: u32::MAX,
    };

    /// Exact divide-free form of `floor((a * x + b) / d)`, `x <= x_max`.
    pub const fn new(a: i32, b: i32, d: i32, x_max: u32) -> Self {
        assert!(a >= 0 && d > 0);
        let a1 = a % d;
        let mut wide = false;
        loop {
            if let Some(core) = MulShift::find(a, b, d, x_max, wide) {
                return Self {
                    lin: 0,
                    core,
                    negate: false,
                    x_max,
                };
            }
            if let Some(core) = MulShift::find(a1, b, d, x_max, wide) {
                return Self {
                    lin: a / d,
                    core,
                    negate: false,
                    x_max,
                };
            }
            // ceil((a2 x - b) / d) = floor((a2 x - b + d - 1) / d)
            if let Some(core) = MulShift::find(d - a1, d - 1 - b, d, x_max, wide) {
                return Self {
                    lin: a / d + 1,
                    core,
                    negate: true,
                    x_max,
                };
            }
            if wide {
                panic!("no exact multiply-shift form for this board constant");
            }
            wide = true;
        }
    }

    /// The quotient for `x <= x_max` (debug builds assert it; beyond it
    /// the 32-bit multiply wraps).
    #[inline(always)]
    pub const fn apply(self, x: u32) -> i32 {
        debug_assert!(x <= self.x_max);
        let c = self.core.apply(x);
        self.lin * x as i32 + if self.negate { -c } else { c }
    }
}

/// Compile-time ADC conversions of a platform's board constants.
/// Blanket-implemented; read through [`AdcCount::centivolts`] and
/// [`AdcCount::milliamps`].
pub(crate) trait AdcScale: Platform + Sized {
    /// Pin mV -> centivolts: `mv * VOLTAGE_DIVIDER_X100 / 1000`.
    const PIN_MV_TO_CV: AffineDiv =
        AffineDiv::new(Self::VOLTAGE_DIVIDER_X100 as i32, 0, 1000, MV_PIN_MAX);
    /// Current numerator `n = raw * 80 - CURRENT_OFFSET * 100`, quotient
    /// truncated toward zero like AM32's C: `floor(n / mVpA)` where `n >= 0`
    /// (raw at or above [`Self::CURRENT_NEG_BELOW`]) ...
    const CURRENT_POS: AffineDiv = if Self::MILLIVOLT_PER_AMP == 0 {
        AffineDiv::ZERO
    } else {
        AffineDiv::new(
            CURRENT_MV_SCALE,
            -(Self::CURRENT_OFFSET as i32 * 100),
            Self::MILLIVOLT_PER_AMP as i32,
            ADC_MAX,
        )
    };
    /// ... and `ceil(n / mVpA) = floor((n + mVpA - 1) / mVpA)` below it.
    const CURRENT_NEG: AffineDiv = if Self::MILLIVOLT_PER_AMP == 0 {
        AffineDiv::ZERO
    } else {
        AffineDiv::new(
            CURRENT_MV_SCALE,
            -(Self::CURRENT_OFFSET as i32 * 100) + Self::MILLIVOLT_PER_AMP as i32 - 1,
            Self::MILLIVOLT_PER_AMP as i32,
            ADC_MAX,
        )
    };
    /// First raw count with a non-negative numerator.
    const CURRENT_NEG_BELOW: u32 = if Self::CURRENT_OFFSET <= 0 {
        0
    } else {
        (Self::CURRENT_OFFSET as u32 * 100).div_ceil(CURRENT_MV_SCALE as u32)
    };
}

impl<P: Platform> AdcScale for P {}

impl AdcCount {
    /// Battery voltage in centivolts: pin millivolts (Q16 3300/4095,
    /// truncated) x divider ratio / 10. Against AM32's `* 3300 / 4095 * 110
    /// / 100` the Q16 pin-mV truncates up to 1 pin-mV lower (<= 2 cV at
    /// 11:1; 4095 -> 3628 vs 3630).
    #[inline]
    pub fn centivolts<P: Platform>(self) -> CentiVolts {
        let mv_pin = (self.0 as u32 * MV_PER_COUNT_Q16) >> 16;
        CentiVolts(<P as AdcScale>::PIN_MV_TO_CV.apply(mv_pin) as u16)
    }

    /// Current in milliamps: `(raw * 80 - offset * 100) / mVpA`, truncated
    /// toward zero (AM32); 0 without a sensor.
    #[inline]
    pub fn milliamps<P: Platform>(self) -> MilliAmps {
        let raw = self.0 as u32;
        let q = if raw < <P as AdcScale>::CURRENT_NEG_BELOW {
            <P as AdcScale>::CURRENT_NEG.apply(raw)
        } else {
            <P as AdcScale>::CURRENT_POS.apply(raw)
        };
        MilliAmps(q as i16)
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
    use crate::platform::Platform;

    /// The S2.3 conversions, verbatim (runtime divisions): the reference
    /// the compile-time forms must equal for every raw count.
    fn s2_3_centivolts(raw: u16, voltage_divider_x100: u16) -> CentiVolts {
        use fixed::types::U16F16;
        const MV_PER_COUNT: U16F16 = U16F16::from_bits((3300u64 * 65536 / 4095) as u32);
        let mv_pin = MV_PER_COUNT * U16F16::from_num(raw);
        let scaled = mv_pin.to_num::<u32>() * voltage_divider_x100 as u32 / 1000;
        CentiVolts(scaled as u16)
    }

    fn s2_3_milliamps(raw: u16, current_offset: i16, mv_per_amp: u16) -> MilliAmps {
        let mv = (raw as i32) * CURRENT_MV_SCALE - (current_offset as i32) * 100;
        if mv_per_amp > 0 {
            MilliAmps((mv / mv_per_amp as i32) as i16)
        } else {
            MilliAmps::ZERO
        }
    }

    macro_rules! board {
        ($name:ident, $div:expr, $mvpa:expr, $off:expr) => {
            struct $name;
            impl Platform for $name {
                const TICK_HZ: u32 = 20_000;
                const TIMER_HZ: u32 = 2_000_000;
                const VOLTAGE_DIVIDER_X100: u16 = $div;
                const MILLIVOLT_PER_AMP: u16 = $mvpa;
                const CURRENT_OFFSET: i16 = $off;
                const USE_NTC: bool = false;
            }
        };
    }
    // Every board yaml's (divider, mV/A, offset), the harness default, and
    // the old test constants (offset 498, no sensor).
    board!(Binz, 1194, 80, 1600);
    board!(L431, 933, 3, 0);
    board!(Am32Default, 1100, 20, 0);
    board!(TwelveS, 2100, 25, 0);
    board!(IflightF051, 1100, 30, 0);
    board!(Offset498, 1100, 20, 498);
    board!(NoSensor, 1100, 0, 498);

    fn matches_s2_3<P: Platform>() {
        for raw in 0..=u16::MAX {
            assert_eq!(
                AdcCount(raw).centivolts::<P>(),
                s2_3_centivolts(raw, P::VOLTAGE_DIVIDER_X100),
                "cV raw {raw} div {}",
                P::VOLTAGE_DIVIDER_X100
            );
            assert_eq!(
                AdcCount(raw).milliamps::<P>(),
                s2_3_milliamps(raw, P::CURRENT_OFFSET, P::MILLIVOLT_PER_AMP),
                "mA raw {raw} mVpA {} off {}",
                P::MILLIVOLT_PER_AMP,
                P::CURRENT_OFFSET
            );
        }
    }

    /// S4.4 gate: every board's compile-time conversion equals S2.3 for
    /// every reading the `Adc` trait can return (all of u16, which covers
    /// every 12-bit count and binz's mA-encoded current), to the count.
    #[test]
    fn adc_conversions_equal_s2_3_for_every_count() {
        matches_s2_3::<Binz>();
        matches_s2_3::<L431>();
        matches_s2_3::<Am32Default>();
        matches_s2_3::<TwelveS>();
        matches_s2_3::<IflightF051>();
        matches_s2_3::<Offset498>();
        matches_s2_3::<NoSensor>();
        matches_s2_3::<crate::platform::TestPlatform>();
    }

    /// The S4.4 regression: binz encodes current as metered mA + 2000, so
    /// readings above 4095 are real (S4.4 first shipped a 12-bit domain and
    /// wrapped every current above ~2.1 A modulo 4096 counts).
    /// `apply` is exact only on the domain `new` proved; debug builds
    /// refuse anything beyond it (S4.4's defect was a caller outside it).
    #[test]
    #[should_panic]
    fn apply_beyond_proved_domain_asserts() {
        let f = AffineDiv::new(933, 0, 1000, 4095);
        assert_eq!(f.apply(4095), 933 * 4095 / 1000);
        let _ = f.apply(4096);
    }

    #[test]
    fn binz_current_above_12_bits_is_exact() {
        assert_eq!(AdcCount(6800).milliamps::<Binz>(), MilliAmps(4800));
        assert_eq!(
            AdcCount(u16::MAX).milliamps::<Binz>(),
            s2_3_milliamps(u16::MAX, 1600, 80)
        );
    }

    #[test]
    fn affine_div_matches_division() {
        // The reviewer's counterexample: no single u32 multiply-shift is
        // exact for (raw * 80) / 3; the affine form on raw is.
        let f = AffineDiv::new(80, 0, 3, 4095);
        for raw in 0..=4095 {
            assert_eq!(f.apply(raw), (raw as i32 * 80) / 3);
        }
    }

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
            let ours = AdcCount(raw).centivolts::<Am32Default>().0 as i32;
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
        assert_eq!(AdcCount(raw).centivolts::<L431>().0, 933); // 9.33 V
        assert_eq!(AdcCount(raw).centivolts::<Binz>().0, 1194); // 11.94 V
    }

    #[test]
    fn adc_to_milliamps() {
        let count = AdcCount(2048);
        let ma = count.milliamps::<Offset498>();
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
        let ma = AdcCount(2048).milliamps::<Am32Default>();
        assert!(
            ma.0 > 8000,
            "expected high mA with zero offset, got {}",
            ma.0
        );
    }

    #[test]
    fn adc_zero_mvperamp_returns_zero() {
        assert_eq!(AdcCount(2048).milliamps::<NoSensor>(), MilliAmps::ZERO);
    }
}
