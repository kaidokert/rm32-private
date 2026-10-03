//! Timer-driven sine updates; no UART/ADC work in ISR. Gates-off cancels it.
use super::*;
use portable_atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
static ACTIVE: AtomicBool = AtomicBool::new(false);
static PHASE: AtomicU32 = AtomicU32::new(0);
static RATE: AtomicU32 = AtomicU32::new(0);
static DUTY: AtomicU32 = AtomicU32::new(0);
static AMPLITUDE: AtomicU32 = AtomicU32::new(0);
static LAST: AtomicU32 = AtomicU32::new(0);
static UPDATES: AtomicU32 = AtomicU32::new(0);
static MAX_GAP: AtomicU32 = AtomicU32::new(0);
static MAX_COST: AtomicU32 = AtomicU32::new(0);
static ELAPSED: AtomicU32 = AtomicU32::new(0);
static FAULT: AtomicU32 = AtomicU32::new(0);
static ON: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];
static SIX: AtomicBool = AtomicBool::new(false);
static STEP: AtomicU32 = AtomicU32::new(0);
static SECTOR_TIME: AtomicU32 = AtomicU32::new(0);
static STOP_PHASE: AtomicU32 = AtomicU32::new(0);
static STOP_TIME: AtomicU32 = AtomicU32::new(0);
static STOP_SIX: AtomicBool = AtomicBool::new(false);
static STOP_VALID: AtomicBool = AtomicBool::new(false);
pub fn stop_anchor() -> (bool, u32, u16, bool) {
    (
        STOP_VALID.load(Relaxed),
        STOP_PHASE.load(Relaxed),
        STOP_TIME.load(Relaxed) as u16,
        STOP_SIX.load(Relaxed),
    )
}
pub fn stop() {
    // Capture once on active->stopped, before bridge_clear removes MOE. Repeated
    // idle safing must not overwrite the anchor. This is commanded phase only.
    cortex_m::interrupt::free(|_| {
        if ACTIVE.swap(false, Relaxed) {
            let now = t17();
            let phase = PHASE.load(Relaxed).wrapping_add(
                RATE.load(Relaxed)
                    .wrapping_mul(now.wrapping_sub(LAST.load(Relaxed) as u16) as u32),
            );
            STOP_PHASE.store(phase, Relaxed);
            STOP_TIME.store(now as u32, Relaxed);
            STOP_SIX.store(SIX.load(Relaxed), Relaxed);
            STOP_VALID.store(true, Relaxed);
        }
    });
    cortex_m::peripheral::NVIC::mask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    unsafe {
        (*stm32::TIM6::ptr()).dier().write(|w| w.bits(0));
        (*stm32::TIM6::ptr()).cr1().write(|w| w.bits(0));
    }
    cortex_m::peripheral::NVIC::unpend(stm32::Interrupt::TIM6_DAC_LPTIM1);
}
pub fn configure(rate: u32, duty: u32) {
    cortex_m::interrupt::free(|_| {
        let duty = duty.min(MAX_DUTY_TENTHS);
        RATE.store(rate, Relaxed);
        DUTY.store(duty, Relaxed);
        AMPLITUDE.store(sine_scale::amplitude(PWM_ARR, duty), Relaxed);
    });
}
pub fn start(rate: u32, duty: u32) {
    stop();
    configure(rate, duty);
    STOP_VALID.store(false, Relaxed);
    SIX.store(false, Relaxed);
    PHASE.store(0, Relaxed);
    UPDATES.store(0, Relaxed);
    MAX_GAP.store(0, Relaxed);
    MAX_COST.store(0, Relaxed);
    ELAPSED.store(0, Relaxed);
    FAULT.store(0, Relaxed);
    for v in &ON {
        v.store(0, Relaxed);
    }
    unsafe {
        (*stm32::RCC::ptr())
            .apbenr1()
            .modify(|r, w| w.bits(r.bits() | (1 << 4)));
        let t = &*stm32::TIM6::ptr();
        t.cr1().write(|w| w.bits(0));
        t.psc().write(|w| w.bits(63));
        t.arr().write(|w| w.bits(99));
        t.egr().write(|w| w.bits(1));
        t.sr().write(|w| w.bits(0));
        LAST.store(t17() as u32, Relaxed);
        ACTIVE.store(true, Relaxed);
        t.dier().write(|w| w.bits(1));
        t.cr1().write(|w| w.bits(1));
        cortex_m::peripheral::NVIC::unmask(stm32::Interrupt::TIM6_DAC_LPTIM1);
    }
}
pub fn start_six(theta: u32, hz: u32, duty: u32) {
    cortex_m::interrupt::free(|_| {
        let previous = stats();
        start(campaign::phase_rate(hz * 100), duty);
        UPDATES.store(previous.0, Relaxed);
        MAX_GAP.store(previous.1, Relaxed);
        MAX_COST.store(previous.2, Relaxed);
        PHASE.store(theta, Relaxed);
        SIX.store(true, Relaxed);
        STEP.store(observation::step_for_phase(theta) as u32, Relaxed);
        SECTOR_TIME.store(t17() as u32, Relaxed);
    });
}
pub fn physical() -> (u8, u16) {
    cortex_m::interrupt::free(|_| (STEP.load(Relaxed) as u8, SECTOR_TIME.load(Relaxed) as u16))
}
pub fn fault() -> u8 {
    FAULT.load(Relaxed) as u8
}
pub fn snapshot() -> (u32, [u16; 3]) {
    cortex_m::interrupt::free(|_| {
        (
            PHASE.load(Relaxed),
            [
                ON[0].load(Relaxed) as u16,
                ON[1].load(Relaxed) as u16,
                ON[2].load(Relaxed) as u16,
            ],
        )
    })
}
pub fn halt_phase() -> u32 {
    cortex_m::interrupt::free(|_| {
        let phase = PHASE.load(Relaxed).wrapping_add(
            RATE.load(Relaxed)
                .wrapping_mul(t17().wrapping_sub(LAST.load(Relaxed) as u16) as u32),
        );
        stop();
        phase
    })
}
pub fn stats() -> (u32, u32, u32) {
    (
        UPDATES.load(Relaxed),
        MAX_GAP.load(Relaxed),
        MAX_COST.load(Relaxed),
    )
}
pub fn interrupt() {
    let start = t17();
    unsafe {
        (*stm32::TIM6::ptr()).sr().write(|w| w.bits(0));
    }
    if !ACTIVE.load(Relaxed) {
        return;
    }
    let dt = start.wrapping_sub(LAST.load(Relaxed) as u16) as u32;
    MAX_GAP.store(MAX_GAP.load(Relaxed).max(dt), Relaxed);
    let elapsed = ELAPSED.load(Relaxed) + dt;
    ELAPSED.store(elapsed, Relaxed);
    let limit = if SIX.load(Relaxed) {
        96_000
    } else if cfg!(feature = "bench-hold-30s") {
        29_999_000
    } else {
        4_999_000
    };
    let reason = if !get_idr(1, 14) {
        2
    } else if dt > 500 {
        7
    } else if elapsed >= limit {
        1
    } else {
        0
    };
    if reason != 0 {
        FAULT.store(reason, Relaxed);
        gates_off();
        set_pin(3, 1, false);
        return;
    }
    // On a guard exit stop() needs the old LAST to extrapolate the final gap.
    LAST.store(start as u32, Relaxed);
    let phase = PHASE
        .load(Relaxed)
        .wrapping_add(RATE.load(Relaxed).wrapping_mul(dt));
    PHASE.store(phase, Relaxed);
    if SIX.load(Relaxed) {
        let desired = observation::step_for_phase(phase);
        if desired as u32 != STEP.load(Relaxed) {
            core_bench::physical_change(desired);
            sixstep_write(desired, DUTY.load(Relaxed));
            STEP.store(desired as u32, Relaxed);
            SECTOR_TIME.store(t17() as u32, Relaxed);
            core_bench::physical_applied(desired);
        }
    } else {
        let on = pwm_sine_prepared(AMPLITUDE.load(Relaxed), phase);
        for i in 0..3 {
            ON[i].store(on[i] as u32, Relaxed);
        }
    }
    UPDATES.store(UPDATES.load(Relaxed) + 1, Relaxed);
    MAX_COST.store(
        MAX_COST.load(Relaxed).max(t17().wrapping_sub(start) as u32),
        Relaxed,
    );
}
