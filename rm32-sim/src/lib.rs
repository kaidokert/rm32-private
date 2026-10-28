//! `rm32-sim`: a host port of the rm32 core that is deliberately unlike the
//! STM32 port (docs/generic_core_design.md §9).
//!
//! | | STM32 port (harness oracle) | rm32-sim |
//! |---|---|---|
//! | control tick | 20 kHz | **40 kHz** |
//! | interval / commutation timer | 2 MHz | **1 MHz** |
//! | ADC | 12-bit, 3.3 V | **10-bit, 3.0 V** |
//! | DShot input | timestamp capture via DMA | **16-bit packets** (bits sliced in hardware, no DMA) |
//!
//! A vector that passes on the STM32 model and fails here exposes a hidden
//! assumption in the core (or in the vector). The board constants (divider,
//! current sense) are AM32's defaults, like the harness's, so only the four
//! port axes above differ.

#![no_std]

use core::marker::PhantomData;

use rm32::board::BoardConfig;
use rm32::platform::Platform;
use rm32::tuning::TuningPolicy;
use rm32::units::{AdcCount, CentiVolts, MilliAmps};

/// The simulated port, generic over the tuning like the harness's platform.
pub struct SimPlatform<T>(PhantomData<T>);

impl<T: TuningPolicy> Platform for SimPlatform<T> {
    const TICK_HZ: u32 = 40_000;
    const TIMER_HZ: u32 = 1_000_000;
    /// Edge capture (servo input) keeps the harness's 64 MHz: design §9
    /// does not vary it, and DShot does not use it here.
    const CAPTURE_HZ: u32 = 64_000_000;
    /// Telemetry is shifted out as bits; the DMA-image shift stays the
    /// harness's for the `gcr_encode` vectors.
    const GCR_SHIFT: u8 = 6;
    const VOLTAGE_DIVIDER_X100: u16 = BoardConfig::DEFAULT.voltage_divider_x100;
    const MILLIVOLT_PER_AMP: u16 = BoardConfig::DEFAULT.millivolt_per_amp;
    const CURRENT_OFFSET: i16 = BoardConfig::DEFAULT.current_offset;
    const USE_NTC: bool = BoardConfig::DEFAULT.use_ntc;
    const ADC_FULL_SCALE: u16 = 1023;
    const ADC_VREF_MV: u16 = 3000;
    const PWM_BASE_PERIOD: u16 = 1999;
    type Tuning = T;
}

/// The ADC count whose conversion is nearest `cv` (ties to the lower
/// count), searched over the port's whole range: what a real ADC at this
/// resolution would hand the core for that bus voltage.
pub fn raw_for_centivolts<P: Platform>(cv: u16) -> u16 {
    nearest(P::ADC_FULL_SCALE, cv as i32, |raw| {
        let CentiVolts(v) = AdcCount(raw).centivolts::<P>();
        v as i32
    })
}

/// The ADC count whose conversion is nearest `ma` (ties to the lower
/// count); 0 without a current sensor.
pub fn raw_for_milliamps<P: Platform>(ma: i16) -> u16 {
    nearest(P::ADC_FULL_SCALE, ma as i32, |raw| {
        let MilliAmps(v) = AdcCount(raw).milliamps::<P>();
        v as i32
    })
}

fn nearest(full_scale: u16, target: i32, convert: impl Fn(u16) -> i32) -> u16 {
    let mut best = 0u16;
    let mut best_err = i32::MAX;
    for raw in 0..=full_scale {
        let err = (convert(raw) - target).abs();
        if err < best_err {
            best = raw;
            best_err = err;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use rm32::tuning::Am32Tuning;

    type Sim = SimPlatform<Am32Tuning>;

    #[test]
    fn adc_model_is_ten_bit_three_volt() {
        // 11:1 divider: full scale 3.0 V at the pin is 33 V on the bus.
        assert_eq!(raw_for_centivolts::<Sim>(9999), 1023);
        // 12.00 V -> 0.3636 V at the pin -> 372.3 counts; 372 reads 11.99 V.
        let raw = raw_for_centivolts::<Sim>(1200);
        assert_eq!(raw, 372);
        assert_eq!(AdcCount(raw).centivolts::<Sim>(), CentiVolts(1199));
        // One count is ~3.2 cV at 11:1 (vs ~0.9 cV on the 12-bit 3.3 V model).
        let step = AdcCount(373).centivolts::<Sim>().0 - AdcCount(372).centivolts::<Sim>().0;
        assert!((3..=4).contains(&step), "step {step}");
    }

    #[test]
    fn current_model_round_trips_within_one_count() {
        // Full scale is 1023 * 293 / 20 = 14.99 A at 20 mV/A.
        assert_eq!(AdcCount(1023).milliamps::<Sim>(), MilliAmps(14986));
        for ma in [0i16, 500, 1000, 5000, 14000] {
            let raw = raw_for_milliamps::<Sim>(ma);
            let MilliAmps(back) = AdcCount(raw).milliamps::<Sim>();
            // One count is 293 / 20 = 14.6 mA at 20 mV/A.
            assert!((back - ma).abs() <= 8, "{ma} mA -> raw {raw} -> {back}");
        }
    }
}
