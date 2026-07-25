//! RM32 STM32 HAL implementation.
//!
//! Supports STM32G071, STM32F051, STM32L431, STM32G431 via feature flags.
//! MCU-specific configuration is centralized in `mcu.rs`.

#![no_std]

/// Bench-debug print: always RTT-prints, and when `feature = "debuguart"`
/// is enabled, also pushes the same text out USART1 (PB6 on L431). Gives a
/// persistent log that survives panics/resets via a tail'd USB-serial port.
#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {{
        rtt_target::rprintln!($($arg)*);
        #[cfg(feature = "debuguart")]
        {
            use core::fmt::Write as _;
            let _ = writeln!($crate::debug_uart::DebugUart, $($arg)*);
        }
    }};
}

pub mod mcu;
pub use mcu::pac;

// --- Shared across all MCUs (zero cfg) ---
pub mod adc_generic;
pub mod adc_hal;
// Bench safety guard (vbat-sag + overcurrent kill). Timing source is
// DWT.CYCCNT — M4 targets only; M0/M0+ have no cycle counter.
#[cfg(any(feature = "stm32l431", feature = "stm32g431"))]
pub mod bench_guard;
// Bench UART throttle input (USART2 on PA2). L431-only.
pub mod bench_uart;
#[cfg(all(feature = "benchuart", not(feature = "stm32l431")))]
compile_error!("feature `benchuart` is L431-only (USART2-on-PA2 bench wiring)");
// Blackbox firmware adapter (DWT timestamps + critical-section ring).
pub mod bench_bb;
// ZC-trace firmware adapter (ring + batch state + enable toggle).
pub mod bench_zct;
// Edge/veto probe — per-window COMP entry/veto counters + TIM16 latency.
pub mod edge_probe;
#[cfg(all(
    feature = "blackbox",
    not(any(feature = "stm32l431", feature = "stm32g431"))
))]
compile_error!("feature `blackbox` needs DWT.CYCCNT (M4 targets: stm32l431/stm32g431)");
pub mod capture_generic;
pub mod capture_hal;
pub mod comp_hal;
pub mod comparator;
#[cfg(feature = "debuguart")]
pub mod dbg_frame_history;
#[cfg(feature = "debuguart")]
pub mod debug_uart;
pub mod dma_buf;
pub mod emergency;
pub mod flash;
pub mod gpio_pin;
pub mod gpio_regs;
pub mod init;
pub mod isr;
pub mod isr_handlers;
#[cfg(not(test))]
mod panic;
pub mod phase;
pub mod regs;
pub mod stub;
pub mod telem_hal;
pub mod timer;
pub mod ws2812_hal;

// --- MCU-specific peripheral modules (one cfg per chip) ---
#[cfg(feature = "stm32f051")]
pub mod mcu_f051;
#[cfg(feature = "stm32g071")]
pub mod mcu_g071;
#[cfg(feature = "stm32g431")]
pub mod mcu_g431;
#[cfg(feature = "stm32l431")]
pub mod mcu_l431;
