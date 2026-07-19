//! TIM1 triple complementary PWM — register setup matched to AM32 / rm32 L431.

use crate::hal::rcc::{APB2, Enable, Reset};
use crate::hal::stm32::{GPIOA, GPIOB, TIM1};
use crate::{TIM1_AUTORELOAD, TIM1_CCR4_TRGO, TIM1_DEAD_TIME};

/// Configure TIM1: 24 kHz PWM, CH1–3 + CH1N–3N, CH4 TRGO, dead-time, MOE.
pub fn init(tim1: TIM1, apb2: &mut APB2) {
    TIM1::enable(apb2);
    TIM1::reset(apb2);

    unsafe {
        let tim1 = &*TIM1::ptr();
        tim1.psc.write(|w| w.psc().bits(0));
        tim1.arr.write(|w| w.arr().bits(TIM1_AUTORELOAD));
        tim1.ccmr1_output().write(|w| {
            w.oc1m()
                .bits(0b110)
                .oc1pe()
                .set_bit()
                .oc2m()
                .bits(0b110)
                .oc2pe()
                .set_bit()
        });
        tim1.ccmr2_output().write(|w| {
            w.oc3m()
                .bits(0b110)
                .oc3pe()
                .set_bit()
                .oc4m()
                .bits(0b110)
                .oc4pe()
                .set_bit()
        });
        tim1.ccer.write(|w| {
            w.cc1e()
                .set_bit()
                .cc1ne()
                .set_bit()
                .cc2e()
                .set_bit()
                .cc2ne()
                .set_bit()
                .cc3e()
                .set_bit()
                .cc3ne()
                .set_bit()
                .cc4e()
                .set_bit()
        });
        tim1.ccr4.write(|w| w.ccr().bits(TIM1_CCR4_TRGO));
        tim1.bdtr
            .write(|w| w.moe().set_bit().bkp().set_bit().dtg().bits(TIM1_DEAD_TIME));
        // CR2.MMS = 010 → TRGO fires on TIM1's update event (once per
        // PWM period). Used by TIM15 in slave-reset mode to align its
        // blanking-pulse generator with the PWM cycle wrap.
        tim1.cr2.write(|w| w.mms().bits(0b010));
        tim1.cr1.write(|w| w.arpe().set_bit().cen().set_bit());
    }

    let _ = tim1;
}

/// Per-leg drive role, mirroring AM32's `phaseXPWM` / `phaseXLOW` /
/// `phaseXFLOAT` trio (`AM32/Mcu/l431/Src/phaseouts.c`):
///
/// - `Pwm` — both pins ALTERNATE, TIM1 complementary PWM drives the
///   leg (dead-time inserted by DTG).
/// - `Low` — low FET solid on by **GPIO force**: LIN OUTPUT-high,
///   HIN OUTPUT-low. The timer compare is irrelevant to this leg,
///   which is what makes commutation a pure pin-mode flip with no
///   dependence on preloaded CCR state (see [`set_six_step`]).
/// - `Float` — both pins OUTPUT-low so the gate driver sees a clean
///   0 V on both inputs and holds both FETs OFF. Leaving the pins in
///   ALTERNATE with `CCxE=0` (`BDTR.OSSR=0` releases them to Hi-Z)
///   is **not** sufficient: the gate driver's input then floats and
///   the FETs end up partially conducting under capacitive pickup
///   from neighboring PWM traces.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PhaseRole {
    Pwm,
    Low,
    Float,
}

/// Apply drive roles to the three legs. Pin map: A = PA8 hi / PA7 lo,
/// B = PA9 hi / PB0 lo, C = PA10 hi / PB1 lo.
///
/// Write order: **BSRR first, then MODER** (one write per port each).
/// The single BSRR write per port snaps every currently-OUTPUT pin to
/// its final level at once — the old Low leg's LIN turns off in the
/// same write the new Low leg's LIN turns on (zero overlap when both
/// sit on one port), and a leg staying `Low` across a commutation
/// keeps its LIN set (BS is idempotent — no off-glitch). Pins
/// currently in ALTERNATE just get their ODR preloaded; the MODER
/// write then flips them onto the pad. (AM32 orders MODER before BRR
/// per pin, which briefly drives stale ODR — ours avoids that.)
/// DRIVE-MODE EXPERIMENT (TRANSIT_AUTOPSY 07-18) — CLOSED, negative.
/// Hypothesis was a phaseouts.c misreading: the "low pin OUTPUT-high"
/// ENABLE style lives only under PWM_ENABLE_BRIDGE, which Vimdrones
/// does NOT define. AM32's shipped comp_pwm=1 phaseXPWM sets the low
/// pin ALTERNATE — byte-identical to our AF drive. Bench confirmed
/// enable-mode is non-viable on the FD6288 anyway: LIN=1 + HIN=1
/// hits the interlock, both FETs off, isns=0.000 A, 0/16 engages.
/// Kept as a compile-time switch for documentation; must stay false.
pub const DRIVE_ENABLE_MODE: bool = false;

