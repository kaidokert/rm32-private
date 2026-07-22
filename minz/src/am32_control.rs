//! AM32 control transliteration, hardware side — commutation, polling
//! ZC, start/stop, setInput. Line-cited against `Src/main.c`; the pure
//! halves live in `minz_core::am32_loop`. Bodies moved verbatim from
//! `examples/am32_clone.rs`; every fn takes its world (the cohesion
//! clusters + `&Bb`) as parameters — the program keeps the storage.

use core::sync::atomic::Ordering;

use crate::am32_timers::{
    com_set_arr, disable_com_timer_int, interval_cnt, set_interval_cnt,
};
use crate::bb::Bb;
use crate::comp2::{
    am32_change_comp_input, am32_enable_comp_interrupts, am32_mask_phase_interrupts,
};
use crate::tim1_motor_pwm;
use crate::zct_trace::ZctTrace;

use minz_core::am32;
use minz_core::am32_loop::{
    BEMF_TIMEOUT_TICKS, Bench, Drive, Duty, MIN_STARTUP_DUTY, POLLING_MODE_CHANGEOVER, Sched,
    STARTUP_INTERVAL_TICKS, TEMP_ADVANCE, VARIABLE_PWM, set_input_clamp,
};
use minz_core::blackbox::{EV_DSY, EV_REF};

/// TIM1 base ARR = 24 kHz carrier. AM32 targets.h:5335
/// `TIM1_AUTORELOAD = CPU_FREQUENCY_MHZ*1e6/NOMINAL_PWM - 1 = 3332`.
const TIMER1_MAX_ARR: u16 = crate::TIM1_AUTORELOAD; // 3332

// ===============================================================
// commutate() — main.c:854-894 (forward-only factory path).
// ===============================================================
#[inline(always)]
pub fn commutate(sched: &Sched, drive: &Drive, bb: &Bb) {
    // step++ ; if step>6 { step=1; desync_check=1 }  (main.c:856-861)
    let mut step = drive.current_step.load(Ordering::Relaxed);
    step += 1;
    if step > 6 {
        step = 1;
        drive.desync_check.store(true, Ordering::Relaxed);
    }
    // rising = step % 2  (main.c:862)
    let rising = (step & 1) == 1;
    drive.rising.store(rising, Ordering::Relaxed);
    drive.current_step.store(step, Ordering::Relaxed);
    let sector = (step - 1) as usize;

    // comStep(step) (main.c:876) — minz role-only flip; CCRs hold the
    // tick-shaped duty. AM32 wraps this in __disable_irq; set_roles_for_step
    // does its own interrupt::free.
    tim1_motor_pwm::set_roles_for_step(sector as u8);
    // changeCompInput() (main.c:879).
    am32_change_comp_input(sector);

    // if average_interval > polling_mode_changeover+500 → old_routine=1
    // (main.c:881-883).
    if sched.average_interval.load(Ordering::Relaxed) > POLLING_MODE_CHANGEOVER + 500 {
        drive.old_routine.store(true, Ordering::Relaxed);
    }
    // bemfcounter=0; zcfound=0 (main.c:885-886).
    drive.bemf_counter.store(0, Ordering::Relaxed);
    drive.zcfound.store(false, Ordering::Relaxed);
    // commutation_intervals[step-1] = commutation_interval (main.c:887).
    sched.intervals().push(sector, sched.commutation_interval.load(Ordering::Relaxed));

    bb.record(EV_REF, sector as u8, sched.commutation_interval.load(Ordering::Relaxed) as u16);
}

// ===============================================================
// zcfoundroutine() — main.c:1868-1915 (polling mode, blocking).
// ===============================================================

/// zcfoundroutine blend band (main.c:1870-1874): capture thiszctime,
/// reset INTERVAL_TIMER, blend commutation_interval, derive advance/waitTime.
#[inline]
pub fn zcfr_blend(sched: &Sched) -> (u32, u32) {
    // thiszctime = INTERVAL_TIMER_COUNT; SET_INTERVAL_TIMER_COUNT(0)
    let thiszc = interval_cnt() as u16; // main.c:1870
    set_interval_cnt(0); // main.c:1871
    sched.this_zc.store(thiszc, Ordering::Relaxed);
    // commutation_interval = (thiszctime + 3*ci)/4  (main.c:1872)
    let ci_old = sched.commutation_interval.load(Ordering::Relaxed);
    let ci = am32::polling_blend(thiszc as u32, ci_old);
    sched.commutation_interval.store(ci, Ordering::Relaxed);
    // advance = temp_advance*ci >> 6 ; waitTime = ci/2 - advance  (1873-4)
    let advance = am32::advance_of(ci, TEMP_ADVANCE);
    let wait = am32::wait_time(ci, advance);
    sched.wait_time.store(wait as u16, Ordering::Relaxed);
    (ci, wait)
}

