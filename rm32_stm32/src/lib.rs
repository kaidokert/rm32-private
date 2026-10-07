//! RM32 STM32 HAL implementation.
//!
//! Supports STM32G071, STM32F051, STM32L431, STM32G431 via feature flags.
//! MCU-specific configuration is centralized in `mcu.rs`.

#![no_std]

/// Bench-debug print: always RTT-prints, and when `feature = "debuguart"`
/// is enabled, also pushes the same text out USART1 (PB6 on L431). Gives a
/// persistent log that survives panics/resets via a tail'd USB-serial port.
///
/// One formatting pass feeds both sinks ([`dline`]). It does NOT use
/// `rtt_target::rprintln!`: rtt-target 0.5 runs the whole `core::fmt`
/// formatting inside `critical_section::with`, i.e. with EVERY interrupt
/// masked, priority-0 COMP / commutation included. On the G071 (M0+) a
/// ~330-character status line held that mask for ~317 us (binz 2026-10-03,
/// TIM6 tick-gap instrument: 2-3 lost control ticks per 10 s at idle, from
/// the `[loop]` heartbeat alone).
#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {{
        $crate::dline(format_args!($($arg)*));
    }};
}

/// [`dprintln!`]'s body: format OUTSIDE any critical section; each
/// already-formatted fragment (a literal piece or one formatted value) goes
/// to RTT under its own short critical section (a buffer copy), and to the
/// debug UART when that feature is on. Then the newline.
pub fn dline(args: core::fmt::Arguments) {
    struct Sinks;
    impl core::fmt::Write for Sinks {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            fragment(s);
            Ok(())
        }
    }
    fn fragment(s: &str) {
        rtt_target::print_impl::write_str(0, s);
        #[cfg(feature = "debuguart")]
        debug_uart::write_str(s);
    }
    use core::fmt::Write as _;
    let _ = Sinks.write_fmt(args);
    fragment("\n");
}

pub mod mcu;
pub use mcu::pac;

// --- Shared across all MCUs (zero cfg) ---
pub mod adc_generic;
pub mod adc_hal;
#[cfg(any(feature = "stm32l431", feature = "stm32g431", feature = "stm32g071"))]
pub mod bench_clock;
pub mod board_ext;
// Bench safety guard (vbat-sag + overcurrent kill). Timing source is
// bench_clock (DWT.CYCCNT on M4, TIM6 tick count on G071).
#[cfg(any(feature = "stm32l431", feature = "stm32g431", feature = "stm32g071"))]
pub mod bench_guard;
// Bench UART throttle input: L431 = USART2 RX on PA2; G071 (binz bench)
// = USART3 RX on PC11 (mcu_g071::bench_uart, same API).
#[cfg(not(feature = "stm32g071"))]
pub mod bench_uart;
#[cfg(all(feature = "stm32g071", feature = "benchuart"))]
pub use mcu_g071::bench_uart;
#[cfg(all(
    feature = "benchuart",
    not(any(feature = "stm32l431", feature = "stm32g071"))
))]
compile_error!(
    "feature `benchuart` needs bench wiring: L431 (USART2/PA2) or G071 binz (USART3/PC11)"
);
// Blackbox firmware adapter (DWT timestamps + critical-section ring).
pub mod bench_bb;
// ISR-duration histograms (constant-cost distribution capture).
pub mod bench_hist;
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
pub mod comp_gate;
pub mod comp_hal;
pub mod comparator;
#[cfg(feature = "debuguart")]
pub mod dbg_frame_history;
#[cfg(feature = "debuguart")]
pub mod debug_uart;
#[cfg(all(feature = "benchuart", feature = "stm32g071"))]
pub mod sector_hist;
#[cfg(all(
    feature = "debuguart",
    not(any(feature = "stm32l431", feature = "stm32g071"))
))]
compile_error!("feature `debuguart` needs a backend: L431 (USART1/PB6) or G071 binz (USART3/PC10)");
#[cfg(all(feature = "zctrace", not(feature = "stm32l431")))]
compile_error!("feature `zctrace` is L431-only (COMP level probe uses L431 registers)");
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
#[cfg(feature = "debuguart")]
pub mod softuart;
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
