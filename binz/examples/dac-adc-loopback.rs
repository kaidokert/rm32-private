//! DAC life-probe with on-pin ADC readback: DAC1 channel 1 drives PA4,
//! and the ADC samples the very same pin (ADC_IN4) -- a fully internal
//! loopback, no wiring needed.
//!
//! Life criteria: readback tracks the commanded staircase monotonically
//! within a few 10s of mV (DAC buffer offset + ADC noise).
//!
//! Run: `cargo run --release --example dac-adc-loopback`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::analog::dac::DacExt;
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

/// PA4 is owned by the DAC; this marker lets the ADC sample channel 4
/// (same pin) anyway. Pin is already in analog mode courtesy of the DAC.
struct Pa4Readback;
impl Channel<Adc> for Pa4Readback {
    type ID = u8;
    fn channel() -> u8 {
        4
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);
    let gpioa = dp.GPIOA.split(&mut rcc);

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    let dac0 = dp.DAC.constrain(gpioa.pa4, &mut rcc);
    let mut dac = dac0.calibrate_buffer(&mut delay).enable();

    let mut pa4 = Pa4Readback;
    rprintln!("dac-adc-loopback: staircase on PA4, ADC_IN4 readback");
    loop {
        let mut prev_mv = 0u16;
        let mut monotonic = true;
        for step in [0u16, 512, 1024, 2048, 3072, 3584, 4095] {
            dac.set_value(step);
            delay.delay(100.micros()); // DAC settle
            let mv = adc.read_voltage(&mut pa4).unwrap();
            let ideal = (step as u32 * 3300 / 4095) as u16;
            rprintln!("  dac={} -> {} mV (ideal ~{} mV)", step, mv, ideal);
            if step > 0 && mv <= prev_mv {
                monotonic = false;
            }
            prev_mv = mv;
        }
        rprintln!("staircase monotonic: {}", monotonic);
        delay.delay(2000.millis());
    }
}