/// zcfoundroutine spin-wait band (main.c:1875-1879):
/// `while INTERVAL_TIMER_COUNT < waitTime { if zero_crosses<5 break }`.
/// DEVIATION #1: bounded by a spin guard — AM32's loop is unbounded (a
/// wedged INTERVAL_TIMER hangs the 20 kHz ISR forever; AM32 accepts that,
/// the IWDG would reboot). We add a loud 65535-iteration break so the tick
/// can't be captured indefinitely.
#[inline]
pub fn zcfr_spin_wait(drive: &Drive, zc: u32, wait: u32) {
    let mut guard: u32 = 0;
    loop {
        if interval_cnt() >= wait {
            break;
        }
        if zc < 5 {
            break;
        }
        guard += 1;
        if guard > 65_535 {
            drive.zcfr_guard_hits.fetch_add(1, Ordering::Relaxed);
            break;
        }
    }
}

#[inline(always)]
pub fn zcfoundroutine<const N: usize>(
    sched: &Sched,
    drive: &Drive,
    zct: &ZctTrace<N>,
    duty: &Duty,
    bb: &Bb,
) {
    let (ci, wait) = zcfr_blend(sched); // main.c:1870-1874
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    zcfr_spin_wait(drive, zc, wait); // main.c:1875-1879

    com_set_arr(wait as u16); // COM_TIMER->ARR = waitTime (main.c:1884)
    commutate(sched, drive, bb); // main.c:1889
    zct.write(sched, drive, duty); // ZC_TRACE main.c:1891
    drive.bemf_counter.store(0, Ordering::Relaxed); // main.c:1893
    drive.bad_count.store(0, Ordering::Relaxed); // main.c:1894
    drive.zero_crosses.store(zc.saturating_add(1), Ordering::Relaxed); // main.c:1896

    // changeover to interrupt mode (non-stall/non-rc_car path,
    // main.c:1908-1913): commutation_interval < polling_mode_changeover.
    if ci < POLLING_MODE_CHANGEOVER {
        drive.old_routine.store(false, Ordering::Relaxed);
        am32_enable_comp_interrupts();
    }
}

// ===============================================================
// startMotor() — main.c:950-959. Per the directive, comp interrupts are
// NOT enabled here (polling reads level, no EXTI) — the enable happens
// at the polling→interrupt changeover in zcfoundroutine. This is the one
// intentional divergence from AM32's line 958 `enableCompInterrupts()`.
// ===============================================================
#[inline]
pub fn start_motor(sched: &Sched, drive: &Drive, bb: &Bb) {
    if !drive.running.load(Ordering::Relaxed) {
        commutate(sched, drive, bb); // main.c:953
        sched.commutation_interval.store(STARTUP_INTERVAL_TICKS, Ordering::Relaxed); // :954
        set_interval_cnt(5000); // SET_INTERVAL_TIMER_COUNT(5000) main.c:955
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.bemf_counter.store(0, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed); // main.c:956
    }
}

// ===============================================================
// Bench-safety kill: float all legs, latch, mask comp, freeze bb.
// ===============================================================
#[inline]
pub fn safety_kill(drive: &Drive, duty: &Duty, bb: &Bb, reason: u16) {
    tim1_motor_pwm::all_off();
    drive.running.store(false, Ordering::Relaxed);
    duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
    duty.duty_cycle.store(0, Ordering::Relaxed);
    duty.last_duty_cycle.store(0, Ordering::Relaxed);
    drive.old_routine.store(true, Ordering::Relaxed);
    am32_mask_phase_interrupts();
    disable_com_timer_int();
    duty.kill_reason.store(reason, Ordering::Relaxed);
    duty.killed.store(true, Ordering::Relaxed);
    bb.freeze();
}

