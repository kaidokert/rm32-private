#![cfg_attr(not(test), no_std)]

pub mod idle_loop;

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
