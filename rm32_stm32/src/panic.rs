//! Safe panic handler for motor controller firmware.
//!
//! Any panic — math overflow, array OOB, unwrap failure, ISR state missing —
//! results in all FETs forced off, interrupts disabled, CPU halted.
//! This prevents a stuck-high FET from burning the motor/ESC.
//!
//! Replaces `panic_halt` which halts without safing hardware.

use core::panic::PanicInfo;

use rm32::hal::EmergencyOff;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 1. Force all FETs off via direct GPIO writes (no state needed)
    crate::emergency::G0AEmergencyOff::emergency_off();

    // 2. Disable all interrupts to prevent further ISR triggers
    cortex_m::interrupt::disable();

    // 3. Log via RTT if initialized in main; ignored otherwise.
    // Avoid full info formatting (Display impl pulls in heavy formatting code
    // and risks stack overflow when called from a deep ISR stack). Just emit
    // file:line so we can identify the panic site.
    if let Some(loc) = info.location() {
        rtt_target::rprintln!("PANIC at {}:{}", loc.file(), loc.line());
    } else {
        rtt_target::rprintln!("PANIC (no location)");
    }

    // 4. Halt — CPU stops here, motor is safe
    loop {
        cortex_m::asm::nop();
    }
}
