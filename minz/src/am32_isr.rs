//! AM32 ISR servicing bodies — COMP / COM(TIM16) / TIM6 tenKhz. The
//! `#[interrupt]` trampolines stay with the program (vector table in
//! `examples/am32_clone.rs`); these take their world as parameters.
//! Bodies moved verbatim; line-cited against AM32 `Src/main.c` +
//! `Mcu/l431/Src/stm32l4xx_it.c`.

use core::sync::atomic::Ordering;

use crate::am32_control::{commutate, safety_kill, zcfoundroutine};
use crate::am32_timers::{
    com_clear_flag, disable_com_timer_int, interval_cnt, set_and_enable_com_int, set_interval_cnt,
};
use crate::bb::Bb;
use crate::comp2::{
    self, am32_enable_comp_interrupts, am32_get_bemf_state, am32_mask_phase_interrupts,
};
use crate::hal::stm32;
use crate::tim1_motor_pwm::{self, max_duty};
use crate::zct_trace::ZctTrace;
use crate::{adc_sync, tim6_loop};

use cortex_m::interrupt::free;
use minz_core::am32;
use minz_core::am32_loop::{
    Bench, Drive, Duty, Sched, TEMP_ADVANCE, duty_ramp, uart_deadman_tick,
};
use minz_core::blackbox::EV_ACC;

// --- Bench-safety kill thresholds (observer ADC; kills only) ---
/// ~85 ms current average over raw injected ch8 counts.
const OC_KILL_RAW_AVG: u32 = 205;
const OC_WINDOW_TICKS: u32 = 1700; // ≈ 85 ms at 20 kHz
/// absolute vbat floor, raw injected ch11 counts (~5.95 V).
const VBAT_ABS_FLOOR_RAW: u16 = 793;

// ===============================================================
// COMP ISR — stm32l4xx_it.c:276-290 + interruptRoutine main.c:918-948.
// Priority 0. (Trampoline in the program's vector table.)
// ===============================================================
#[inline]
pub fn comp_isr(sched: &Sched, drive: &Drive, bb: &Bb) {
    let exti = unsafe { &*stm32::EXTI::ptr() };
    if exti.pr1.read().pr22().bit_is_set() {
        // if INTERVAL_TIMER->CNT > average_interval>>1  (it.c:280)
        if interval_cnt() > (sched.average_interval.load(Ordering::Relaxed) >> 1) {
            comp2::clear_pending(); // it.c:281
            interrupt_routine(sched, drive, bb); // it.c:282
        } else {
            // gate closed: clear ONLY if the level sits at the pre-ZC
            // level (AM32: getCompOutputLevel()==rising; minz-inverted →
            // comp2::value() != rising). Else LEAVE PENDING (their camp:
            // a post-ZC crossing re-fires until the gate opens).
            if comp2::value() != drive.rising.load(Ordering::Relaxed) {
                comp2::clear_pending(); // it.c:284-285
            }
        }
    }
}

/// interruptRoutine — main.c:918-948.
#[inline]
pub fn interrupt_routine(sched: &Sched, drive: &Drive, bb: &Bb) {
    // persistence: reject while the level is still pre-ZC (main.c:932-940;
    // `getCompOutputLevel()==rising` → return, inverted to `value != rising`).
    let filter = drive.filter_level.load(Ordering::Relaxed);
    let rising = drive.rising.load(Ordering::Relaxed);
    for _ in 0..filter {
        if comp2::value() != rising {
            return;
        }
    }
    free(|_| {
        am32_mask_phase_interrupts(); // main.c:942
        sched.last_zc.store(sched.this_zc.load(Ordering::Relaxed), Ordering::Relaxed); // :943
        let t = interval_cnt() as u16; // :944 thiszctime = INTERVAL_TIMER_COUNT
        sched.this_zc.store(t, Ordering::Relaxed);
        set_interval_cnt(0); // :945
        set_and_enable_com_int(sched.wait_time.load(Ordering::Relaxed).wrapping_add(1)); // :946
    });
    bb.record(EV_ACC, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, sched.this_zc.load(Ordering::Relaxed));
}

// ===============================================================
// COM ISR (TIM16 wrap on the shared TIM1_UP_TIM16 vector) —
// PeriodElapsedCallback main.c:896-916. Priority 0. (Trampoline in
// the program's vector table.)
// ===============================================================
#[inline]
pub fn tim1_up_tim16_isr<const N: usize>(
    sched: &Sched,
    drive: &Drive,
    zct: &ZctTrace<N>,
    duty: &Duty,
    bb: &Bb,
) {
    com_clear_flag(); // ack TIM16 UIF (TIM1.UIE is off, so this is the COM tick)
    disable_com_timer_int(); // main.c:898
    commutate(sched, drive, bb); // :899
    // commutation_interval = (ci + (lastzctime+thiszctime)/2) / 2  (:900)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let lz = sched.last_zc.load(Ordering::Relaxed) as u32;
    let tz = sched.this_zc.load(Ordering::Relaxed) as u32;
    let ci = am32::blend_interval(ci_old, lz, tz);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = ci*temp_advance>>6 ; waitTime = ci/2 - advance  (:902-906)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);
    zct.write(sched, drive, duty); // ZC_TRACE main.c:908
    if !drive.old_routine.load(Ordering::Relaxed) {
        am32_enable_comp_interrupts(); // main.c:910-912
    }
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    if zc < 10000 {
        drive.zero_crosses.store(zc + 1, Ordering::Relaxed); // main.c:913-915
    }
}

