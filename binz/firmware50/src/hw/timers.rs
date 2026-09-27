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
/// the ADC and the guard, TIM17 is the µs clock.
///
/// **8 MHz, 125 ns per tick (PSC=7), 536.87 s to wrap** (campaign 11). E180
/// ran it free at 64 MHz for 15.625 ns, and that was wrong for this campaign
/// in one specific way: its 32-bit counter wraps at **67.11 s** while runs are
/// commanded `total_ms = 80000`, so the run anchor wrapped *inside a run* and
/// was not an anchor. At 8 MHz one unwrapped 32-bit anchor covers the longest
/// run 6.7 times over, and the **u16** fine stamps in [`crate::chain::Beat`]
/// span **8.192 ms** instead of 1.024 ms -- which a 71 µs sector at 55% needs.
///
/// Still division-free, which the ISR arithmetic audit requires: 8 MHz is
/// 2^3 MHz, so ticks to µs is a shift by three. And 64/8 = 8 exactly, so the
/// prescaler is integral and the rate is not approximate.
///
/// The resolution given up is 8x, and 125 ns still resolves the chain's 4 µs
/// term to 3.1% where TIM17's 1 µs is 25%. **The range was bought with the
/// prescaler and not with wider fields on purpose** (E185): widening `Beat`
/// for fine stamps took the chain image's `.bss` from 28 792 to 32 888 B, left
/// under 4 KB of stack on a part whose largest frame reserves 5076 B and which
/// has no stack guard, and every run of that image died within a millisecond.
///
/// **The tick rate is part of the capture format.** Every dump states it
/// (`FINEHZ`), and the host parsers refuse a capture that does not, because a
/// silent 15.625 ns -> 125 ns change reinterprets every recorded delta by 8x --
/// the same trap `scripts/chain.py` already refuses legacy captures for.
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
    // The units and every piece of wrap-safe arithmetic live in
    // `crate::fine`, which is host-visible and therefore unit-tested. This
    // module owns only the counter.
    pub use crate::fine::{FINE_HZ, FINE_PSC, SPAN16_US, US_SHIFT, since, since16, to_tenths, to_us};

    pub fn init() {
        // SAFETY: the RCC block from the PAC's own pointer constant, and the
        // only bit touched is TIM2's peripheral clock enable -- a peripheral
        // nothing else in this firmware uses.
        let rcc = unsafe { &*stm32::RCC::ptr() };
        rcc.apbenr1().modify(|_, w| w.tim2en().set_bit());
        let t = regs();
        t.psc().write(|w| w.psc().set(FINE_PSC));
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

    /// The raw 32-bit count: 125 ns ticks, wrapping every 536.87 s.
    #[inline(always)]
    #[must_use]
    pub fn raw() -> u32 {
        regs().cnt().read().cnt().bits()
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
    use super::{Enable, Rcc, Reset, stm32};

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

    /// Crossing-arm preparation, inside the caller's atomic stop/arm region.
    /// Full CR1 write leaves CEN and ARPE clear: the final ARR is immediate.
    /// Stop the interrupt source before clearing its stale NVIC pending bit.
    #[inline(always)]
    pub fn prepare_crossing() {
        let t = regs();
        t.dier().reset();
        t.cr1().write(|w| w.opm().set_bit().urs().set_bit());
        t.cnt().write(|w| w.cnt().set(0));
        t.egr().write(|w| w.ug().set_bit());
        t.sr().reset();
        crate::hw::nvic::unpend(stm32::Interrupt::TIM16);
    }

    /// Finish only after prepare_crossing, without leaving the same mask.
    /// With ARPE=0 no second UG is needed after the deadline calculation.
    #[inline(always)]
    pub fn start_crossing(arr: u16) {
        let t = regs();
        t.arr().write(|w| w.arr().set(arr));
        t.dier().write(|w| w.uie().set_bit());
        t.cr1().write(|w| w.opm().set_bit().urs().set_bit().cen().set_bit());
    }

    /// Boot-only native-update check with bridge disabled. No COM dispatches.
    /// Validates ARR-after-UG and the minimum reload on the real peripheral;
    /// does not measure physical-edge or powered interrupt-service latency.
    pub fn crossing_selftest_off() -> bool {
        if crate::hw::pwm::moe_is_set() || crate::hw::pwm::compares() != (0, 0, 0) {
            return false;
        }
        crate::hw::nvic::mask(stm32::Interrupt::TIM16);
        let mut ok = true;
        for us in [2u16, 10, 40] {
            prepare_crossing();
            let start = super::clock::raw();
            start_crossing(us - 1);
            let mut seen = false;
            for _ in 0..2048 {
                let elapsed = super::clock::raw().wrapping_sub(start);
                if regs().sr().read().uif().bit_is_set() {
                    seen = elapsed >= us.saturating_sub(1) && elapsed <= us + 3;
                    break;
                }
                if elapsed > 80 { break; }
            }
            ok &= seen && !regs().cr1().read().cen().bit_is_set();
            stop();
            crate::hw::nvic::unpend(stm32::Interrupt::TIM16);
        }
        ok && !crate::hw::pwm::moe_is_set() && crate::hw::pwm::compares() == (0, 0, 0)
    }

    /// Counter state for the bridge-disabled peripheral exercise.
    pub fn is_running() -> bool { regs().cr1().read().cen().bit_is_set() }

    /// Update-event witness, sampled before acknowledgment in disabled tests.
    pub fn update_pending() -> bool { regs().sr().read().uif().bit_is_set() }

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
