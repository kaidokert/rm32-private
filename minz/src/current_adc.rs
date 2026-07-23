//! ADC1 power-up + calibration via the HAL (`ADC::new` runs the
//! factory calibration sequence), consuming the analog sense pins
//! PA3 (IN8, supply current) and PA6 (IN11, battery voltage). The
//! running samples come from the injected group (`adc_sync`); this
//! type exists so the calibrated peripheral has an owner.

use crate::hal::adc::{ADC, SampleTime};
use crate::hal::delay::DelayCM;
use crate::hal::gpio::Analog;
use crate::hal::gpio::gpioa::{PA3, PA6};
use crate::hal::rcc::{AHB2, CCIPR, Clocks};
use crate::hal::stm32::{ADC_COMMON, ADC1};

/// Held only for ownership: keeps the calibrated ADC + analog pins
/// alive so nothing re-configures them. After `adc_sync::start` takes
/// over the registers, no method on this may run (see `adc_sync` docs);
/// the am32_clone binds it as `_sense` and never touches it again.
pub struct SenseAdc {
    #[allow(dead_code)]
    adc: ADC,
    #[allow(dead_code)]
    pa3: PA3<Analog>,
    #[allow(dead_code)]
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
}
