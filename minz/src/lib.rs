#![no_std]

pub mod board_init;
pub mod nvic;
pub mod open_loop;
pub mod panic;
pub mod softuart;
pub mod softuart_lptim1;
pub mod softuart_tim2;
pub mod tim1_motor_pwm;
pub mod timer_ext;

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
pub const PWM_FREQUENCY_HZ: u32 = 24_000;

/// TIM1 ARR = SYSCLK / PWM_FREQUENCY_HZ − 1.
pub const TIM1_AUTORELOAD: u16 = (SYSCLK.raw() / PWM_FREQUENCY_HZ - 1) as u16;

/// TIM1 dead-time generator ticks (board YAML `dead_time`).
pub const TIM1_DEAD_TIME: u8 = 45;

/// TIM1 CCR4 — TRGO sample point for ADC (AM32 uses 0x64).
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
