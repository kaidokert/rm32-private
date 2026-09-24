//! TIM17 (the µs clock), TIM16 (the COM one-shot), TIM6 (ADC trigger and
//! guard tick) and TIM2 (the diagnostic fine clock): step 2a of the `hw/` move
//! (notebook E111), plus E180's fine clock.

use stm32g0xx_hal::rcc::{Enable, Rcc, Reset};
use stm32g0xx_hal::stm32;

/// **TIM2, free-running at the core clock: the diagnostic fine clock (E180).**
///
/// The control timebase is TIM17 at 1 MHz, and that is right for control --
/// every wait, blank and interval in this firmware is a whole number of
/// microseconds. It is wrong for *measuring* the commutation chain: the
/// crossing-to-bridge delay is 17-20 µs and the chain term inside it is 4 µs,
/// so a 1 µs stamp quantises the measured quantity in 25% steps, and two A/B
/// comparisons (E166, E168) turned on differences of exactly one tick.
///
/// TIM2 is the only 32-bit timer on this part and this firmware does not use
/// it: TIM1 drives the bridge, TIM16 is the commutation one-shot, TIM6 paces
/// the ADC and the guard, TIM17 is the µs clock. Free-running with no
/// prescaler it ticks at 64 MHz -- **15.6 ns**, 67 s to wrap -- so no software
/// extension is needed at all, which also retires the extended-modulus bug
/// class this bench has already been bitten by. And because 64 MHz is 2^6 MHz,
/// converting ticks to µs is a shift by six: division-free by construction,
/// which the ISR arithmetic audit requires.
///
/// **Diagnostic images only.** Production never initialises TIM2 and never
/// reads it; the recorders that do are compiled out of production by their
/// `const ON: bool`. Control decisions continue to use TIM17 -- the scheduled
/// wait *is* a µs quantity, and pretending otherwise would be a different
/// error from the one this fixes.
pub mod fine {
    use super::stm32;

    /// Enable TIM2 and let it run free at the core clock.
    ///
    /// Takes no `Rcc`: the HAL's `Enable` trait needs the frozen `Rcc` value,
    /// which the binary does not keep, and this module is inside `hw/` where
    /// register writes belong. It sets only TIM2's own enable bit.
    pub fn init() {
        // SAFETY: the RCC block from the PAC's own pointer constant, and the
        // only bit touched is TIM2's peripheral clock enable -- a peripheral
        // nothing else in this firmware uses.
        let rcc = unsafe { &*stm32::RCC::ptr() };
        rcc.apbenr1().modify(|_, w| w.tim2en().set_bit());
        let t = regs();
        t.psc().write(|w| w.psc().set(0));
        t.arr().write(|w| w.arr().set(u32::MAX));
        t.egr().write(|w| w.ug().set_bit());
        t.cr1().write(|w| w.cen().set_bit());
    }

    #[inline(always)]
    fn regs() -> &'static stm32::tim2::RegisterBlock {
        // SAFETY: TIM2's block from the PAC's own pointer constant. Only this
        // module touches it, and only the diagnostic images call `init`.
        unsafe { &*stm32::TIM2::ptr() }
    }

    /// The raw 32-bit count: 15.6 ns ticks, wrapping every 67 s.
    #[inline(always)]
    #[must_use]
    pub fn raw() -> u32 {
        regs().cnt().read().cnt().bits()
    }

    /// Ticks to whole µs: a shift by six, never a division (64 MHz = 2^6 MHz).
    #[inline(always)]
    #[must_use]
    pub const fn to_us(ticks: u32) -> u32 {
        ticks >> 6
    }

    /// Ticks to tenths of a µs, for the host's own arithmetic: `t * 10 >> 6`
    /// stays inside `u32` for any delta under 6.7 s.
    #[inline(always)]
    #[must_use]
    pub const fn to_tenths(ticks: u32) -> u32 {
        (ticks * 10) >> 6
    }
}

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