fn set_phase_roles(a: PhaseRole, b: PhaseRole, c: PhaseRole) {
    const AF: u32 = 0b10;
    let gpioa = unsafe { &*GPIOA::ptr() };
    let gpiob = unsafe { &*GPIOB::ptr() };

    // High pins: AF when the leg PWMs. Low pins: AF only in
    // timer-complementary mode; under DRIVE_ENABLE_MODE they are
    // ALWAYS plain OUTPUT (level from BSRR below).
    let mh = |r: PhaseRole| if r == PhaseRole::Pwm { AF } else { 0b01 };
    let ml = |r: PhaseRole| {
        if r == PhaseRole::Pwm && !DRIVE_ENABLE_MODE {
            AF
        } else {
            0b01
        }
    };

    // GPIOA MODER: PA7 (A low), PA8 (A high), PA9 (B high), PA10 (C high).
    let a_mask = (0b11u32 << 14) | (0b11 << 16) | (0b11 << 18) | (0b11 << 20);
    let a_val = (ml(a) << 14) | (mh(a) << 16) | (mh(b) << 18) | (mh(c) << 20);
    // GPIOB MODER: PB0 (B low), PB1 (C low).
    let b_mask = (0b11u32 << 0) | (0b11 << 2);
    let b_val = (ml(b) << 0) | (ml(c) << 2);

    // BSRR: BS (bit N) the Low leg's LIN, BR (bit 16+N) every other
    // pin destined for OUTPUT. Pwm legs get no BSRR bits (ODR is
    // don't-care under ALTERNATE).
    let mut bsrr_a = 0u32;
    let mut bsrr_b = 0u32;
    match a {
        PhaseRole::Pwm => {
            if DRIVE_ENABLE_MODE {
                bsrr_a |= 1 << 7; // LIN=1: FD6288 interlock drives the low FET
            }
        }
        PhaseRole::Low => bsrr_a |= (1 << 7) | (1 << (16 + 8)),
        PhaseRole::Float => bsrr_a |= (1 << (16 + 7)) | (1 << (16 + 8)),
    }
    match b {
        PhaseRole::Pwm => {
            if DRIVE_ENABLE_MODE {
                bsrr_b |= 1 << 0;
            }
        }
        PhaseRole::Low => {
            bsrr_b |= 1 << 0;
            bsrr_a |= 1 << (16 + 9);
        }
        PhaseRole::Float => {
            bsrr_b |= 1 << (16 + 0);
            bsrr_a |= 1 << (16 + 9);
        }
    }
    match c {
        PhaseRole::Pwm => {
            if DRIVE_ENABLE_MODE {
                bsrr_b |= 1 << 1;
            }
        }
        PhaseRole::Low => {
            bsrr_b |= 1 << 1;
            bsrr_a |= 1 << (16 + 10);
        }
        PhaseRole::Float => {
            bsrr_b |= 1 << (16 + 1);
            bsrr_a |= 1 << (16 + 10);
        }
    }
    if bsrr_a != 0 {
        gpioa.bsrr.write(|w| unsafe { w.bits(bsrr_a) });
    }
    if bsrr_b != 0 {
        gpiob.bsrr.write(|w| unsafe { w.bits(bsrr_b) });
    }
    gpioa
        .moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !a_mask) | a_val) });
    gpiob
        .moder
        .modify(|r, w| unsafe { w.bits((r.bits() & !b_mask) | b_val) });
}

/// Phase A/B/C duty (CCR1/2/3) for continuous 3-phase drive (sine).
/// All six PWM pins are restored to ALTERNATE so TIM1 drives them;
/// `CCER` stays at its init value (all channels enabled).
#[inline]
pub fn set_duties(ch1: u16, ch2: u16, ch3: u16) {
    cortex_m::interrupt::free(|_| {
        let tim1 = unsafe { &*TIM1::ptr() };
        tim1.ccr1.write(|w| w.ccr().bits(ch1));
        tim1.ccr2.write(|w| w.ccr().bits(ch2));
        tim1.ccr3.write(|w| w.ccr().bits(ch3));
        set_phase_roles(PhaseRole::Pwm, PhaseRole::Pwm, PhaseRole::Pwm);
    });
}

