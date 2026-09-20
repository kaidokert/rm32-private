//! DMM-speed static walk of all six gate-drive signals. NO PWM: each
//! signal is a plain GPIO driven high alone for 4 s while everything else
//! stays low, looping forever. Disconnect the motor wires first (though
//! one-phase-at-a-time also has no current path).
//!
//! Expected at the phase screw terminal / OUTx while its step is active:
//!   INHx high -> OUTx ~= VM (~11.85 V), VPHx ~2230 mV internally
//!   INLx high -> OUTx ~= 0 V,           VPHx ~0 mV internally
//!   idle      -> OUTx floats (hi-Z)
//!
//! Probe map per step (G071 config): INH1=PA8/C10-23, INH2=PA9/C10-21,
//! INH3=PA10/C10-33 (3.3 V at the morpho pin while that step is active);
//! INL1=PA7/C10-15, INL2=PD3/C7-11, INL3=PD4/C7-15.
//!
//! Run: `cargo run --release --example pin-walk`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

struct ChVph1;
impl Channel<Adc> for ChVph1 {
    type ID = u8;
    fn channel() -> u8 {
        9
    }
}
struct ChVph2;
impl Channel<Adc> for ChVph2 {
    type ID = u8;
    fn channel() -> u8 {
        8
    }
}
struct ChVph3;
impl Channel<Adc> for ChVph3 {
    type ID = u8;
    fn channel() -> u8 {
        10
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
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    let _vph1 = gpiob.pb1.into_analog();
    let _vph2 = gpiob.pb0.into_analog();
    let _vph3 = gpiob.pb2.into_analog();
    let mut nflt = gpioa.pa6.into_floating_input();

    // All six drive signals as plain push-pull GPIO, low.
    let mut inh1 = gpioa.pa8.into_push_pull_output();
    let mut inh2 = gpioa.pa9.into_push_pull_output();
    let mut inh3 = gpioa.pa10.into_push_pull_output();
    let mut inl1 = gpioa.pa7.into_push_pull_output();
    let mut inl2 = gpiod.pd3.into_push_pull_output();
    let mut inl3 = gpiod.pd4.into_push_pull_output();
    inh1.set_low().ok();
    inh2.set_low().ok();
    inh3.set_low().ok();
    inl1.set_low().ok();
    inl2.set_low().ok();
    inl3.set_low().ok();

    // EN open-drain released (33k pull-up), STBY high.
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    stby.set_high().ok();
    en.set_high().ok();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    rprintln!("pin-walk: 4 s per step, loop: INH1,INL1,INH2,INL2,INH3,INL3");
    let mut cycle = 0u32;
    loop {
        cycle += 1;
        for step in 0..6u8 {
            // Activate exactly one signal.
            match step {
                0 => inh1.set_high().ok(),
                1 => inl1.set_high().ok(),
                2 => inh2.set_high().ok(),
                3 => inl2.set_high().ok(),
                4 => inh3.set_high().ok(),
                _ => inl3.set_high().ok(),
            };
            delay.delay(3500.millis());
            // Sample near the end of the window.
            let v1 = adc.read_voltage(&mut ChVph1).unwrap();
            let v2 = adc.read_voltage(&mut ChVph2).unwrap();
            let v3 = adc.read_voltage(&mut ChVph3).unwrap();
            let name = ["INH1", "INL1", "INH2", "INL2", "INH3", "INL3"][step as usize];
            rprintln!(
                "cycle {} step {}: vph={}/{}/{} mV nflt={}",
                cycle,
                name,
                v1,
                v2,
                v3,
                if nflt.is_high().unwrap() { "hi" } else { "LOW" }
            );
            match step {
                0 => inh1.set_low().ok(),
                1 => inl1.set_low().ok(),
                2 => inh2.set_low().ok(),
                3 => inl2.set_low().ok(),
                4 => inh3.set_low().ok(),
                _ => inl3.set_low().ok(),
            };
            delay.delay(500.millis());
        }
    }
}
