//! Internal ADC life-probe: temperature sensor, VREFINT (true VDDA-scaled
//! millivolts via the HAL's vref calibration), and VBAT/3 channel.
//!
//! Life criteria: temp plausible (15-45 C on a bench), VREFINT ~1212 mV,
//! VBAT ~= VDD (~3300 mV) since VBAT is tied to VDD on the Nucleo.
//!
//! Run: `cargo run --release --example adc-temp`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{OversamplingRatio, Precision, SampleTime, VBat, VRef};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    adc.set_oversampling_ratio(OversamplingRatio::X_16);
    adc.set_oversampling_shift(16);
    adc.oversampling_enable(true);
    delay.delay(20.micros()); // ADC voltage regulator settle
    adc.calibrate();

    let mut vref = VRef::new();
    vref.enable(&mut adc);
    let mut vbat = VBat::new();
    vbat.enable(&mut adc);

    rprintln!("adc-temp: probing internal channels @64 MHz");
    loop {
        let t = adc.read_temperature().unwrap();
        let vref_mv = adc.read_voltage(&mut vref).unwrap();
        let vbat_mv = adc.read_voltage(&mut vbat).unwrap() * 3;
        rprintln!("temp={} C  vrefint={} mV  vbat={} mV", t, vref_mv, vbat_mv);
        delay.delay(1000.millis());
    }
}
