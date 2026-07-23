//! RTT panic logging for firmware binaries using this crate.

use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};
use rtt_target::rprintln;

static RTT_INIT: AtomicBool = AtomicBool::new(false);

/// Initialize the RTT print channel once (`rtt_init_print!` must not be duplicated).
pub fn ensure_rtt() {
    if RTT_INIT.swap(true, Ordering::Acquire) {
        return;
    }
    rtt_target::rtt_init_print!();
}

/// Raw blocking USART1 write for panic context: poll TXE, no ISRs,
/// no HAL state - works from any wedged context once board_init has
/// run. The bench IWDG turns an RTT-only panic into a SILENT reboot
/// (tautopsy1/4 incidents: two crashes, zero evidence on the wire).
fn uart_raw(bytes: &[u8]) {
    // Safety: register-level poll writes; panic context is
    // single-threaded (interrupts disabled by caller).
    let usart1 = unsafe { &*crate::hal::pac::USART1::ptr() };
    for &b in bytes {
        let mut spins = 0u32;
        while usart1.isr.read().txe().bit_is_clear() {
            spins += 1;
            if spins > 1_000_000 {
                return; // UART dead - do not hang the report
            }
        }
        usart1.tdr.write(|w| unsafe { w.bits(b as u32) });
    }
}

struct UartFmt;
impl core::fmt::Write for UartFmt {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        uart_raw(s.as_bytes());
        Ok(())
    }
}

/// Log panic location and message over RTT + raw UART, mask
/// interrupts, then halt (the IWDG will reset; the message has
/// already left the wire).
pub fn halt(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    // Bridge OFF before reporting: a panic must not leave TIM1 driving
    // the motor for the ~1 s until the IWDG resets (MOE stays set
    // through a core halt; the last CCR/pin roles would keep the
    // bridge live under a wedged control loop). Raw pin-mode writes,
    // no state needed; harmless pre-clock-enable (writes to an
    // unclocked GPIO port are ignored).
    crate::tim1_motor_pwm::all_off();
    {
        use core::fmt::Write;
        let mut u = UartFmt;
        let _ = write!(u, "\r\n!! PANIC ");
        if let Some(loc) = info.location() {
            let _ = write!(u, "at {}:{}", loc.file(), loc.line());
        }
        let _ = write!(u, ": {}\r\n", info.message());
    }
    ensure_rtt();

    if let Some(loc) = info.location() {
        rprintln!("PANIC at {}:{}:{}", loc.file(), loc.line(), loc.column());
    } else {
        rprintln!("PANIC (no location)");
    }

    rprintln!("  {}", info.message());

    loop {
        cortex_m::asm::nop();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    halt(info)
}
