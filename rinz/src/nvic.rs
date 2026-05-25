//! Safe NVIC helpers — `cortex_m::NVIC::unmask` is `unsafe` in the dependency API.

use cortex_m::interrupt::InterruptNumber;
use cortex_m::peripheral::NVIC;

/// Enable an interrupt in the NVIC (wraps `NVIC::unmask`).
pub fn unmask<I>(interrupt: I)
where
    I: InterruptNumber,
{
    unsafe { NVIC::unmask(interrupt) }
}
