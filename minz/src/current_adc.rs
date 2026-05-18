//! Bench instrumentation ADC on ADC1: PA3 (IN8) supply current,
//! PA6 (IN11) battery voltage. Oneshot reads via the HAL's `ADC::read`
//! impl of `embedded_hal::adc::OneShot` — same pattern as
//! `stm32l4xx-hal/examples/adc.rs`. Each call blocks for ~16 µs at
//! the HAL's default sample time; that's invisible at our 24 kHz PWM
//! and the signals are slow rails anyway, so no need for DMA / free
//! running / hardware oversampling.

use crate::hal::adc::{ADC, SampleTime};
use crate::hal::delay::DelayCM;
use crate::hal::gpio::Analog;
use crate::hal::gpio::gpioa::{PA3, PA6};
use crate::hal::prelude::*;
use crate::hal::rcc::{AHB2, CCIPR, Clocks};
use crate::hal::stm32::{ADC_COMMON, ADC1};

pub struct SenseAdc {
    adc: ADC,
    pa3: PA3<Analog>,
    pa6: PA6<Analog>,
}

impl SenseAdc {
    pub fn new(
        adc1: ADC1,
        adc_common: ADC_COMMON,
        pa3: PA3<Analog>,
        pa6: PA6<Analog>,
        ahb2: &mut AHB2,
        ccipr: &mut CCIPR,
        clocks: Clocks,
    ) -> Self {
        let mut delay = DelayCM::new(clocks);
        let mut adc = ADC::new(adc1, adc_common, ahb2, ccipr, &mut delay);
        // Longest sample time the chip offers (~8 µs at 80 MHz ADC
        // clock). HAL default is Cycles2_5 = ~31 ns — far too short
        // for the vbat divider's ~3.2 kΩ source impedance, which
        // needs roughly 80+ ADC cycles to charge the sample cap to
        // within 12-bit accuracy. Both channels share this setting
        // since `OneShot::read` uses `self.sample_time`.
        adc.set_sample_time(SampleTime::Cycles640_5);
        Self { adc, pa3, pa6 }
    }

    pub fn isns_raw(&mut self) -> u16 {
        self.adc.read(&mut self.pa3).unwrap()
    }

    pub fn vbat_raw(&mut self) -> u16 {
        self.adc.read(&mut self.pa6).unwrap()
    }

    /// Convert a raw 12-bit sample to millivolts using the VREF-
    /// calibrated VDDA captured during `ADC::new()`. Doesn't trigger
    /// a new conversion — pass the value returned by `isns_raw` /
    /// `vbat_raw` if you already have it.
    pub fn adc_to_mv(&self, raw: u16) -> u16 {
        self.adc.to_millivolts(raw)
    }
}
