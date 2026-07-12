#![no_std]

pub mod a85;
pub mod adc_sync;
pub mod board_init;
pub mod comp2;
pub mod current_adc;
pub mod idle_loop;
pub mod iwdg;
pub mod lptim2_oneshot;
pub mod nvic;
pub mod open_loop;
pub mod panic;
pub mod priority;
pub mod softuart;
pub mod softuart_lptim1;
pub mod softuart_tim2;
pub mod tim1_motor_pwm;
pub mod tim7_drive;
pub mod timer_ext;
pub mod uart_tx;
pub mod usart2_rx;

pub use stm32l4xx_hal as hal;

use fugit::HertzU32 as Hertz;

/// CPU / DWT cycle-counter rate. Must match `rcc.cfgr.sysclk` for this board.
/// Pass directly to `rcc.cfgr.sysclk(...)`; use `.raw()` for u32 arithmetic.
pub const SYSCLK: Hertz = Hertz::MHz(80);

/// SysTick tick rate used across examples (10 µs / tick = 100 kHz). Drives
/// the soft-UART idle-gap detector among other things.
pub const SYSTICK: Hertz = Hertz::kHz(100);

/// One second of `SYSCLK` cycles (for DWT busy-waits).
pub const CYCLES_PER_SECOND: u32 = SYSCLK.raw();

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
pub const PWM_FREQUENCY_HZ: u32 = 48_000;

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

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
