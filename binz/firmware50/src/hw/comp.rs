//! COMP2 and its EXTI line 18: the comparator path of the COMP root
//! (step 2c of the `hw/` move, notebook E113).
//!
//! **Why not the HAL** (moved from the binary): the HAL's comparator
//! constructor takes its inputs *by value*, so the negative input could not be
//! re-selected per sector, and its `exti` module offers no edge selection,
//! mask or pending clear for a configurable internal line such as COMP2's.
//! COMP2's `INPSEL`/`INMSEL`/`HYST` field writers are `Unsafe` in the PAC; the
//! values written come from `commutation::comparator_inmsel` (host-tested)
//! and fixed constants.

use stm32g0xx_hal::stm32;
use stm32g0xx_hal::stm32::Interrupt;

use super::nvic;

#[inline(always)]
fn comp() -> &'static stm32::comp::RegisterBlock {
    // SAFETY: COMP's block from the PAC's pointer constant. COMP2_CSR is
    // written by setup, the COM root (mux) and the coast (hysteresis), never
    // concurrently.
    unsafe { &*stm32::COMP::ptr() }
}

#[inline(always)]
fn exti() -> &'static stm32::exti::RegisterBlock {
    // SAFETY: EXTI's block from the PAC's pointer constant. Only line 18 is
    // touched, by the COMP and COM roots (peers) and the foreground with the
    // line masked.
    unsafe { &*stm32::EXTI::ptr() }
}

/// Bring COMP2 up: `inpsel` on the positive input, `inmsel` on the negative,
/// `hyst` of hardware hysteresis, non-inverted, enabled; then the ~5 µs the
/// reference allows before the output means anything
/// (`binz/examples/shell-pwm.rs:1461`, `asm::delay(320)`).
pub fn init(inpsel: u8, inmsel: u8, hyst: u8) {
    // COMP shares the SYSCFG clock gate; the HAL's `Rcc` does not expose it.
    // SAFETY: RCC's block from the PAC's pointer constant; one named bit.
    unsafe { &*stm32::RCC::ptr() }
        .apbenr2()
        .modify(|_, w| w.syscfgen().set_bit());
    comp().comp2_csr().modify(|_, w| {
        // The PAC marks these three field writers unsafe (module docs).
        // SAFETY: an INPSEL code from `roots::COMP2_INPSEL_PA3`.
        unsafe { w.inpsel().bits(inpsel) };
        // SAFETY: an INMSEL code from `commutation::comparator_inmsel`.
        unsafe { w.inmsel().bits(inmsel) };
        // SAFETY: a HYST code, 0..=3, from `roots::COMP2_HYST`.
        unsafe { w.hyst().bits(hyst) };
        w.polarity().clear_bit().en().set_bit()
    });
    cortex_m::asm::delay(320);
}

/// Point the negative input at the sector's floating phase.
#[inline(always)]
pub fn select_negative(inmsel: u8) {
    // SAFETY: see the module docs.
    comp().comp2_csr().modify(|_, w| unsafe { w.inmsel().bits(inmsel) });
}

/// Set the hardware hysteresis (the coast witness uses its own).
pub fn set_hysteresis(hyst: u8) {
    // SAFETY: see the module docs.
    comp().comp2_csr().modify(|_, w| unsafe { w.hyst().bits(hyst) });
}

/// The comparator output level.
#[inline(always)]
#[must_use]
pub fn level() -> bool {
    comp().comp2_csr().read().value().bit_is_set()
}

/// COMP2_CSR.VALUE is bit30 (the PAC field used by `level`).
/// Prepare once outside persistence, avoiding repeated Boolean normalization.
pub const fn expected_level_word(high: bool) -> u32 {
    (high as u32) << 30
}

#[inline(always)]
pub fn level_word() -> u32 {
    comp().comp2_csr().read().bits() & expected_level_word(true)
}

/// Disable line 18 **at the NVIC first**, then in IMR (binz Entry 094,
/// firmware50 E102/E103: an IMR mask alone does not stop a latched edge from
/// dispatching on the G071).
#[inline(always)]
pub fn line_disable() {
    nvic::mask(Interrupt::ADC_COMP);
    exti().imr1().modify(|_, w| w.im18().clear_bit());
}

/// Enable line 18 the way binz Entry 094 does: IMR set, stale NVIC pending
/// cleared (the EXTI pending is kept), then the NVIC unmasked.
#[inline(always)]
pub fn line_enable() {
    exti().imr1().modify(|_, w| w.im18().set_bit());
    nvic::unpend(Interrupt::ADC_COMP);
    nvic::unmask(Interrupt::ADC_COMP);
}

/// Clear both of line 18's pending flags (write-1-to-clear).
#[inline(always)]
pub fn clear_pending() {
    let e = exti();
    e.rpr1().write(|w| w.rpif18().clear_bit_by_one());
    e.fpr1().write(|w| w.fpif18().clear_bit_by_one());
}

/// Select the edge line 18 fires on: exactly one of rising or falling.
#[inline(always)]
pub fn select_edge(rising: bool) {
    let e = exti();
    e.rtsr1().modify(|_, w| w.tr18().bit(rising));
    e.ftsr1().modify(|_, w| w.tr18().bit(!rising));
}

/// Is line 18 enabled in IMR?
#[inline(always)]
#[must_use]
pub fn line_live() -> bool {
    exti().imr1().read().im18().bit_is_set()
}

/// Is an edge pending on line 18 (either direction)?
#[inline(always)]
#[must_use]
pub fn pending() -> bool {
    let e = exti();
    // Both registers read every time, as the pre-E113 code did (E128): the
    // short-circuit `||` this replaced changed the level revisit's
    // critical-section timing.
    let rising = e.rpr1().read().rpif18().bit_is_set();
    let falling = e.fpr1().read().fpif18().bit_is_set();
    rising | falling
}

/// Drop a pending COMP request at the NVIC.
#[inline(always)]
pub fn nvic_unpend() {
    nvic::unpend(Interrupt::ADC_COMP);
}

/// Pend the COMP root in software (the level revisit and the storm
/// provocation).
#[inline(always)]
pub fn pend() {
    nvic::pend(Interrupt::ADC_COMP);
}

/// Priority for the COMP root, and its NVIC line opened (the EXTI line itself
/// stays masked until the first `line_enable`).
pub fn nvic_init(prio: u8) {
    nvic::set_priority(Interrupt::ADC_COMP, prio);
    nvic::unmask(Interrupt::ADC_COMP);
}
