//! Instrument validation: the HARDWARE-TRIGGERED ADC (TIM1 TRGO2 -> mid-ON).
//!
//! Proves the trigger fires inside the PWM-ON window BEFORE the closed-loop
//! relies on it. Energizes a known six-step vector and probes each phase at
//! the trigger instant via the triggered ADC (mzhal::seed_step retargets the
//! DMA slot, floating_mv() reads it). Expected, per step, at DUTY:
//!   HIGH phase  ~= VM_pin (~670 mV)   <- high-side conducting => trigger in ON
//!   LOW  phase  ~= 0                  <- low-side to GND
//!   FLOAT phase ~= VM/2 (~375 mV)     <- star point, rotor stationary
//! If HIGH reads ~0 instead of ~VM, the trigger is landing in the OFF window.
//!
//! Low duty, brief energize per step, stage-safed on exit. Rotor may twitch.
//! Run: `cargo run --release --example adc-trig`

#![no_std]
#![no_main]

use binz::{mzhal, stage};
use cortex_m_rt::entry;
use rtt_target::{rprintln, rtt_init_print};
use stm32g0xx_hal::prelude::*;
use stm32g0xx_hal::rcc::{Config, RccExt};
use stm32g0xx_hal::stm32;

// step -> (high phase, low phase, float phase), phases 0=A 1=B 2=C.
const STEPS: [(u8, u8, u8); 6] = [
    (0, 1, 2),
    (0, 2, 1),
    (1, 2, 0),
    (1, 0, 2),
    (2, 0, 1),
    (2, 1, 0),
];

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

    let _ = (
        gpioa.pa1.into_analog(),
        gpiob.pb0.into_analog(),
        gpiob.pb1.into_analog(),
        gpiob.pb2.into_analog(),
        gpiob.pb11.into_analog(),
    );
    let _nflt = gpioa.pa6.into_floating_input();
    let mut en = gpioc.pc9.into_open_drain_output();
    let mut stby = gpioc.pc8.into_push_pull_output();
    en.set_low().ok();
    stby.set_high().ok();

    let _ = (
        gpioa.pa7, gpioa.pa8, gpioa.pa9, gpioa.pa10, gpiod.pd3, gpiod.pd4,
    );
    unsafe {
        let pa = &*stm32::GPIOA::ptr();
        pa.moder().modify(|_, w| {
            w.moder7()
                .alternate()
                .moder8()
                .alternate()
                .moder9()
                .alternate()
                .moder10()
                .alternate()
        });
        pa.afrl().modify(|_, w| w.afr(7).af2());
        pa.afrh()
            .modify(|_, w| w.afr(0).af2().afr(1).af2().afr(2).af2());
        let pd = &*stm32::GPIOD::ptr();
        pd.moder()
            .modify(|_, w| w.moder3().alternate().moder4().alternate());
        pd.afrl().modify(|_, w| w.afr(3).af2().afr(4).af2());
    }

    mzhal::tim1_init(); // sets OC4/TRGO2 trigger at ADC_TRIG_CNT
    mzhal::adc_init(); // triggered ADC + DMA
    mzhal::timers_init();

    // Let the ADC/DMA run a few periods (PWM idle, MOE off) and read VM.
    delay.delay(5.millis());
    let vm = mzhal::vm_mv();
    rprintln!(
        "adc-trig: VM(pin)={} mV, expected HIGH~={}, FLOAT~={}",
        vm,
        vm,
        vm / 2
    );
    if vm < 300 {
        rprintln!("VM low, aborting");
        loop {
            cortex_m::asm::nop();
        }
    }

    en.set_high().ok();
    delay.delay(2.millis());
    mzhal::moe(true);
    delay.delay(20.millis());

    // 20% duty so the ON window comfortably holds the triggered conversions.
    let ccr: u16 = (mzhal::ARR as u16 + 1) / 5;

    let names = ['A', 'B', 'C'];
    for step in 1u8..=6 {
        let (hi, lo, fl) = STEPS[(step - 1) as usize];
        mzhal::drive_step(step, ccr); // energize this vector
        delay.delay(15.millis()); // settle + let DMA fill

        // All three phases are in the fixed triggered scan — read them at the
        // trigger instant directly.
        let mv = [mzhal::phase_mv(0), mzhal::phase_mv(1), mzhal::phase_mv(2)];
        rprintln!(
            "step{} HIGH={}({}={}mV) LOW={}({}={}mV) FLOAT={}({}={}mV)",
            step,
            names[hi as usize],
            hi,
            mv[hi as usize],
            names[lo as usize],
            lo,
            mv[lo as usize],
            names[fl as usize],
            fl,
            mv[fl as usize],
        );

        // Verdict: HIGH must be near VM (=> ON-window trigger), LOW near 0.
        let hi_ok = mv[hi as usize] > vm * 7 / 10;
        let lo_ok = mv[lo as usize] < vm * 3 / 10;
        rprintln!(
            "      -> {} (HIGH~VM {}, LOW~0 {})",
            if hi_ok && lo_ok {
                "PASS: trigger is mid-ON"
            } else {
                "FAIL: check ON-window timing"
            },
            hi_ok,
            lo_ok,
        );
    }

    stage::force_safe();
    rprintln!("adc-trig: done, stage safed");
    loop {
        cortex_m::asm::nop();
    }
}
