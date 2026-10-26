//! The STM32 port's `rm32::platform::Platform`: clocks derived from the
//! active chip's constants, board constants from the board yaml.

use crate::mcu::{Chip, ChipConfig};

mod board {
    include!(concat!(env!("OUT_DIR"), "/board_platform.rs"));
}

/// Zero-sized platform marker for the active STM32 chip.
pub struct StmPlatform;

impl rm32::platform::Platform for StmPlatform {
    /// TIM6 update rate. Nominal: the L431 runs PSC=79/ARR=50 (19.6 kHz),
    /// exactly as AM32 does with `LOOP_FREQUENCY_HZ` = 20000.
    const TICK_HZ: u32 = 20_000;
    /// Interval/commutation timer: CPU clock / (TIMER_PSC + 1).
    const TIMER_HZ: u32 = Chip::CPU_FREQUENCY_MHZ * 1_000_000 / (Chip::TIMER_PSC as u32 + 1);
    const VOLTAGE_DIVIDER_X100: u16 = board::VOLTAGE_DIVIDER_X100;
    const MILLIVOLT_PER_AMP: u16 = board::MILLIVOLT_PER_AMP;
    const CURRENT_OFFSET: i16 = board::CURRENT_OFFSET;
    const USE_NTC: bool = board::USE_NTC;
}
