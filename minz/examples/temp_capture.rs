//! Standalone internal-temperature-sensor capture (no motor, no adc_sync).
//!
//! Streams the L431 die-temperature ADC channel (IN17) over USART1/PB6 at
//! 2 Mbaud, one raw 12-bit count per line, at a fixed cadence. A banner line
//! carries the factory TS_CAL constants so the host can convert to °C.
//!
//! This is deliberately a SEPARATE binary from am32_clone: the clone's ADC1 is
//! fully owned by the motor injected group (adc_sync), which must not be
//! reconfigured. Here nothing else touches ADC1, so the temp sensor gets clean
//! sole use. Flash this for a temp capture, reflash the monitor build for motor
//! work.
//!
//! Host: scripts/temp_grab.py collects a window; ratch22-probe runs it through
//! the windowed feature evaluator (trend/spectrum/distribution/ACF/scalars).

#![no_std]
#![no_main]

use core::fmt::Write as _;

use cortex_m_rt::entry;

use minz::board_init::{BoardInit, init};
use minz::hal::adc::{ADC, SampleTime};
use minz::hal::delay::DelayCM;
use minz::hal::prelude::*;
use minz::hal::serial::{Config, Serial};
use minz::hal::signature::{VDDA_CALIB_MV, VtempCalHigh, VtempCalLow};
use minz::hal::stm32;
use minz::uart_tx::usart1_tx_push_pull_pb6;

use rtt_target::{rprintln, rtt_init_print};

const BAUD: u32 = 2_000_000;
/// Sample cadence. 20 Hz × a host window of 512 ≈ 25 s — long enough for a
/// finger-warm transient, dense enough for ACF/spectrum structure.
const SAMPLE_HZ: u32 = 20;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();
    let BoardInit {
        clocks,
        mut ahb2,
        mut apb2,
        mut ccipr,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);

    let mut gpiob = dp.GPIOB.split(&mut ahb2);

    // USART1 TX on PB6: HAL half-duplex open-drain init, then flip to
    // push-pull for clean 2 Mbaud (the open-drain pull-up caps ~115200).
    let mut usart_tx = gpiob.pb6.into_alternate_open_drain::<7>(
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );
    usart_tx.internal_pull_up(&mut gpiob.pupdr, true);
    let serial = Serial::usart1(
        dp.USART1,
        (usart_tx,),
        Config::default().baudrate(BAUD.bps()),
        clocks,
        &mut apb2,
    );
    let (mut tx, _) = serial.split();
    usart1_tx_push_pull_pb6();

    // ADC1 + internal temperature sensor (IN17). Longest sample time — the
    // sensor has a high source impedance and needs the charge window.
    let mut delay = DelayCM::new(clocks);
    let mut adc = ADC::new(dp.ADC1, dp.ADC_COMMON, &mut ahb2, &mut ccipr, &mut delay);
    adc.set_sample_time(SampleTime::Cycles640_5);
    let mut temp = adc.enable_temperature(&mut delay);

    // Factory calibration points (system memory): host converts raw→°C as
    //   °C = t1 + (t2−t1)·(raw·VDDA/VDDA_CALIB − cal1)/(cal2 − cal1)
    let cal1 = VtempCalLow::get().read();
    let cal2 = VtempCalHigh::get().read();
    let t1 = VtempCalLow::TEMP_DEGREES;
    let t2 = VtempCalHigh::TEMP_DEGREES;
    rprintln!("temp_capture: sysclk={} hz", clocks.sysclk().raw());

    // Reprint the calibration banner every N samples so a grabber that
    // connects mid-stream (after the boot banner scrolled past) still sees it.
    let period_ms = 1000 / SAMPLE_HZ;
    let mut n: u32 = 0;
    loop {
        if n % 100 == 0 {
            let _ = write!(
                tx,
                "TSCAL cal1={} cal2={} t1={} t2={} vdda_calib={}\r\n",
                cal1, cal2, t1, t2, VDDA_CALIB_MV,
            );
        }
        let raw: u16 = adc.read(&mut temp).unwrap();
        let _ = write!(tx, "T {}\r\n", raw);
        n = n.wrapping_add(1);
        delay.delay_ms(period_ms);
    }
}