/// Six-step BLDC commutation: 2 phases driven, 1 floating per sector.
/// `step ∈ 0..5` picks the (Hi, Lo, Float) mapping below.
///
/// **AM32 `SET_DUTY_CYCLE_ALL` semantics** (peripherals.h:25): `duty`
/// is written to ALL THREE CCRs, always, and the low-side leg is
/// GPIO-forced (LIN OUTPUT-high) instead of relying on the timer
/// comparing against `CCR=0`. Commutation is therefore a pure
/// pin-mode flip with zero dependence on CCR preload timing.
///
/// Why this matters: CCR writes are preloaded (`OCxPE=1`) — they land
/// at the next PWM wrap, up to one full carrier period later (20.8 µs
/// at 48 kHz). The old per-phase-CCR scheme (`duty` on hi, `0`
/// elsewhere) opened a stale-compare window on every commutation
/// where the hi role moved: the new PWM leg went AF immediately but
/// compared against its stale preloaded `CCR=0` → complementary
/// output solid high → its LOW FET fully on alongside the real low
/// leg = both motor terminals grounded = the spinning motor's BEMF
/// shorted line-to-line through two low FETs for 0–20.8 µs. A
/// multi-amp, phase-agnostic brake pulse every other commutation,
/// growing with speed/BEMF — and invisible to GECKO, whose sample
/// point sits just *after* the wrap that ends the pulse.
///
/// CCER never changes after `init()` — it stays at all-enabled.
/// TIM1's channels keep toggling internally for the floating and low
/// legs; the MODER override is what isolates them from the pad.
///
/// | Step | Hi | Lo | Float |
/// |------|----|----|-------|
/// | 0    | A  | B  | C     |
/// | 1    | A  | C  | B     |
/// | 2    | B  | C  | A     |
/// | 3    | B  | A  | C     |
/// | 4    | C  | A  | B     |
/// | 5    | C  | B  | A     |
///
/// Phase A/B/C = TIM1 CH1/2/3 (PA8/PA9/PA10 high, PA7/PB0/PB1 low).
/// R1b — duty-only write (AM32 `SET_DUTY_CYCLE_ALL` semantics: the
/// SAME value in all three CCRs, preserving the stale-CCR-fix
/// invariant). Sole intended writer: the control tick. Commutation
/// must NOT call this — AM32's commutation never touches CCR, and
/// the R1 attempt-1 bench showed tick/commutation duty interleaving
/// perturbs the loop (paired spike regression).
#[inline]
pub fn set_duty(duty: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.ccr1.write(|w| w.ccr().bits(duty));
    tim1.ccr2.write(|w| w.ccr().bits(duty));
    tim1.ccr3.write(|w| w.ccr().bits(duty));
}

/// R1b — role-only commutation flip: the HIGH/LOW/FLOAT rotation
/// with ZERO CCR traffic (CCRs already hold the tick-shaped duty).
#[inline]
pub fn set_roles_for_step(step: u8) {
    const HIGH: [u8; 6] = [0, 0, 1, 1, 2, 2];
    const LOW: [u8; 6] = [1, 2, 2, 0, 0, 1];
    let s = (step % 6) as usize;
    let hi = HIGH[s];
    let lo = LOW[s];
    let role = |ch: u8| {
        if ch == hi {
            PhaseRole::Pwm
        } else if ch == lo {
            PhaseRole::Low
        } else {
            PhaseRole::Float
        }
    };
    cortex_m::interrupt::free(|_| {
        set_phase_roles(role(0), role(1), role(2));
    });
}

#[inline]
pub fn set_six_step(step: u8, duty: u16) {
    const HIGH: [u8; 6] = [0, 0, 1, 1, 2, 2];
    const LOW: [u8; 6] = [1, 2, 2, 0, 0, 1];
    let s = (step % 6) as usize;
    let hi = HIGH[s];
    let lo = LOW[s];

    let role = |ch: u8| {
        if ch == hi {
            PhaseRole::Pwm
        } else if ch == lo {
            PhaseRole::Low
        } else {
            PhaseRole::Float
        }
    };

    cortex_m::interrupt::free(|_| {
        let tim1 = unsafe { &*TIM1::ptr() };
        tim1.ccr1.write(|w| w.ccr().bits(duty));
        tim1.ccr2.write(|w| w.ccr().bits(duty));
        tim1.ccr3.write(|w| w.ccr().bits(duty));
        set_phase_roles(role(0), role(1), role(2));
    });
}

