//! The bridge's digital pins: the six TIM1 gate outputs, ENABLE (PD1) and the
//! DRV8304's open-drain nFAULT (PB14).
//!
//! **Why PAC and not the HAL.** The gate pins change owner at run time --
//! TIM1 alternate function while driving, plain low outputs the instant the
//! bridge is made safe -- and `safe_off` runs from the panic handler and the
//! protection ISR, where no HAL pin object can be reached. ENABLE is reached
//! from the panic handler for the same reason.

use stm32g0xx_hal::stm32;

/// Gate pins on port A: PA7 (CH1N), PA8 (CH1), PA9 (CH2), PA10 (CH3).
pub const GATE_A: [u8; 4] = [7, 8, 9, 10];
/// Gate pins on port B: PB0 (CH2N), PB1 (CH3N).
pub const GATE_B: [u8; 2] = [0, 1];
/// DRV8304 ENABLE on PD1.
pub const EN_PIN: u8 = 1;
/// DRV8304 nFAULT on PB14 (open-drain, pulled up).
pub const NFAULT_PIN: u8 = 14;

#[inline(always)]
fn gpioa() -> &'static stm32::gpioa::RegisterBlock {
    // SAFETY: GPIOA's block from the PAC's pointer constant; the pins touched
    // here are owned by this module (the HAL never splits them out).
    unsafe { &*stm32::GPIOA::ptr() }
}

#[inline(always)]
fn gpiob() -> &'static stm32::gpiob::RegisterBlock {
    // SAFETY: as `gpioa`; PB0/PB1/PB14 only.
    unsafe { &*stm32::GPIOB::ptr() }
}

/// GPIOD shares GPIOB's register-block layout in this PAC.
#[inline(always)]
fn gpiod() -> &'static stm32::gpiob::RegisterBlock {
    // SAFETY: as `gpioa`; PD1 only.
    unsafe { &*stm32::GPIOD::ptr() }
}

/// Put the six gate pins into AF2 so TIM1 drives them.
pub fn gates_to_timer() {
    let a = gpioa();
    // PA7 lives in AFRL. PA8/9/10 live in AFRH, whose indexed accessor is
    // 0-based over pins 8..15 -- `afr(0)` is PA8, not PA0. Indexing it with the
    // pin number is a known trap on this PAC and panics at 8.
    a.afrl().modify(|_, w| w.afr(7).af2());
    a.afrh().modify(|_, w| {
        w.afr(0).af2();
        w.afr(1).af2();
        w.afr(2).af2()
    });
    for p in GATE_A {
        a.moder().modify(|_, w| w.moder(p).alternate());
    }
    let b = gpiob();
    b.afrl().modify(|_, w| {
        w.afr(0).af2();
        w.afr(1).af2()
    });
    for p in GATE_B {
        b.moder().modify(|_, w| w.moder(p).alternate());
    }
}

/// Drive all six gate pins low as plain outputs, reclaiming them from TIM1.
#[inline(always)]
pub fn gates_low_now() {
    let a = gpioa();
    let b = gpiob();
    // Pre-load the output latches, then switch mode, then drive again: in this
    // order the pin never passes through an undefined level.
    for p in GATE_A {
        a.bsrr().write(|w| w.br(p).set_bit());
    }
    for p in GATE_B {
        b.bsrr().write(|w| w.br(p).set_bit());
    }
    for p in GATE_A {
        a.moder().modify(|_, w| w.moder(p).output());
    }
    for p in GATE_B {
        b.moder().modify(|_, w| w.moder(p).output());
    }
    for p in GATE_A {
        a.bsrr().write(|w| w.br(p).set_bit());
    }
    for p in GATE_B {
        b.bsrr().write(|w| w.br(p).set_bit());
    }
}

/// True when every gate pin reads low at the pad.
pub fn gates_all_low() -> bool {
    let a = gpioa().idr().read();
    let b = gpiob().idr().read();
    let mut low = true;
    for p in GATE_A {
        if a.idr(p).is_high() {
            low = false;
        }
    }
    for p in GATE_B {
        if b.idr(p).is_high() {
            low = false;
        }
    }
    low
}

/// Drive ENABLE high or low.
#[inline(always)]
pub fn enable_set(high: bool) {
    gpiod().bsrr().write(|w| {
        if high {
            w.bs(EN_PIN).set_bit()
        } else {
            w.br(EN_PIN).set_bit()
        }
    });
}

#[inline(always)]
pub fn enable_is_high() -> bool {
    gpiod().idr().read().idr(EN_PIN).is_high()
}

#[inline(always)]
pub fn nfault_high() -> bool {
    gpiob().idr().read().idr(NFAULT_PIN).is_high()
}

/// Gate-4 stimulus: pull the shared open-drain nFAULT node low from the MCU
/// side (open-drain output, ODR low). The DRV8304's own output is open-drain
/// on the same pulled-up line, so both pulling low is electrically benign.
pub fn nfault_inject_assert() {
    let g = gpiob();
    g.otyper().modify(|_, w| w.ot(NFAULT_PIN).open_drain());
    g.bsrr().write(|w| w.br(NFAULT_PIN).set_bit());
    g.moder().modify(|_, w| w.moder(NFAULT_PIN).output());
}

/// Undo `nfault_inject_assert`: input, push-pull (the reset configuration).
pub fn nfault_inject_release() {
    let g = gpiob();
    g.moder().modify(|_, w| w.moder(NFAULT_PIN).input());
    g.otyper().modify(|_, w| w.ot(NFAULT_PIN).push_pull());
}
