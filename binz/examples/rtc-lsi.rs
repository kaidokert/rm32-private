//! RTC life-probe on the LSI oscillator: set a known time, busy-wait ~4 s,
//! read back and verify the clock advanced.
//!
//! Run: `cargo run --release --example rtc-lsi`

#![no_std]
#![no_main]

use binz as _; // panic handler
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::rtc::RtcExt;
use stm32g0xx_hal::stm32;
use stm32g0xx_hal::time::Time;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.freeze(Config::pll());
    let mut delay = cp.SYST.delay(&mut rcc);

    let mut rtc = dp.RTC.constrain(&mut rcc); // LSI source
    rtc.set_time(&Time {
        hours: 12,
        minutes: 0,
        seconds: 0,
        daylight_savings: false,
    });

    let t0 = rtc.get_time();
    rprintln!(
        "rtc-lsi: set 12:00:00, read back {:02}:{:02}:{:02}",
        t0.hours,
        t0.minutes,
        t0.seconds
    );

    delay.delay(4000.millis());

    let t1 = rtc.get_time();
    let advanced = t1.seconds >= 3 && t1.seconds <= 6;
    rprintln!(
        "after ~4 s wall: {:02}:{:02}:{:02} -> RTC {}",
        t1.hours,
        t1.minutes,
        t1.seconds,
        if advanced {
            "ALIVE - PASS"
        } else {
            "FAIL (no/wrong advance)"
        }
    );

    loop {
        delay.delay(5000.millis());
        let t = rtc.get_time();
        rprintln!("tick {:02}:{:02}:{:02}", t.hours, t.minutes, t.seconds);
    }
}
