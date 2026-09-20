//! Power-stage static test for bench metering: all three phases at 50%
//! complementary PWM, indefinitely. Identical duties = zero line-line
//! voltage = zero motor current -- safe to leave running while probing.
//!
//! Meter targets while this runs:
//!   TP7 (VCC)  ~10-11.5 V   gate-drive rail from internal regulator
//!   TP5 (BOOT) ~VM+VCC      charge-pump rail; ~VM or less = pump dead
//!   Phase screw terminals   avg ~VM/2 vs GND if high sides switch
//!
//! VPH telemetry ~1100 mV = high sides switching; ~0 mV = low-only.
//!
//! Run: `cargo run --release --example stage-test`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::analog::adc::{Adc, Channel, Precision, SampleTime};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::timer::pwm::PwmExt;

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
struct ChIs;
impl Channel<Adc> for ChIs {
    type ID = u8;
    fn channel() -> u8 {
        15
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
    let _is = gpiob.pb11.into_analog();
    let mut nflt = gpioa.pa6.into_floating_input();

    // Open-drain EN on the shared EN/nFLT node (JP1 1-2), STBY high.
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    let mut adc = dp.ADC.constrain(&mut rcc);
    adc.set_sample_time(SampleTime::T_80);
    adc.set_precision(Precision::B_12);
    delay.delay(20.micros());
    adc.calibrate();

    let pwm = dp.TIM1.pwm(24_000.Hz(), &mut rcc);
    let mut ch1 = pwm.bind_pin(gpioa.pa8);
    let mut ch2 = pwm.bind_pin(gpioa.pa9);
    let mut ch3 = pwm.bind_pin(gpioa.pa10);
    let max_duty = ch1.get_max_duty();

    let _ = (gpiod.pd3, gpiod.pd4, gpioa.pa7);
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| w.moder7().alternate());
        pa.afrl().modify(|_, w| w.afr(7).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }
    let tim = unsafe { &*stm32::TIM1::ptr() };
    tim.ccer().modify(|_, w| {
        w.cc1e()
            .set_bit()
            .cc2e()
            .set_bit()
            .cc3e()
            .set_bit()
            .cc1ne()
            .set_bit()
            .cc2ne()
            .set_bit()
            .cc3ne()
            .set_bit()
    });
    tim.bdtr()
        .modify(|_, w| unsafe { w.dtg().bits(26).ossi().set_bit().ossr().set_bit() });

    // Release EN, enable outputs, precharge, then park all at 50%.
    en.set_high().ok();
    delay.delay(5.millis());
    tim.bdtr().modify(|_, w| w.moe().set_bit());
    ch1.set_duty(0);
    ch2.set_duty(0);
    ch3.set_duty(0);
    delay.delay(30.millis());
    let half = max_duty / 2;
    ch1.set_duty(half);
    ch2.set_duty(half);
    ch3.set_duty(half);

    rprintln!(
        "stage-test: all phases 50% ({}/{}) continuous. Meter TP5/TP7 now.",
        half,
        max_duty
    );
    loop {
        delay.delay(1000.millis());
        let v1 = adc.read_voltage(&mut ChVph1).unwrap();
        let v2 = adc.read_voltage(&mut ChVph2).unwrap();
        let v3 = adc.read_voltage(&mut ChVph3).unwrap();
        let is = adc.read_voltage(&mut ChIs).unwrap();
        rprintln!(
            "vph={}/{}/{} mV (want ~1100) IS={} mV en/nflt_node={}",
            v1,
            v2,
            v3,
            is,
            if nflt.is_high().unwrap() { "hi" } else { "LOW" }
        );
    }
}
