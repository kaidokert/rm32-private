//! EVLDRIVE102H stack-detection probe -- strictly PASSIVE. Reads every
//! sense line the drive board exposes on the G071 solder-bridge config
//! (JP13-JP19 at 2-3) without ever touching INH/INL/EN/STBY, so the gate
//! driver stays in its hardware default state.
//!
//! Signals read: BUS=PA1 (VM/18.4 divider), TEMP=PC4 (NTC divider off the
//! board's 3V3), IS=PB11 (shunt amp out), VPH1/2/3=PB1/PB0/PB2 (phase
//! dividers ~1:5.3), Halls H1/H2/H3=PA15/PB3/PB10 (4.7k pull-ups to board
//! 3V3), nFLT=PA6.
//!
//! Interpretation: Halls high + TEMP mid-scale => board 3V3 alive (VM
//! powered or JP12 at 2-3). All ~0 with Halls low => board seated but VM
//! unpowered with JP12 default. BUS mV * 18.4 ~= VM.
//!
//! Run: `cargo run --release --example drive-detect`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

macro_rules! adc_ch {
    ($name:ident, $ch:expr) => {
        struct $name;
        impl Channel<Adc> for $name {
            type ID = u8;
            fn channel() -> u8 {
                $ch
            }
        }
    };
}
adc_ch!(ChBus, 1); // PA1  = ADC_IN1
adc_ch!(ChVph2, 8); // PB0  = ADC_IN8
adc_ch!(ChVph1, 9); // PB1  = ADC_IN9
adc_ch!(ChVph3, 10); // PB2  = ADC_IN10
adc_ch!(ChIs, 15); // PB11 = ADC_IN15
adc_ch!(ChTemp, 17); // PC4  = ADC_IN17

/// ln(x) for x in ~(0.05, 20) via atanh series -- plenty for NTC math.
fn ln_approx(x: f32) -> f32 {
    let y = (x - 1.0) / (x + 1.0);
    let y2 = y * y;
    2.0 * y * (1.0 + y2 / 3.0 + y2 * y2 / 5.0 + y2 * y2 * y2 / 7.0)
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

    // Analog inputs (also the reset default -- explicit for clarity).
    let _bus = gpioa.pa1.into_analog();
    let _vph2 = gpiob.pb0.into_analog();
    let _vph1 = gpiob.pb1.into_analog();
    let _vph3 = gpiob.pb2.into_analog();
    let _is = gpiob.pb11.into_analog();
    let _temp = gpioc.pc4.into_analog();

    // Digital senses: floating -- the drive board provides the pull-ups.
    let mut h1 = gpioa.pa15.into_floating_input();
    let mut h2 = gpiob.pb3.into_floating_input();
    let mut h3 = gpiob.pb10.into_floating_input();
    let mut nflt = gpioa.pa6.into_floating_input();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_160); // high-impedance dividers
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    rprintln!("drive-detect: EVLDRIVE102H passive sense sweep (nothing driven)");
    loop {
        let bus = adc.read_voltage(&mut ChBus).unwrap();
        let temp = adc.read_voltage(&mut ChTemp).unwrap();
        let is = adc.read_voltage(&mut ChIs).unwrap();
        let v1 = adc.read_voltage(&mut ChVph1).unwrap();
        let v2 = adc.read_voltage(&mut ChVph2).unwrap();
        let v3 = adc.read_voltage(&mut ChVph3).unwrap();
        let halls = (
            h1.is_high().unwrap(),
            h2.is_high().unwrap(),
            h3.is_high().unwrap(),
        );
        // Divider: R16 75k top, R17 4.3k + R20 220R bottom = 1:17.59
        // (meter-verified: 676 mV -> 11.85 V bus).
        let vm_mv = bus as u32 * 1759 / 100;

        // NTC (10k, B~3435) on top, R26 1.2k + R29 91R bottom, from 3V3:
        // R_ntc = 1.291k * (3300/V - 1); T from the B-parameter equation.
        let temp_c: i32 = if temp > 50 {
            let r_ntc = 1.291 * (3300.0 / temp as f32 - 1.0); // in kohm
            let ln_ratio = ln_approx(r_ntc / 10.0);
            (3435.0 / (ln_ratio + 3435.0 / 298.15) - 273.15) as i32
        } else {
            i32::MIN // divider dead
        };

        rprintln!(
            "BUS={} mV (VM~{}.{:02} V) TEMP={} mV (~{} C) IS={} mV VPH={}/{}/{} mV halls={:?} nFLT={}",
            bus,
            vm_mv / 1000,
            (vm_mv % 1000) / 10,
            temp,
            temp_c,
            is,
            v1,
            v2,
            v3,
            halls,
            if nflt.is_high().unwrap() { "hi" } else { "LO" }
        );
        let board_3v3_alive = halls.0 && halls.1 && halls.2 && temp > 300;
        rprintln!(
            "  => board 3V3 {} | VM {}",
            if board_3v3_alive {
                "ALIVE"
            } else {
                "dead (VM off + JP12 default?)"
            },
            if bus > 250 { "POWERED" } else { "unpowered" }
        );
        delay.delay(2000.millis());
    }
}
