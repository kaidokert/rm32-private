#![no_std]
#![no_main]

use cortex_m_rt::entry;
use minz::hal::time::MonoTimer;
use minz::hal::{self, prelude::*};
use minz::open_loop::{self, OPEN_LOOP_STEP_CYCLES, Waveform};
use minz::tim1_motor_pwm::{self, max_duty};
use minz::{SYSCLK_HZ, panic};
use rtt_target::rprintln;

// PB1  = LIN1  (TIM1_CH3N)   phase C low
// PA10 = HIN1  (TIM1_CH3)    phase C high
// PB0  = LIN2  (TIM1_CH2N)   phase B low
// PA9  = HIN2  (TIM1_CH2)    phase B high
// PA7  = LIN3  (TIM1_CH1N)   phase A low
// PA8  = HIN3  (TIM1_CH1)    phase A high

/// Peak-to-peak swing as % of ARR (centered sine). Start low — open-loop slip heats fast.
const AMPLITUDE_PCT: u16 = 12;

#[entry]
fn main() -> ! {
    let mut cp = cortex_m::Peripherals::take().unwrap();
    let dp = hal::stm32::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let mut pwr = dp.PWR.constrain(&mut rcc.apb1r1);
    let clocks = rcc
        .cfgr
        .sysclk(SYSCLK_HZ.Hz())
        .freeze(&mut flash.acr, &mut pwr);

    panic::ensure_rtt();
    rprintln!(
        "open-loop: {} MHz, TIM1 {} kHz, {} Hz elec, sine 120°",
        SYSCLK_HZ / 1_000_000,
        minz::PWM_FREQUENCY_HZ / 1000,
        open_loop::OPEN_LOOP_ELECTRICAL_HZ,
    );

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb2);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb2);
    let _lin1 = gpiob
        .pb1
        .into_alternate::<1>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    let _hin1 =
        gpioa
            .pa10
            .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
    let _lin2 = gpiob
        .pb0
        .into_alternate::<1>(&mut gpiob.moder, &mut gpiob.otyper, &mut gpiob.afrl);
    let _hin2 = gpioa
        .pa9
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);
    let _lin3 = gpioa
        .pa7
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrl);
    let _hin3 = gpioa
        .pa8
        .into_alternate::<1>(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);

    tim1_motor_pwm::init(dp.TIM1, &mut rcc.apb2);

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
