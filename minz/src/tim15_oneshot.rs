//! TIM15 as a microsecond one-shot commutation timer — the low-latency
//! replacement for `lptim2_oneshot`.
//!
//! LPTIM2's ARR write crosses a clock-domain sync (the ARROK poll) and
//! overwriting a running one-shot needs a disable/enable bounce + a
//! ~2.5 µs warm-up `delay(200)` — ~3 µs of fixed schedule overhead in
//! the commutation ISR, and the commutation fires that much later. A
//! general-purpose timer in one-pulse mode has none of it: write ARR,
//! reset CNT, set CEN — the compare reprograms on the fly, no sync.
//!
//! TIM16 is production rm32's COM_TIMER, but on L431 its interrupt is
//! the `TIM1_UP_TIM16` vector — which minz already uses for the 24 kHz
//! ADC ISR (rm32 gets away with TIM16 only because it never enables
//! TIM1_UP). TIM15's `TIM1_BRK_TIM15` vector is free, so TIM15 is
//! the clean L431 equivalent: same GP-timer, own vector, commutation
//! stays at NVIC priority 1 (no ISR merge, no priority shuffle).
//!
//! Clock: APB2 timer clock = 80 MHz, PSC=79 → 1 MHz (1 µs/tick), 16-bit
//! ARR → 65 ms range (commutation delays are ≤ ~1.7 ms). One-pulse
//! mode (`CR1.OPM`) auto-clears CEN at the ARR match, so re-arming is
//! just "write ARR, reset CNT, set CEN".

use crate::hal::stm32;
use core::sync::atomic::AtomicU32;

/// 1 tick per µs (80 MHz / (PSC+1=80)).
const TICKS_PER_US: u32 = 1;

/// Kept for `i`-output compatibility with the LPTIM2 module; TIM15 has
/// no clock-domain ARR sync, so this never increments.
pub static ARROK_GUARD_HITS: AtomicU32 = AtomicU32::new(0);

pub fn init() {
    unsafe {
        (*stm32::RCC::ptr())
            .apb2enr
            .modify(|_, w| w.tim15en().set_bit());
    }
    let tim = unsafe { &*stm32::TIM15::ptr() };
    // Stop, 1 µs tick, one-pulse mode. URS=1 so a UG (used to load PSC)
    // does NOT raise the update IRQ — only a real ARR match does.
    tim.cr1.modify(|_, w| {
        w.cen()
            .clear_bit()
            .opm()
            .set_bit()
            .urs()
            .set_bit()
            .arpe()
            .clear_bit()
    });
    tim.psc
        .write(|w| w.psc().bits((80 / TICKS_PER_US - 1) as u16));
    tim.arr.write(|w| unsafe { w.arr().bits(1) });
    // Load PSC/ARR now (UG); URS=1 keeps it from setting the IRQ.
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.modify(|_, w| w.uif().clear_bit());
    tim.dier.write(|w| w.uie().set_bit());
    // CEN stays 0 — nothing runs until schedule_us().
}

/// Arm (or RE-arm) the one-shot to fire the `TIM1_BRK_TIM15`
/// interrupt in `us` microseconds. Clamped to [1 µs, 65 ms]. Callable
/// from ISR context; no sync poll, no busy-wait. Overwrites a running
/// one-shot cleanly (stop → reload → start).
pub fn schedule_us(us: u32) {
    let tim = unsafe { &*stm32::TIM15::ptr() };
    let ticks = (us.clamp(1, 65_000) * TICKS_PER_US).min(0xFFFE) as u16;
    tim.cr1.modify(|_, w| w.cen().clear_bit());
    tim.arr.write(|w| unsafe { w.arr().bits(ticks) });
    // UG reloads ARR AND resets CNT + the PRESCALER counter, so the
    // first tick after start is a full, phase-consistent µs (a bare
    // CNT=0 write leaves the prescaler mid-period → 0-1 µs of
    // per-commutation jitter). URS=1 blocks UG from raising the IRQ;
    // with CEN=0 the OPM stop-on-update is moot. Then clear the UG's
    // UIF and start.
    tim.egr.write(|w| w.ug().set_bit());
    tim.sr.modify(|_, w| w.uif().clear_bit());
    tim.cr1.modify(|_, w| w.cen().set_bit());
}

/// TIM15 has no bounce/warm-up, so the "light" re-arm is identical to
/// [`schedule_us`] (kept for API parity with the LPTIM2 module).
#[inline]
pub fn reschedule_light(us: u32) {
    schedule_us(us);
}

/// Ack the update flag — first line of the `TIM1_BRK_TIM15` ISR.
#[inline]
pub fn clear_flag() {
    let tim = unsafe { &*stm32::TIM15::ptr() };
    tim.sr.modify(|_, w| w.uif().clear_bit());
}
