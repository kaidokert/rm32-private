//! PB3 scope trigger — push-pull GPIO toggled once per electrical
//! revolution (sector 5→0), giving the scope a square wave at
//! exactly the electrical frequency. On the Vimdrones board PB3
//! reaches the J7 `D0` pin through the SN74LVC1T45 level shifter
//! (5 V, output-only) — clip the scope there, no soldering.

use crate::hal::stm32;

/// Drive PB3 high or low via BSRR (atomic set/reset, no RMW).
#[inline]
pub fn set(high: bool) {
    unsafe {
        (*stm32::GPIOB::ptr())
            .bsrr
            .write(|w| w.bits(if high { 1 << 3 } else { 1 << (16 + 3) }));
    }
}
