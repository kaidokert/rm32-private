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

/// Log panic location and message over RTT, mask interrupts, then halt.
pub fn halt(info: &PanicInfo) -> ! {
    cortex_m::interrupt::disable();
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
