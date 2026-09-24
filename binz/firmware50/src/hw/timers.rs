//! TIM17 (the µs clock), TIM16 (the COM one-shot) and TIM6 (ADC trigger and
//! guard tick): step 2a of the `hw/` move (notebook E111).

use stm32g0xx_hal::rcc::{Enable, Rcc, Reset};
use stm32g0xx_hal::stm32;

/// TIM17, free-running at exactly 1 MHz with a full 16-bit modulus.
///
/// **Why not the HAL** (moved from the binary, E018): `Timer::start(period)`
/// *derives* PSC and ARR from a microsecond period. It produced PSC=64
/// (984.6 kHz, every duration 1.5% long) and ARR=64526, while the extension
/// arithmetic assumes a 16-bit wrap -- 1010 µs of phantom time per wrap, which
/// tripped the guard's tick-gap and feedback-age checks on a lying clock.
/// `Stopwatch` exposes a prescaler but no ARR setter.
pub mod clock {
    use super::stm32;

    #[inline(always)]
    fn regs() -> &'static stm32::tim16::RegisterBlock {
        // SAFETY: TIM17's block from the PAC's own pointer constant; this
        // module is the only writer of PSC/ARR/EGR and only reads CNT.
        unsafe { &*stm32::TIM17::ptr() }
    }

    /// Force exactly 1 MHz (64 MHz / 64) and a full 16-bit wrap, loaded now.
    pub fn init_1mhz() {
        let t = regs();
        t.psc().write(|w| w.psc().set(63));
        t.arr().write(|w| w.arr().set(0xFFFF));
        t.egr().write(|w| w.ug().set_bit());
    }

    /// The raw 16-bit count, µs.
    #[inline(always)]
    #[must_use]
    pub fn raw() -> u16 {
        regs().cnt().read().cnt().bits()
    }
}

/// TIM16 as the commutation one-shot (the COM root's timer).
///
/// **Why not the HAL** (moved from the binary, E070): the HAL's `Timer`
/// derives PSC/ARR from a period and offers no one-pulse mode.
pub mod com_timer {
    use super::{stm32, Enable, Rcc, Reset};

    #[inline(always)]
    fn regs() -> &'static stm32::tim16::RegisterBlock {
        // SAFETY: TIM16's block from the PAC's pointer constant; owned by the
        // COM root and the foreground's arm/stop, which never overlap (the
        // foreground only arms with COM inactive or inside a critical section).
        unsafe { &*stm32::TIM16::ptr() }
    }

    /// One-time setup: clock on, reset, 1 MHz, one-pulse, update only on
    /// overflow, stopped, flags and interrupt enables clear.
    pub fn init(rcc: &mut Rcc) {
        stm32::TIM16::enable(rcc);
        stm32::TIM16::reset(rcc);
        let t = regs();
        t.cr1().write(|w| w.opm().set_bit().urs().set_bit());
        t.psc().write(|w| w.psc().set(63));
        t.arr().write(|w| w.arr().set(0xFFFF));
        t.egr().write(|w| w.ug().set_bit());
        t.sr().reset();
        t.dier().reset();
    }

    /// Fire once after `arr + 1` µs. The register sequence the COM root has
    /// always used: one-pulse config, ARR, CNT 0, UG (URS set, so no UIF),
    /// flags clear, update interrupt on, then start.
    #[inline(always)]
    pub fn arm(arr: u16) {
        let t = regs();
        t.cr1().write(|w| w.opm().set_bit().urs().set_bit());
        t.arr().write(|w| w.arr().set(arr));
        t.cnt().write(|w| w.cnt().set(0));
        t.egr().write(|w| w.ug().set_bit());
        t.sr().reset();
        t.dier().write(|w| w.uie().set_bit());
        t.cr1().write(|w| w.opm().set_bit().urs().set_bit().cen().set_bit());
    }

    /// Stop the one-shot: counter off, interrupt enables and flags clear.
    #[inline(always)]
    pub fn stop() {
        let t = regs();
        t.cr1().reset();
        t.dier().reset();
        t.sr().reset();
    }

    /// Acknowledge the update event (the COM root's first action).
    #[inline(always)]
    pub fn ack() {
        regs().sr().reset();
    }

    /// Disable the update interrupt (a stray event with COM inactive).
    #[inline(always)]
    pub fn disable_interrupt() {
        regs().dier().reset();
    }
}

/// TIM6: the 9901 Hz ADC trigger (TRGO on update) and the guard root's tick.
///
/// The period itself is set by the HAL's `Timer` (it is not critical to the
/// µs); only the master-mode selection and the flag clear are here, neither of
/// which the HAL exposes.
pub mod pace {
    use super::stm32;

    #[inline(always)]
    fn regs() -> &'static stm32::tim6::RegisterBlock {
        // SAFETY: TIM6's block from the PAC's pointer constant. CR2 is written
        // once at ADC setup; SR is cleared only by the guard root.
        unsafe { &*stm32::TIM6::ptr() }
    }

    /// Emit TRGO on every update, so each tick triggers one ADC scan.
    pub fn trgo_on_update() {
        regs().cr2().modify(|_, w| w.mms().update());
    }

    /// Clear the update flag (the guard root's first action).
    #[inline(always)]
    pub fn clear_update() {
        regs().sr().write(|w| w.uif().clear_bit());
    }
}
