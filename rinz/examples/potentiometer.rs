#![no_std]
#![no_main]

use hal::adc::AdcCommonExt;
use hal::adc::{AdcClaim, config::SampleTime};
use hal::prelude::*;
use hal::pwr::PwrExt;
use hal::{rcc, stm32};
use rinz::hal;

use cortex_m_rt::entry;

#[entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();
    rtt_target::rprintln!("potentiometer start");

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain().freeze();
    let mut rcc = dp.RCC.freeze(rcc::Config::hsi(), pwr);

    let mut delay = cp.SYST.delay(&rcc.clocks);

    let adc12_common = dp.ADC12_COMMON.claim(Default::default(), &mut rcc);
    let mut adc = adc12_common.claim_and_configure(
        dp.ADC1,
        hal::adc::config::AdcConfig::default(),
        &mut delay,
    );

    // PB12 = potentiometer (ADC1 channel 11)
    let gpiob = dp.GPIOB.split(&mut rcc);
    let pb12 = gpiob.pb12.into_analog();

    loop {
        let sample = adc.convert(&pb12, SampleTime::Cycles_640_5);
        let mv = adc.sample_to_millivolts(sample);
        rtt_target::rprintln!("pot: {}mV (raw {})", mv, sample);
        delay.delay_ms(200u32);
    }
}
