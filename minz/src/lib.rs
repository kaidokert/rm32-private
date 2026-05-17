#![no_std]

pub mod nvic;
pub mod open_loop;
pub mod panic;
pub mod softuart;
pub mod softuart_tim2;
pub mod tim1_motor_pwm;
pub mod timer_ext;

pub use stm32l4xx_hal as hal;

/// CPU / DWT cycle-counter rate. Must match `rcc.cfgr.sysclk` for this board.
pub const SYSCLK_HZ: u32 = 80_000_000;

/// One second of `SYSCLK_HZ` cycles (for DWT busy-waits).
pub const CYCLES_PER_SECOND: u32 = SYSCLK_HZ;

/// Motor PWM carrier on TIM1 (AM32 default for this ESC).
pub const PWM_FREQUENCY_HZ: u32 = 24_000;

/// TIM1 ARR = SYSCLK / PWM_FREQUENCY_HZ − 1.
pub const TIM1_AUTORELOAD: u16 = (SYSCLK_HZ / PWM_FREQUENCY_HZ - 1) as u16;

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
