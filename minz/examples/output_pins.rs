#![no_std]
#![no_main]

use cortex_m_rt::entry;
use minz::SYSCLK;
use minz::board_init::{BoardInit, configure_motor_pwm_pins, init};
use minz::hal;
use minz::hal::prelude::*;
use minz::hal::time::MonoTimer;
use minz::open_loop::{self, OPEN_LOOP_STEP_CYCLES, Waveform};
use minz::tim1_motor_pwm::{self, max_duty};
use rtt_target::rprintln;

/// Peak-to-peak swing as % of ARR (centered sine).
///
/// Open-loop sine has no rotor sync and no current limit, so the applied
/// voltage divides across the milliohm winding resistance and dumps into
/// I²R heat (cost us one motor on this bench at 12% / 14 V). 8 % at 5 V
/// bench supply ≈ 0.4 V / 0.05 Ω ≈ 8 A peak per phase — already plenty
/// for an open-loop demo. Bump it back up only when the bench supply is
/// turned down.
const AMPLITUDE_PCT: u16 = 8;

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = hal::stm32::Peripherals::take().unwrap();
    let BoardInit {
        mut cp,
        clocks,
        mut ahb2,
        mut apb2,
        ..
    } = init(cp, dp.FLASH, dp.RCC, dp.PWR);
    rprintln!(
        "open-loop: {} MHz, TIM1 {} kHz, {} Hz elec, sine 120°",
        SYSCLK.to_MHz(),
        minz::PWM_FREQUENCY_HZ / 1000,
        open_loop::OPEN_LOOP_ELECTRICAL_HZ,
    );

    let mut gpioa = dp.GPIOA.split(&mut ahb2);
    let mut gpiob = dp.GPIOB.split(&mut ahb2);
    configure_motor_pwm_pins(
        gpioa.pa7,
        gpioa.pa8,
        gpioa.pa9,
        gpioa.pa10,
        gpiob.pb0,
        gpiob.pb1,
        &mut gpioa.moder,
        &mut gpioa.otyper,
        &mut gpioa.afrl,
        &mut gpioa.afrh,
        &mut gpiob.moder,
        &mut gpiob.otyper,
        &mut gpiob.afrl,
    );
    tim1_motor_pwm::init(dp.TIM1, &mut apb2);

    cp.DCB.enable_trace();
    let mono = MonoTimer::new(cp.DWT, clocks);

    let arr = max_duty();
    let waveform = Waveform::Sine;
    let mut angle: u16 = 0;
    let mut rev: u32 = 0;

    loop {
        let (c1, c2, c3) = open_loop::duties(waveform, angle, arr, AMPLITUDE_PCT);
        // TIM1 CH1/2/3 = phases A/B/C (PA8, PA9, PA10 high sides)
        tim1_motor_pwm::set_duties(c1, c2, c3);

        let start = mono.now();
        while start.elapsed() < OPEN_LOOP_STEP_CYCLES {
            cortex_m::asm::nop();
        }

        let prev = angle;
        angle = open_loop::advance_angle(angle, true);
        if prev == 0 && angle == 359 {
            rev = rev.wrapping_add(1);
            rprintln!("elec rev {}", rev);
        }
    }
}
