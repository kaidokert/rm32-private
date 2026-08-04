#![no_std]

pub mod adc_sync;
pub mod am32_timers;
pub mod bb;
pub mod board_init;
pub mod comp2;
pub mod current_adc;
pub mod iwdg;
#[cfg(feature = "monitor")]
pub mod monitor;
pub mod panic;
pub mod priority;
pub mod spin;
pub mod tim1_motor_pwm;
pub mod tim6_loop;
pub mod uart_tx;
pub mod usart2_rx;

pub use stm32l4xx_hal as hal;

/// The concrete AM32-clone motor bundle for this platform: zero-sized
/// register impls statically dispatched through the rm32-verbatim
/// `minz_core::am32_hal::MotorHal` trait. The program
/// (`examples/am32_clone.rs`) builds instances via its `motor()` wiring
/// constructor (the `&mut self` rm32 seams — pwm/phase/timers/comp —
/// are held by value, so a shared `static` cannot serve them). `Tim1Pwm`
/// fills BOTH the `PwmOutput` and `PhaseOutput` slots, `Am32Timers`
/// both timer slots (rm32-L431 shape: one TIM1 owns both roles).
pub type Am32Motor = minz_core::am32_hal::Motor<
    tim1_motor_pwm::Tim1Pwm,
    comp2::Comp2,
    tim1_motor_pwm::Tim1Pwm,
    am32_timers::Am32Timers,
    am32_timers::Am32Timers,
>;

/// The minz-owned observer bundle (bb/cs/adc/lt) — the seams the rm32
/// `MotorHal` knows nothing about. Built by the program's `observer()`
/// wiring constructor over its `static BB` etc.
pub type Am32Observer<'a> =
    minz_core::am32_hal::Observer<'a, bb::Bb, bb::CortexCs, adc_sync::InjAdc1, tim6_loop::Tim6Loop>;

use fugit::HertzU32 as Hertz;

/// CPU / DWT cycle-counter rate. Must match `rcc.cfgr.sysclk` for this board.
/// Pass directly to `rcc.cfgr.sysclk(...)`; use `.raw()` for u32 arithmetic.
pub const SYSCLK: Hertz = Hertz::MHz(80);

/// Motor PWM carrier on TIM1 (AM32 default for this ESC).
///
/// 48 kHz was implemented and bench-tested (2026-07-07) to halve the
/// ZC-confirm quantum (42 → 21 µs): the ADC trigger had to move to
/// 88 ticks (a 0.6 µs trigger sampled ch9 during dead-time — phase
/// flatlined 0) and the software blank had to shrink from 20 µs
/// (which covers an entire 48 kHz period — loop self-blinds). After
/// both fixes CL engaged and validated, but the envelope DROPPED
/// (~260 Hz vs 500 Hz at 24 kHz): doubling the carrier doubles
/// noise-edge density per window (~1.8/cycle vs 0.9) and candidate
/// churn starves the confirm pipeline. A real 48 kHz campaign needs
/// a candidate-hold policy redesign and likely the HEDGEHOG caps.
// 2026-07-13 carrier A/B (t1u counter live, all fixes in): 24 kHz
// reclaims real headroom (cpu 75→57 %, tim1_up=24000 exact, zero
// true losses, old ~650 Hz ceiling GONE — 1,230 Hz at amp 44) but
// the SPIKE EVENTS get 4.5× more frequent and 2.7× deeper at the
// same amp (6.4/s, 7.3 A, 5.0 V bus dips, audible chops; envelope
// dies 44→46 vs 50→55). Events scale with carrier ripple, NOT with
// CPU/ISR health — the decisive decoupling.
//
// STAYING at 24 kHz for the event investigation (operator call):
// CPU headroom is out of the picture as a confound, and the
// amplified event rate/depth makes the analog autopsy faster and
// clearer. 48 kHz remains the performance recipe once the event
// mechanism is fixed.
//
// 2026-07-15: 48 kHz RE-INSTANCED behind the `pwm48` cargo feature
// for the post-audit carrier re-qualification (the spike-mechanism
// fixes, the constants-audit E-ladder, and the engage-lottery fix
// all landed since the last 48 kHz run). Default stays 24 kHz.
#[cfg(feature = "pwm48")]
pub const PWM_FREQUENCY_HZ: u32 = 48_000;
#[cfg(not(feature = "pwm48"))]
pub const PWM_FREQUENCY_HZ: u32 = 24_000;

/// TIM1 ARR = SYSCLK / PWM_FREQUENCY_HZ − 1.
pub const TIM1_AUTORELOAD: u16 = (SYSCLK.raw() / PWM_FREQUENCY_HZ - 1) as u16;

/// TIM1 dead-time generator ticks (board YAML `dead_time`). DTG
/// counts CK_INT (80 MHz) periods, so the ns value is carrier-
/// independent — no change for 48 kHz.
pub const TIM1_DEAD_TIME: u8 = 45;

/// TIM1 CCR4 — TRGO sample point for ADC (AM32 uses 0x64). Hard
/// floor learned at 48 kHz: the trigger must clear dead-time
/// (562 ns) + gate-driver propagation + FET turn-on ≈ 1.0 µs, or
/// the first channel samples a phase node that hasn't risen yet
/// (flatlined 0 in the 48 kHz WAXWING).
pub const TIM1_CCR4_TRGO: u16 = 0x64;