/// Bench stop-request band: float, disarm, mask, zero the pipeline.
#[inline]
pub fn honor_stop(drive: &Drive, duty: &Duty, bench: &Bench) {
    if bench.stop_req.swap(false, Ordering::Relaxed) {
        tim1_motor_pwm::all_off();
        drive.running.store(false, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(0, Ordering::Relaxed);
        duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
        duty.duty_cycle.store(0, Ordering::Relaxed);
        duty.last_duty_cycle.store(0, Ordering::Relaxed);
        am32_mask_phase_interrupts();
        disable_com_timer_int();
    }
}

/// variable_pwm carrier-ride band (main.c:2192-2195). mode 1: carrier
/// rides the commutation interval; duty ratio is preserved by the tenKhz
/// `duty*tim1_arr/2000` rescale.
#[inline]
pub fn variable_pwm_ride(sched: &Sched) {
    if VARIABLE_PWM == 1 {
        let ci = sched.commutation_interval.load(Ordering::Relaxed) as i32;
        let arr = am32::map(ci, 96, 200, (TIMER1_MAX_ARR / 2) as i32, TIMER1_MAX_ARR as i32);
        tim1_motor_pwm::set_carrier_arr(arr as u16);
    }
}

/// bemf-timeout re-kick band (main.c:2495-2509).
#[inline]
pub fn bemf_timeout_rekick<const N: usize>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    zct: &ZctTrace<N>,
    bb: &Bb,
    running: bool,
) {
    if interval_cnt() > BEMF_TIMEOUT_TICKS && running {
        drive.bemf_timeout_happened.fetch_add(1, Ordering::Relaxed);
        am32_mask_phase_interrupts();
        drive.old_routine.store(true, Ordering::Relaxed);
        if duty.input.load(Ordering::Relaxed) < 48 {
            drive.running.store(false, Ordering::Relaxed);
            sched.commutation_interval.store(5000, Ordering::Relaxed);
        }
        drive.zero_crosses.store(0, Ordering::Relaxed);
        zcfoundroutine(sched, drive, zct, duty, bb);
    }
}

/// desync_check band (main.c:2284-2300) — non-bi-dir subset.
#[inline]
pub fn desync_check_band(sched: &Sched, drive: &Drive, duty: &Duty, bb: &Bb, average_interval: u32) {
    if drive.desync_check.load(Ordering::Relaxed) && drive.zero_crosses.load(Ordering::Relaxed) > 10 {
        let lai = sched.last_average_interval.load(Ordering::Relaxed);
        if am32::desync_due(lai, average_interval) {
            drive.zero_crosses.store(0, Ordering::Relaxed); // main.c:2286
            drive.desync_happened.fetch_add(1, Ordering::Relaxed); // :2287
            // (!bi_direction && input>47) || commutation_interval>1000 → running=0
            let input = duty.input.load(Ordering::Relaxed);
            let ci = sched.commutation_interval.load(Ordering::Relaxed);
            if input > 47 || ci > 1000 {
                drive.running.store(false, Ordering::Relaxed);
            }
            drive.old_routine.store(true, Ordering::Relaxed); // :2291
            duty.last_duty_cycle.store(MIN_STARTUP_DUTY / 2, Ordering::Relaxed); // :2295
            bb.record(EV_DSY, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, average_interval as u16);
        }
        drive.desync_check.store(false, Ordering::Relaxed); // :2297
        sched.last_average_interval.store(average_interval, Ordering::Relaxed); // :2299
    }
}

/// setInput() duty-setpoint block — main.c:1180-1326 factory subset
/// (armed always true; no sine, no brake, no current limit).
#[inline]
pub fn set_input(sched: &Sched, drive: &Drive, duty: &Duty, bb: &Bb) {
    // input = uart_duty_get()  (main.c:1131,1423-1431).
    let input = duty.uart_duty_input.load(Ordering::Relaxed);
    duty.input.store(input, Ordering::Relaxed);
    set_input_arming(sched, drive, duty, bb, input);
    set_input_clamp(drive, duty, input);
}

/// setInput arm/disarm band (main.c:1182-1300, comp_pwm subset).
#[inline]
pub fn set_input_arming(sched: &Sched, drive: &Drive, duty: &Duty, bb: &Bb, input: u16) {
    let running = drive.running.load(Ordering::Relaxed);
    if input >= 47 {
        // main.c:1182-1196
        if !running {
            tim1_motor_pwm::all_off(); // main.c:1184
            if !drive.old_routine.load(Ordering::Relaxed) {
                start_motor(sched, drive, bb); // main.c:1185-1187
            }
            drive.running.store(true, Ordering::Relaxed); // main.c:1188
            duty.last_duty_cycle.store(MIN_STARTUP_DUTY, Ordering::Relaxed); // :1189
        }
    } else {
        // input < 47 (main.c:1203-1300, comp_pwm subset)
        if !running {
            drive.old_routine.store(true, Ordering::Relaxed);
            drive.zero_crosses.store(0, Ordering::Relaxed);
            drive.bad_count.store(0, Ordering::Relaxed);
            tim1_motor_pwm::all_off();
        }
    }
}
