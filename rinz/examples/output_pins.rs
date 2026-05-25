#![no_std]
#![no_main]

use cortex_m_rt::entry;
use hal::prelude::*;
use hal::pwm::PwmAdvExt;
use hal::pwr::PwrExt;
use hal::time::{ExtU32, RateExtU32};
use hal::{rcc, stm32};
use rinz::hal;

/// 48-step sine table centred at 127 (0 = full off, 254 = full on).
/// Phase B offset = 16 steps (120°), phase C = 32 steps (240°).
static SINE48: [u8; 48] = [
    127, 144, 160, 176, 191, 205, 217, 227, 237, 244, 250, 253, 254, 253, 250, 244, 237, 227, 217,
    205, 191, 176, 160, 144, 127, 110, 94, 79, 64, 50, 37, 27, 17, 10, 4, 1, 0, 1, 4, 10, 17, 27,
    37, 50, 64, 79, 94, 110,
];

/// Open-loop amplitude as fraction of max duty (0..255).
/// Keep this low without current limiting — winding heats fast.
const AMPLITUDE: u32 = 30;

#[entry]
fn main() -> ! {
    rinz::panic::ensure_rtt();
    rtt_target::rprintln!("output_pins start");

    let dp = stm32::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain().freeze();
    let mut rcc = dp.RCC.freeze(rcc::Config::hsi(), pwr);
    let mut delay = cp.SYST.delay(&rcc.clocks);

    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);

    // High-side: TIM1 CH1/2/3 on PA8/PA9/PA10 (AF6)
    let pa8 = gpioa.pa8.into_alternate::<6>();
    let pa9 = gpioa.pa9.into_alternate::<6>();
    let pa10 = gpioa.pa10.into_alternate::<6>();
    // Low-side: TIM1 CH1N/2N/3N on PC13 (AF4) / PA12 (AF6) / PB15 (AF4)
    let pc13 = gpioc.pc13.into_alternate::<4>();
    let pa12 = gpioa.pa12.into_alternate::<6>();
    let pb15 = gpiob.pb15.into_alternate::<4>();

    // 20 kHz center-aligned complementary PWM, 100 ns dead-time
    let (_ctrl, (c1, c2, c3)) = dp
        .TIM1
        .pwm_advanced((pa8, pa9, pa10), &mut rcc)
        .frequency(20_000u32.Hz())
        .with_deadtime(100u32.nanos())
        .center_aligned()
        .finalize();

    let mut c1 = c1.into_complementary(pc13);
    let mut c2 = c2.into_complementary(pa12);
    let mut c3 = c3.into_complementary(pb15);

    let max = c1.max_duty_cycle() as u32;
    c1.enable();
    c2.enable();
    c3.enable();

    rtt_target::rprintln!("TIM1 running, max_duty={}", max);

    let mut step: usize = 0;
    let mut revs: u32 = 0;

    loop {
        let a = SINE48[step] as u32;
        let b = SINE48[(step + 16) % 48] as u32;
        let c = SINE48[(step + 32) % 48] as u32;

        // Scale: centre at max/2, swing ±(AMPLITUDE/255 * max/2)
        let half = max / 2;
        let _ = c1.set_duty_cycle((half + (a * AMPLITUDE * half) / (255 * 128)) as u16);
        let _ = c2.set_duty_cycle((half + (b * AMPLITUDE * half) / (255 * 128)) as u16);
        let _ = c3.set_duty_cycle((half + (c * AMPLITUDE * half) / (255 * 128)) as u16);

        step += 1;
        if step >= 48 {
            step = 0;
            revs = revs.wrapping_add(1);
            rtt_target::rprintln!("elec rev {}", revs);
        }

        // 48 steps × 347 µs ≈ 60 Hz electrical
        delay.delay_us(347u32);
    }
}