// ===============================================================
// TIM6 ISR — tenKhzRoutine main.c:1608-1809. Priority 3.
// (Trampoline in the program's vector table.)
// ===============================================================
#[inline]
pub fn tim6_dacunder_isr<const N: usize>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    bench: &Bench,
    zct: &ZctTrace<N>,
    bb: &Bb,
) {
    tim6_loop::clear_flag();

    // duty_cycle = duty_cycle_setpoint (main.c:1611); tenkhzcounter++ (:1612)
    let setpoint = duty.duty_cycle_setpoint.load(Ordering::Relaxed) as i32;
    drive.tenkhz_counter.store(drive.tenkhz_counter.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);

    if !duty.killed.load(Ordering::Relaxed) {
        let duty_val = duty_ramp(sched, drive, duty, setpoint);
        let running = duty_apply(drive, duty, duty_val);
        polling_bemf_check(sched, drive, duty, zct, bb, running);
    }

    uart_deadman_tick(duty, bench);
    adc_harvest_and_safety(drive, duty, bench, bb);
}

/// Duty apply band (main.c:1771-1791): CCR write path. Returns the
/// single `running` load so the caller's polling band reuses it
/// (preserves the original one-load ordering).
#[inline]
pub fn duty_apply(drive: &Drive, duty: &Duty, duty_val: u16) -> bool {
    let tim1_arr = max_duty() as u32;
    let running = drive.running.load(Ordering::Relaxed);
    let input = duty.input.load(Ordering::Relaxed);
    let base = (duty_val as u32 * tim1_arr) / 2000;
    let adjusted = if running && input > 47 { base + 1 } else { base };
    duty.last_duty_cycle.store(duty_val, Ordering::Relaxed); // main.c:1789
    tim1_motor_pwm::set_carrier_arr(tim1_arr as u16); // SET_AUTO_RELOAD_PWM (:1790)
    tim1_motor_pwm::set_duty(adjusted as u16); // SET_DUTY_CYCLE_ALL (:1791)
    running
}

/// old_routine polling band (main.c:1679-1696): comparator BEMF
/// counting toward min_bemf_counts, then the polling ZC accept.
#[inline]
pub fn polling_bemf_check<const N: usize>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    zct: &ZctTrace<N>,
    bb: &Bb,
    running: bool,
) {
    if drive.old_routine.load(Ordering::Relaxed) && running {
        am32_mask_phase_interrupts(); // main.c:1681
        am32_get_bemf_state(drive); // :1682
        if !drive.zcfound.load(Ordering::Relaxed) {
            let rising = drive.rising.load(Ordering::Relaxed);
            let bc = drive.bemf_counter.load(Ordering::Relaxed);
            let thresh = if rising {
                drive.min_bemf_up.load(Ordering::Relaxed)
            } else {
                drive.min_bemf_down.load(Ordering::Relaxed)
            };
            if bc > thresh {
                drive.zcfound.store(true, Ordering::Relaxed);
                zcfoundroutine(sched, drive, zct, duty, bb);
            }
        }
    }
}

/// Observer ADC harvest + the two bench-safety KILLS (non-AM32;
/// they only stop the loop, never modulate it).
#[inline]
pub fn adc_harvest_and_safety(drive: &Drive, duty: &Duty, bench: &Bench, bb: &Bb) {
    let (_a, _b, cur, vbat) = adc_sync::inj_read();
    bench.i_raw.store(cur, Ordering::Relaxed);
    bench.vbat_raw.store(vbat, Ordering::Relaxed);
    let (acc, cnt, tripped) = am32::oc_step(
        bench.oc_acc.load(Ordering::Relaxed),
        bench.oc_cnt.load(Ordering::Relaxed),
        cur,
        OC_WINDOW_TICKS,
        OC_KILL_RAW_AVG,
    );
    bench.oc_acc.store(acc, Ordering::Relaxed);
    bench.oc_cnt.store(cnt, Ordering::Relaxed);
    if tripped {
        safety_kill(drive, duty, bb, 1);
    }
    if vbat < VBAT_ABS_FLOOR_RAW && drive.running.load(Ordering::Relaxed) {
        safety_kill(drive, duty, bb, 2);
    }
}
