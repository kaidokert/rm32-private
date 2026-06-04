#![cfg_attr(not(test), no_std)]

pub mod accumulator;
pub mod accumulator_state;
pub mod algorithm_params;
pub mod ewma_pow2;
pub mod filter;
pub mod idle_loop;
pub mod more_atomic_pid;
pub mod pi_controller_state;
pub mod pll_controller;
pub mod pll_state;
pub mod signed_calc;

#[cfg(target_arch = "arm")]
pub use stm32g4xx_hal as hal;

#[cfg(target_arch = "arm")]
pub mod nvic;
#[cfg(target_arch = "arm")]
pub mod panic;
#[cfg(target_arch = "arm")]
pub mod priority;
#[cfg(target_arch = "arm")]
pub mod tim7_drive;
