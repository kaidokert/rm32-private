//! INDEPENDENT WATCHDOG — the guard that survives the MCU.
//!
//! The 2026-07-10 burnt motor: a deep bus sag browned out the core,
//! which WEDGED with the bridge frozen in its last state; every
//! software guard died with it and the PSU poured into a stalled
//! winding until the operator cut power. The IWDG is the layer
//! below all firmware: it resets a wedged chip and releases the
//! bridge no matter what the code was doing.
//!
//! LSI 32 kHz / 32 → 1 kHz, reload 1000 ≈ 1 s. Refresh cadence
//! contract: once per main-loop microloop (~10 ms, 100× margin).
//! The longest legitimate main-loop stall is the `u` UART blast
//! (~0.35 s at 2 Mbaud → ~3× margin); refresh mid-blast before ever
//! dropping the link baud.

use crate::hal::stm32;

/// Start with a ~1 s timeout. Irreversible until reset — that is
/// the point.
pub fn start_1s() {
    unsafe {
        let iwdg = &*stm32::IWDG::ptr();
        iwdg.kr.write(|w| w.key().bits(0x5555)); // register access
        iwdg.pr.write(|w| w.pr().bits(0b011)); // /32
        iwdg.rlr.write(|w| w.rl().bits(1000));
        iwdg.kr.write(|w| w.key().bits(0xCCCC)); // start
    }
}

/// Feed the dog.
#[inline]
pub fn refresh() {
    unsafe { (*stm32::IWDG::ptr()).kr.write(|w| w.key().bits(0xAAAA)) };
}