/// Hard kill: force all six motor-control pins to GPIO OUTPUT-LOW.
/// Each gate-driver's HIN and LIN see a commanded `0` → top FET off,
/// bottom FET off, motor terminal high-Z. This reaches the same
/// "no FETs conducting" outcome the old MOE-clear path did, but by
/// **actively commanding** the gate-driver inputs rather than
/// releasing the pads to Hi-Z. Per the bench rule: the MCU always
/// drives the gate-driver inputs, never leaves them floating.
///
/// MOE stays set (the init value); the TIM1 output stage is never
/// shut down. Pin direction (AF vs OUTPUT) is what selects whether
/// TIM1 or this command holds the line — same mechanism
/// [`set_six_step`] already uses for the per-sector float phase.
#[inline]
pub fn all_off() {
    set_phase_roles(PhaseRole::Float, PhaseRole::Float, PhaseRole::Float);
}

/// Re-arm after [`all_off`]: restore all six pins to ALTERNATE
/// function so TIM1 drives them per the current CCR / CCER state.
/// The next TIM7 commutation tick (within ≤167 µs at 6 kHz) calls
/// [`set_six_step`] which re-sets the correct AF/OUTPUT split for
/// the active sector. MOE was never cleared — see [`all_off`].
#[inline]
pub fn arm_output() {
    set_phase_roles(PhaseRole::Pwm, PhaseRole::Pwm, PhaseRole::Pwm);
}

/// Enable the TIM1 update-event interrupt (DIER.UIE). Fires the
/// `TIM1_UP_TIM16` IRQ once per PWM period (24 kHz @ ARR=3332). The
/// IRQ vector is shared with TIM16, but TIM16 isn't initialised on
/// this bench — so the handler can assume every fire is TIM1.UIF.
///
/// The ISR is responsible for acking `TIM1.SR.UIF`.
#[inline]
pub fn enable_update_interrupt() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.dier.modify(|_, w| w.uie().set_bit());
}

/// Ack the TIM1 update-event flag (`SR.UIF`). Call this at the top
/// of the `TIM1_UP_TIM16` ISR before doing anything else, so the
/// IRQ doesn't re-fire on return.
#[inline]
pub fn clear_update_flag() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.sr.modify(|_, w| w.uif().clear_bit());
}

/// Is the update (PWM wrap) flag pending? The shared TIM1_UP_TIM16
/// vector dispatches on this vs TIM16's UIF (the COM timer).
#[inline]
pub fn update_flag_set() -> bool {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.sr.read().uif().bit_is_set()
}

/// Enable the capture/compare interrupts for CH1/CH2/CH3. CC4 is
/// intentionally left disabled — its compare value is the fixed
/// ADC-TRGO point (`TIM1_CCR4_TRGO = 0x64`), not a PWM transition,
/// so it would just be noise on the `TIM1_CC` vector.
///
/// Each enabled CCxIE fires `Interrupt::TIM1_CC` when `TIM1.CNT`
/// matches the corresponding `CCRx` value — i.e. exactly when that
/// channel's output transitions (high-to-low in PWM mode 1). The
/// bench uses this to timestamp every PWM edge for the software
/// COMP-blanking gate. The ISR is responsible for clearing the
/// matched flags via [`clear_cc_flags`].
/// R5a: raw TIM1 counter read for the CNT-position blank check in
/// the COMP ISR (replaces the TIM1_CC edge-timestamp ISR).
#[inline]
pub fn read_cnt() -> u16 {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.cnt.read().cnt().bits()
}

#[inline]
pub fn enable_cc_interrupts() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.dier
        .modify(|_, w| w.cc1ie().set_bit().cc2ie().set_bit().cc3ie().set_bit());
}

/// Ack TIM1's CC1IF / CC2IF / CC3IF in one shot. Call at the top of
/// the `TIM1_CC` ISR — leaving any of these set after return causes
/// an immediate re-fire of the vector.
#[inline]
pub fn clear_cc_flags() {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.sr.modify(|_, w| {
        w.cc1if()
            .clear_bit()
            .cc2if()
            .clear_bit()
            .cc3if()
            .clear_bit()
    });
}

/// R4: the LIVE carrier ARR (variable_pwm port). The control tick
/// owns writes; everyone else reads. Seeded at the base carrier.
pub static LIVE_ARR: portable_atomic::AtomicU16 = portable_atomic::AtomicU16::new(TIM1_AUTORELOAD);

#[inline]
pub fn max_duty() -> u16 {
    LIVE_ARR.load(portable_atomic::Ordering::Relaxed)
}

/// R4 — set the carrier ARR (preloaded via ARPE: lands at the next
/// wrap, glitch-free; duty RATIO is preserved by the caller
/// rescaling its counts in the same tick). AM32's variable_pwm
/// (main.c:2131): carrier rises 24→48 kHz as the interval falls.
#[inline]
pub fn set_carrier_arr(arr: u16) {
    let tim1 = unsafe { &*TIM1::ptr() };
    tim1.arr.write(|w| w.arr().bits(arr));
    LIVE_ARR.store(arr, portable_atomic::Ordering::Relaxed);
}
