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
use core::sync::atomic::{AtomicU32, Ordering};

/// 1.25 ticks per µs (80 MHz PCLK / 64).
const TICKS_PER_US_X4: u32 = 5; // ×4 fixed point: 1.25 = 5/4

/// ARROK-poll safety tripwire — the ARR-register write crosses into
/// the LPTIM kernel clock domain (PCLK/64 = 1.25 MHz), so we poll
/// ARROK before SNGSTRT. Measured max spin count is ~26 iterations in
/// BOTH the full and light re-arm paths (2026-07-13 investigation);
/// the 10 000 guard is a paranoid backstop that must NEVER fire.
/// A non-zero value here (surfaced in the firmware's `i` output)
/// means the ARR write failed to sync — the light re-arm's premise
/// (warm kernel → ARR always syncs) would be broken.
pub static ARROK_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

#[inline(always)]
fn poll_arrok(lptim: &stm32::lptim1::RegisterBlock) {
    let mut guard = 0u32;
    while lptim.isr.read().arrok().bit_is_clear() && guard < 10_000 {
        guard += 1;
    }
    if guard >= 10_000 {
        ARROK_GUARD_HITS.fetch_add(1, Ordering::Relaxed);
    }
}

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
/// `us` microseconds. Clamped to [4 µs, 13 ms]. Callable from ISR
/// context (the ARROK poll is a few kernel clocks).
///
/// The disable/enable bounce gives clean restart semantics: in single
/// mode a running count ignores SNGSTRT, but FALCON needs to
/// overwrite a pending fallback shot with a precise ZC-derived one.
/// Disabling resets the counter (and wipes ARR — rewritten anyway).
pub fn schedule_us(us: u32) {
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    let ticks = (us.clamp(4, 13_000) * TICKS_PER_US_X4 / 4).min(0xFFFE) as u16;
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
    poll_arrok(lptim);
    lptim.cr.modify(|_, w| w.sngstrt().set_bit());
}

/// LIGHT re-arm (lever #2, validated 2026-07-13): re-arm the one-shot
/// WITHOUT the disable/enable bounce or the `delay(200)` busy-wait.
///
/// Valid ONLY when the counter is already STOPPED — i.e. from inside
/// the `LPTIM2` ISR, which fires AT the ARR match, at which point
/// single-counting mode has halted the counter with ENABLE still set
/// and the kernel clock still warm. No pending count to cancel, so
/// SNGSTRT restarts cleanly; and because the kernel never went down
/// the ARR write syncs promptly — the ARROK poll alone suffices
/// (measured ≤26 spins, guard never hits). The `delay(200)` in
/// [`schedule_us`] was ONLY for the post-disable kernel warm-up, NOT
/// the ARR sync, so it isn't needed here. Saves ~310 cyc per
/// commutation AND removes a 2.5 µs busy-wait from the priority-1
/// commutation ISR.
///
/// Must NOT be used to overwrite a RUNNING count (the COMP ZC-refine
/// path) — single mode ignores SNGSTRT while counting, so cancelling
/// a pending shot still needs the full [`schedule_us`] disable/enable.
pub fn reschedule_light(us: u32) {
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    let ticks = (us.clamp(4, 13_000) * TICKS_PER_US_X4 / 4).min(0xFFFE) as u16;
    lptim
        .icr
        .write(|w| w.arrmcf().set_bit().arrokcf().set_bit());
    lptim.arr.write(|w| unsafe { w.arr().bits(ticks) });
    poll_arrok(lptim);
    lptim.cr.modify(|_, w| w.sngstrt().set_bit());
}

/// Ack the ARR-match flag — first line of the `LPTIM2` ISR.
#[inline]
pub fn clear_flag() {
    let lptim = unsafe { &*stm32::LPTIM2::ptr() };
    lptim.icr.write(|w| w.arrmcf().set_bit());
}
