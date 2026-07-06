//! LPTIM2 as a microsecond-class one-shot commutation timer (FALCON).
//!
//! The 6 kHz TIM7 stepper quantizes sector timing to 166 µs — fine for
//! observing, disqualifying for driving (30 % of a window at f=300).
//! LPTIM2 (free per the bench timer inventory) clocked from PCLK/64 =
//! 1.25 MHz gives 0.8 µs resolution and a 52 ms single-shot range.
//!
//! LPTIM quirks handled here (learned the hard way in the soft-UART
//! era): `IER` is only writable while ENABLE=0, `ARR` only while
//! ENABLE=1, and ARR writes cross a clock-domain sync (poll `ARROK`).
//! In single mode (`SNGSTRT`) the counter stops at the ARR match, so
//! re-scheduling is just "write ARR, pulse SNGSTRT" with the timer
//! left enabled.

use crate::hal::stm32;

/// 1.25 ticks per µs (80 MHz PCLK / 64).
const TICKS_PER_US_X4: u32 = 5; // ×4 fixed point: 1.25 = 5/4

pub fn init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb1enr2
            .modify(|_, w| w.lptim2en().set_bit());
    }
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    // PRESC=0b110 → /64. IER while disabled: ARR-match interrupt only.
    lptim.cr.modify(|_, w| w.enable().clear_bit());
    lptim.ier.write(|w| w.arrmie().set_bit());
    lptim.cfgr.write(|w| unsafe { w.presc().bits(0b110) });
    // Leave enabled so ARR stays writable; nothing runs until SNGSTRT.
    lptim.cr.modify(|_, w| w.enable().set_bit());
}

/// Arm (or RE-arm) the one-shot to fire the `LPTIM2` interrupt in
/// `us` microseconds. Clamped to [16 µs, 52 ms]. Callable from ISR
/// context (the ARROK poll is a few kernel clocks).
///
/// The disable/enable bounce gives clean restart semantics: in single
/// mode a running count ignores SNGSTRT, but FALCON needs to
/// overwrite a pending fallback shot with a precise ZC-derived one.
/// Disabling resets the counter (and wipes ARR — rewritten anyway).
pub fn schedule_us(us: u32) {
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    let ticks = (us.clamp(16, 52_000) * TICKS_PER_US_X4 / 4).min(0xFFFE) as u16;
    lptim.cr.modify(|_, w| w.enable().clear_bit());
    lptim.cr.modify(|_, w| w.enable().set_bit());
    // RM0394: the LPTIM is actually enabled two counter-clock cycles
    // (1.6 µs at 1.25 MHz) after ENABLE is set — starting earlier
    // silently drops SNGSTRT and kills the commutation chain.
    cortex_m::asm::delay(200);
    lptim
        .icr
        .write(|w| w.arrmcf().set_bit().arrokcf().set_bit());
    lptim.arr.write(|w| unsafe { w.arr().bits(ticks) });
    let mut guard = 0u32;
    while lptim.isr.read().arrok().bit_is_clear() && guard < 10_000 {
        guard += 1;
    }
    lptim.cr.modify(|_, w| w.sngstrt().set_bit());
}

/// Ack the ARR-match flag — first line of the `LPTIM2` ISR.
#[inline]
pub fn clear_flag() {
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    lptim.icr.write(|w| w.arrmcf().set_bit());
}
