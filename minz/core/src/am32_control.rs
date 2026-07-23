//! AM32 control transliteration — commutation, polling ZC, start/stop,
//! setInput. Line-cited against `Src/main.c`; the pure arithmetic lives
//! in `crate::am32` and the tenKhz/main band fns in `crate::am32_loop`.
//! Hardware is reached ONLY through the [`crate::am32_hal`] traits
//! (static dispatch, no `dyn`): the firmware passes its zero-sized
//! register impls in a [`MotorHal`] bundle (`hal`, rm32's calling
//! convention — `hal.pwm().set_duty_all(..)`) plus the minz-owned
//! [`Observer`] bundle (`obs` — bb/cs/adc/lt); host tests pass mocks —
//! which is what makes every fn here unit-testable on a PC. Every fn
//! takes its world (the cohesion clusters + the bundles it touches) as
//! parameters; the program keeps the storage.

use core::sync::atomic::Ordering;

use crate::am32;
use crate::am32_hal::{
    ComTimer, ComTimerExt, Comparator, Cs, InjAdc, IntervalTimer, LoopTimer, MotorHal, Observer,
    PhaseOutput, PwmOutput, Recorder,
};
use crate::am32_loop::{
    BAD_COUNT_THRESHOLD, BEMF_TIMEOUT_TICKS, Bench, Drive, Duty, MIN_STARTUP_DUTY,
    POLLING_MODE_CHANGEOVER, Sched, STARTUP_INTERVAL_TICKS, TEMP_ADVANCE, VARIABLE_PWM,
    set_input_clamp,
};
use crate::blackbox::{EV_DSY, EV_REF};
use crate::zct_trace::ZctTrace;

// TIM1 base ARR (AM32 targets.h:5335 `TIM1_AUTORELOAD = 3332` on
// L431 @ 24 kHz) arrives as `variable_pwm_ride`'s `base_arr` parameter
// — the firmware value is carrier-feature-dependent, so the program
// threads it in (rm32's PwmOutput has no ARR getters; ARR is state,
// held in `Duty::tim1_arr` like AM32's `tim1_arr` variable).

// ===============================================================
// commutate() — main.c:854-894 (forward-only factory path).
// ===============================================================
#[inline(always)]
pub fn commutate<M: MotorHal>(
    sched: &Sched,
    drive: &Drive,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
) {
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

    // comStep(step) (main.c:876) — rm32's 1..6 step convention at the
    // trait boundary (the firmware impl converts to its 0..5 sector
    // frame internally); role-only flip, CCRs hold the tick-shaped
    // duty. AM32 wraps this in __disable_irq; the firmware impl does
    // its own interrupt::free.
    hal.phase().com_step(step as u8);
    // changeCompInput() (main.c:879) — rm32's two-call shape: set_step
    // stores the (step, rising) pair, change_input applies the mux +
    // EXTI edge from it. Same register ops, same order as the old
    // single-call change_comp_input(sector).
    hal.comp().set_step(step as u8, rising);
    hal.comp().change_input();

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

    obs.bb.record(EV_REF, sector as u8, sched.commutation_interval.load(Ordering::Relaxed) as u16);
}

// ===============================================================
// zcfoundroutine() — main.c:1868-1915 (polling mode, blocking).
// ===============================================================

/// zcfoundroutine blend band (main.c:1870-1874): capture thiszctime,
/// reset INTERVAL_TIMER, blend commutation_interval, derive advance/waitTime.
#[inline]
pub fn zcfr_blend<M: MotorHal>(sched: &Sched, hal: &mut M) -> (u32, u32) {
    // thiszctime = INTERVAL_TIMER_COUNT; SET_INTERVAL_TIMER_COUNT(0)
    let thiszc = hal.interval().count() as u16; // main.c:1870
    hal.interval().set_count(0); // main.c:1871
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
pub fn zcfr_spin_wait<M: MotorHal>(drive: &Drive, zc: u32, wait: u32, hal: &mut M) {
    let mut guard: u32 = 0;
    loop {
        if hal.interval().count() >= wait {
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
pub fn zcfoundroutine<const N: usize, M: MotorHal<Com: ComTimerExt>>(
    sched: &Sched,
    drive: &Drive,
    zct: &ZctTrace<N>,
    duty: &Duty,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
) {
    let (ci, wait) = zcfr_blend(sched, hal); // main.c:1870-1874
    let zc = drive.zero_crosses.load(Ordering::Relaxed);
    zcfr_spin_wait(drive, zc, wait, hal); // main.c:1875-1879

    hal.com_timer().com_set_arr(wait as u16); // COM_TIMER->ARR = waitTime (main.c:1884)
    commutate(sched, drive, hal, obs); // main.c:1889
    zct.write(sched, drive, duty, obs.cs); // ZC_TRACE main.c:1891
    drive.bemf_counter.store(0, Ordering::Relaxed); // main.c:1893
    drive.bad_count.store(0, Ordering::Relaxed); // main.c:1894
    drive.zero_crosses.store(zc.saturating_add(1), Ordering::Relaxed); // main.c:1896

    // changeover to interrupt mode (non-stall/non-rc_car path,
    // main.c:1908-1913): commutation_interval < polling_mode_changeover.
    if ci < POLLING_MODE_CHANGEOVER {
        drive.old_routine.store(false, Ordering::Relaxed);
        hal.comp().enable_interrupts();
    }
}

// ===============================================================
// startMotor() — main.c:950-959. Per the directive, comp interrupts are
// NOT enabled here (polling reads level, no EXTI) — the enable happens
// at the polling→interrupt changeover in zcfoundroutine. This is the one
// intentional divergence from AM32's line 958 `enableCompInterrupts()`.
// ===============================================================
#[inline]
pub fn start_motor<M: MotorHal>(
    sched: &Sched,
    drive: &Drive,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
) {
    if !drive.running.load(Ordering::Relaxed) {
        commutate(sched, drive, hal, obs); // main.c:953
        sched.commutation_interval.store(STARTUP_INTERVAL_TICKS, Ordering::Relaxed); // :954
        hal.interval().set_count(5000); // SET_INTERVAL_TIMER_COUNT(5000) main.c:955
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.bemf_counter.store(0, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed); // main.c:956
    }
}

// ===============================================================
// Bench-safety kill: float all legs, latch, mask comp, freeze bb.
// ===============================================================
#[inline]
pub fn safety_kill<M: MotorHal>(
    drive: &Drive,
    duty: &Duty,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
    reason: u16,
) {
    hal.phase().all_off();
    drive.running.store(false, Ordering::Relaxed);
    duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
    duty.duty_cycle.store(0, Ordering::Relaxed);
    duty.last_duty_cycle.store(0, Ordering::Relaxed);
    drive.old_routine.store(true, Ordering::Relaxed);
    hal.comp().mask_interrupts();
    hal.com_timer().disable_interrupt();
    duty.kill_reason.store(reason, Ordering::Relaxed);
    duty.killed.store(true, Ordering::Relaxed);
    obs.bb.freeze();
}

/// Bench stop-request band: float, disarm, mask, zero the pipeline.
#[inline]
pub fn honor_stop<M: MotorHal>(drive: &Drive, duty: &Duty, bench: &Bench, hal: &mut M) {
    if bench.stop_req.swap(false, Ordering::Relaxed) {
        hal.phase().all_off();
        drive.running.store(false, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(0, Ordering::Relaxed);
        duty.duty_cycle_setpoint.store(0, Ordering::Relaxed);
        duty.duty_cycle.store(0, Ordering::Relaxed);
        duty.last_duty_cycle.store(0, Ordering::Relaxed);
        hal.comp().mask_interrupts();
        hal.com_timer().disable_interrupt();
    }
}

/// variable_pwm carrier-ride band (main.c:2192-2195). mode 1: carrier
/// rides the commutation interval; duty ratio is preserved by the tenKhz
/// `duty*tim1_arr/2000` rescale. `base_arr` (AM32 `TIMER1_MAX_ARR`,
/// targets.h:5335) is threaded in by the program. Stores the mapped ARR
/// into `duty.tim1_arr` — AM32's `tim1_arr = map(...)` (main.c:2194) —
/// so `duty_apply` rescales against the VARIABLE, then writes the same
/// value to the register (`SET_AUTO_RELOAD_PWM`).
#[inline]
pub fn variable_pwm_ride<M: MotorHal>(sched: &Sched, duty: &Duty, base_arr: u16, hal: &mut M) {
    if VARIABLE_PWM == 1 {
        let ci = sched.commutation_interval.load(Ordering::Relaxed) as i32;
        let arr = am32::map(ci, 96, 200, (base_arr / 2) as i32, base_arr as i32);
        duty.tim1_arr.store(arr as u16, Ordering::Relaxed);
        hal.pwm().set_auto_reload(arr as u16);
    }
}

/// bemf-timeout re-kick band (main.c:2495-2509).
#[inline]
pub fn bemf_timeout_rekick<const N: usize, M: MotorHal<Com: ComTimerExt>>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    zct: &ZctTrace<N>,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
    running: bool,
) {
    if hal.interval().count() > BEMF_TIMEOUT_TICKS && running {
        drive.bemf_timeout_happened.fetch_add(1, Ordering::Relaxed);
        hal.comp().mask_interrupts();
        drive.old_routine.store(true, Ordering::Relaxed);
        if duty.input.load(Ordering::Relaxed) < 48 {
            drive.running.store(false, Ordering::Relaxed);
            sched.commutation_interval.store(5000, Ordering::Relaxed);
        }
        drive.zero_crosses.store(0, Ordering::Relaxed);
        zcfoundroutine(sched, drive, zct, duty, hal, obs);
    }
}

/// desync_check band (main.c:2284-2300) — non-bi-dir subset.
#[inline]
pub fn desync_check_band(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
    average_interval: u32,
) {
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
            obs.bb.record(EV_DSY, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, average_interval as u16);
        }
        drive.desync_check.store(false, Ordering::Relaxed); // :2297
        sched.last_average_interval.store(average_interval, Ordering::Relaxed); // :2299
    }
}

/// setInput() duty-setpoint block — main.c:1180-1326 factory subset
/// (armed always true; no sine, no brake, no current limit).
#[inline]
pub fn set_input<M: MotorHal>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
) {
    // input = uart_duty_get()  (main.c:1131,1423-1431).
    let input = duty.uart_duty_input.load(Ordering::Relaxed);
    duty.input.store(input, Ordering::Relaxed);
    set_input_arming(sched, drive, duty, hal, obs, input);
    set_input_clamp(drive, duty, input);
}

/// setInput arm/disarm band (main.c:1182-1300, comp_pwm subset).
#[inline]
pub fn set_input_arming<M: MotorHal>(
    sched: &Sched,
    drive: &Drive,
    duty: &Duty,
    hal: &mut M,
    obs: &Observer<'_, impl Recorder, impl Cs, impl InjAdc, impl LoopTimer>,
    input: u16,
) {
    let running = drive.running.load(Ordering::Relaxed);
    if input >= 47 {
        // main.c:1182-1196
        if !running {
            hal.phase().all_off(); // main.c:1184
            if !drive.old_routine.load(Ordering::Relaxed) {
                start_motor(sched, drive, hal, obs); // main.c:1185-1187
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
            hal.phase().all_off();
        }
    }
}

/// getBemfState() — main.c:817-852 (L431 `!getCompOutputLevel()` branch,
/// which equals minz `comp2::value()` = our `output_level()`). Counts when
/// the level matches the direction; a run of bad reads over threshold
/// resets the counter.
#[inline]
pub fn get_bemf_state(drive: &Drive, comp: &impl Comparator) {
    let cs = comp.output_level(); // = !getCompOutputLevel() (main.c:831)
    let rising = drive.rising.load(Ordering::Relaxed);
    // rising: count when current_state; else count when !current_state.
    // Both reduce to `cs == rising` (main.c:833-851).
    let (bemf, bad) = am32::bemf_count_step(
        drive.bemf_counter.load(Ordering::Relaxed),
        drive.bad_count.load(Ordering::Relaxed),
        cs == rising,
        BAD_COUNT_THRESHOLD,
    );
    drive.bemf_counter.store(bemf, Ordering::Relaxed);
    drive.bad_count.store(bad, Ordering::Relaxed);
}

// ===============================================================
// Host tests — rm32-style HAL mocks with call recording (the payoff
// of the trait seam: every safety-relevant HAL call is asserted).
// ===============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::am32_hal::mock::{BenchStore, DriveStore, DutyStore, MockHal, SchedStore, ZctStore};
    use crate::am32_loop::{DUTY_FULL, INIT_INTERVAL_TICKS, STARTUP_MAX_DUTY_CYCLE};

    // The mock HAL (all seven seams on one struct, ordered call log)
    // + the owned-atomics cluster fixtures live in
    // `crate::am32_hal::mock`, shared with the am32_isr tests.

    // --- commutate --------------------------------------------------

    #[test]
    fn commutate_wraps_step_and_flags_desync_check() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        let (mut hal, obs) = (m.motor(), m.observer());
        drive.current_step.store(6, Ordering::Relaxed);
        sched.commutation_interval.store(746, Ordering::Relaxed);
        commutate(&sched, &drive, &mut hal, &obs);
        // 6→1 wrap sets desync_check (main.c:856-861).
        assert_eq!(drive.current_step.load(Ordering::Relaxed), 1);
        assert!(drive.desync_check.load(Ordering::Relaxed));
        // rising = step % 2 → step 1 is rising.
        assert!(drive.rising.load(Ordering::Relaxed));
        // comStep gets the AM32 STEP (1..6, main.c:876); set_step gets
        // the SAME step + rising pair (rung 3 two-call shape) and
        // change_input follows it (the -1 sector conversion lives in
        // the firmware impl).
        assert_eq!(*m.roles.borrow(), [1]);
        assert_eq!(*m.steps.borrow(), [(1u8, true)]);
        assert_eq!(
            m.calls.borrow().iter().position(|c| *c == "set_step").unwrap() + 1,
            m.calls.borrow().iter().position(|c| *c == "change_input").unwrap()
        );
        // bemfcounter/zcfound reset; interval pushed into slot 0.
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 0);
        assert!(!drive.zcfound.load(Ordering::Relaxed));
        assert_eq!(ss.interval_hist[0].load(Ordering::Relaxed), 746);
        // EV_REF recorded with sector + interval.
        assert_eq!(*m.events.borrow(), [(EV_REF, 0, 746)]);
    }

    #[test]
    fn commutate_no_wrap_even_step_and_polling_fallback() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        let (mut hal, obs) = (m.motor(), m.observer());
        drive.current_step.store(1, Ordering::Relaxed);
        commutate(&sched, &drive, &mut hal, &obs);
        // 1→2: no desync_check, even step is falling — and set_step
        // carries the falling pair (rung 3 coverage).
        assert_eq!(drive.current_step.load(Ordering::Relaxed), 2);
        assert!(!drive.desync_check.load(Ordering::Relaxed));
        assert!(!drive.rising.load(Ordering::Relaxed));
        assert_eq!(m.steps.borrow()[0], (2u8, false));
        assert!(!drive.old_routine.load(Ordering::Relaxed));
        // average_interval > changeover+500 → polling fallback (main.c:881).
        sched.average_interval.store(POLLING_MODE_CHANGEOVER + 501, Ordering::Relaxed);
        commutate(&sched, &drive, &mut hal, &obs);
        assert!(drive.old_routine.load(Ordering::Relaxed));
        // At exactly changeover+500: NOT (strict >).
        drive.old_routine.store(false, Ordering::Relaxed);
        sched.average_interval.store(POLLING_MODE_CHANGEOVER + 500, Ordering::Relaxed);
        commutate(&sched, &drive, &mut hal, &obs);
        assert!(!drive.old_routine.load(Ordering::Relaxed));
    }

    // --- zcfoundroutine bands --------------------------------------

    #[test]
    fn zcfr_blend_matches_polling_blend_and_stores_wait() {
        let (ss, m) = (SchedStore::default(), MockHal::new());
        let (sched, mut hal) = (ss.sched(), m.motor());
        m.interval.set(700); // thiszctime
        sched.commutation_interval.store(746, Ordering::Relaxed);
        let (ci, wait) = zcfr_blend(&sched, &mut hal);
        // ci = (thiszc + 3*ci)/4 (main.c:1872), via am32::polling_blend.
        assert_eq!(ci, am32::polling_blend(700, 746));
        assert_eq!(sched.commutation_interval.load(Ordering::Relaxed), ci);
        assert_eq!(sched.this_zc.load(Ordering::Relaxed), 700);
        // INTERVAL_TIMER reset (main.c:1871).
        assert!(m.called("set_interval_cnt"));
        assert_eq!(m.interval.get(), 0);
        // wait = ci/2 - advance stored as u16 (main.c:1873-4).
        let advance = am32::advance_of(ci, TEMP_ADVANCE);
        assert_eq!(wait, am32::wait_time(ci, advance));
        assert_eq!(sched.wait_time.load(Ordering::Relaxed), wait as u16);
    }

    #[test]
    fn zcfr_spin_wait_breaks_on_cnt_reaching_wait() {
        let (ds, m) = (DriveStore::default(), MockHal::new());
        let (drive, mut hal) = (ds.drive(), m.motor());
        m.interval.set(0);
        m.interval_step.set(100); // CNT climbs 100/read
        zcfr_spin_wait(&drive, 5, 250, &mut hal);
        // Reads 0,100,200,300 → breaks at 300 ≥ 250; guard untouched.
        assert_eq!(m.interval.get(), 400);
        assert_eq!(drive.zcfr_guard_hits.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zcfr_spin_wait_breaks_immediately_below_five_zcs() {
        let (ds, m) = (DriveStore::default(), MockHal::new());
        let (drive, mut hal) = (ds.drive(), m.motor());
        m.interval.set(0); // stuck below wait
        zcfr_spin_wait(&drive, 4, 1000, &mut hal);
        // zc<5 breaks after ONE interval read (main.c:1877).
        assert_eq!(drive.zcfr_guard_hits.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zcfr_spin_wait_guard_trips_on_wedged_timer() {
        let (ds, m) = (DriveStore::default(), MockHal::new());
        let (drive, mut hal) = (ds.drive(), m.motor());
        m.interval.set(0); // wedged INTERVAL_TIMER (DEVIATION #1)
        zcfr_spin_wait(&drive, 5, 1000, &mut hal);
        assert_eq!(drive.zcfr_guard_hits.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn zcfoundroutine_full_sequence_with_changeover() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let (mut hal, obs) = (m.motor(), m.observer());
        // thiszc=100, ci_old=100 → ci=100 < POLLING_MODE_CHANGEOVER.
        m.interval.set(100);
        m.interval_step.set(1); // let the spin-wait terminate honestly
        sched.commutation_interval.store(100, Ordering::Relaxed);
        drive.zero_crosses.store(7, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.current_step.store(1, Ordering::Relaxed);
        drive.bad_count.store(3, Ordering::Relaxed);
        zcfoundroutine(&sched, &drive, &zct, &duty, &mut hal, &obs);
        // COM_TIMER->ARR = waitTime (main.c:1884).
        let wait = sched.wait_time.load(Ordering::Relaxed);
        assert_eq!(*m.com_arrs.borrow(), [wait]);
        // commutate ran (roles + EV_REF).
        assert_eq!(m.roles.borrow().len(), 1);
        assert_eq!(m.events.borrow().len(), 1);
        // ZC_TRACE row written.
        assert_eq!(zs.records(), 1);
        // counters (main.c:1893-1896).
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 0);
        assert_eq!(drive.bad_count.load(Ordering::Relaxed), 0);
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 8);
        // changeover: ci < 2000 → interrupt mode + comp ints ON.
        assert!(!drive.old_routine.load(Ordering::Relaxed));
        assert!(m.called("enable_interrupts"));
    }

    #[test]
    fn zcfoundroutine_no_changeover_at_slow_interval() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let (mut hal, obs) = (m.motor(), m.observer());
        // thiszc=4000, ci_old=4000 → ci=4000 ≥ POLLING_MODE_CHANGEOVER.
        m.interval.set(4000);
        sched.commutation_interval.store(4000, Ordering::Relaxed);
        drive.zero_crosses.store(0, Ordering::Relaxed); // zc<5 skips the spin
        drive.old_routine.store(true, Ordering::Relaxed);
        zcfoundroutine(&sched, &drive, &zct, &duty, &mut hal, &obs);
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert!(!m.called("enable_interrupts"));
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 1);
    }

    // --- start_motor ------------------------------------------------

    #[test]
    fn start_motor_seeds_only_when_not_running() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        let (mut hal, obs) = (m.motor(), m.observer());
        sched.commutation_interval.store(INIT_INTERVAL_TICKS, Ordering::Relaxed);
        drive.bemf_counter.store(9, Ordering::Relaxed);
        start_motor(&sched, &drive, &mut hal, &obs);
        // commutate ran, then the seeds (main.c:953-956).
        assert_eq!(m.roles.borrow().len(), 1);
        assert_eq!(
            sched.commutation_interval.load(Ordering::Relaxed),
            STARTUP_INTERVAL_TICKS
        );
        assert!(m.called("set_interval_cnt"));
        assert_eq!(m.interval.get(), 5000); // SET_INTERVAL_TIMER_COUNT(5000)
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 0);
        assert!(drive.running.load(Ordering::Relaxed));
        // Already running: a second call is a no-op.
        m.clear_calls();
        sched.commutation_interval.store(777, Ordering::Relaxed);
        start_motor(&sched, &drive, &mut hal, &obs);
        assert!(m.calls.borrow().is_empty());
        assert_eq!(sched.commutation_interval.load(Ordering::Relaxed), 777);
    }

    // --- safety_kill / honor_stop ----------------------------------

    #[test]
    fn safety_kill_full_off_mask_disable_freeze_latch() {
        let (ds, us, m) = (DriveStore::default(), DutyStore::default(), MockHal::new());
        let (drive, duty) = (ds.drive(), us.duty());
        let (mut hal, obs) = (m.motor(), m.observer());
        drive.running.store(true, Ordering::Relaxed);
        duty.duty_cycle_setpoint.store(500, Ordering::Relaxed);
        duty.duty_cycle.store(400, Ordering::Relaxed);
        duty.last_duty_cycle.store(300, Ordering::Relaxed);
        safety_kill(&drive, &duty, &mut hal, &obs, 2);
        // The rm32-style HAL-call assertions: every safety call fired.
        assert!(m.called("all_off"));
        assert!(m.called("mask_interrupts"));
        assert!(m.called("disable_com_timer_int"));
        assert!(m.frozen.get());
        // Pipeline zeroed + latched kill.
        assert!(!drive.running.load(Ordering::Relaxed));
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert_eq!(duty.duty_cycle_setpoint.load(Ordering::Relaxed), 0);
        assert_eq!(duty.duty_cycle.load(Ordering::Relaxed), 0);
        assert_eq!(duty.last_duty_cycle.load(Ordering::Relaxed), 0);
        assert_eq!(duty.kill_reason.load(Ordering::Relaxed), 2);
        assert!(duty.killed.load(Ordering::Relaxed));
    }

    #[test]
    fn honor_stop_swaps_flag_and_acts_once() {
        let (ds, us, bs, m) = (
            DriveStore::default(),
            DutyStore::default(),
            BenchStore::default(),
            MockHal::new(),
        );
        let (drive, duty, bench, mut hal) = (ds.drive(), us.duty(), bs.bench(), m.motor());
        drive.running.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(42, Ordering::Relaxed);
        duty.duty_cycle_setpoint.store(500, Ordering::Relaxed);
        bench.stop_req.store(true, Ordering::Relaxed);
        honor_stop(&drive, &duty, &bench, &mut hal);
        // Swap semantics: flag consumed, actions fired.
        assert!(!bench.stop_req.load(Ordering::Relaxed));
        assert!(m.called("all_off"));
        assert!(m.called("mask_interrupts"));
        assert!(m.called("disable_com_timer_int"));
        assert!(!drive.running.load(Ordering::Relaxed));
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 0);
        assert_eq!(duty.duty_cycle_setpoint.load(Ordering::Relaxed), 0);
        // No pending request → no-op.
        m.clear_calls();
        honor_stop(&drive, &duty, &bench, &mut hal);
        assert!(m.calls.borrow().is_empty());
    }

    // --- variable_pwm_ride -----------------------------------------

    #[test]
    fn variable_pwm_ride_maps_interval_to_carrier_endpoints() {
        let (ss, us, m) = (SchedStore::default(), DutyStore::default(), MockHal::new());
        let (sched, duty, mut hal) = (ss.sched(), us.duty(), m.motor());
        // ci ≤ 96 → half the base ARR (main.c:2193 map low end).
        sched.commutation_interval.store(50, Ordering::Relaxed);
        variable_pwm_ride(&sched, &duty, 3332, &mut hal);
        assert_eq!(duty.tim1_arr.load(Ordering::Relaxed), 3332 / 2);
        // ci ≥ 200 → full base ARR.
        sched.commutation_interval.store(500, Ordering::Relaxed);
        variable_pwm_ride(&sched, &duty, 3332, &mut hal);
        assert_eq!(duty.tim1_arr.load(Ordering::Relaxed), 3332);
        assert_eq!(*m.carrier_arrs.borrow(), [3332 / 2, 3332]);
    }

    #[test]
    fn variable_pwm_shadow_feeds_duty_apply() {
        // The rung-2 coupling: variable_pwm_ride stores the mapped ARR
        // in the tim1_arr shadow, and duty_apply's rescale consumes the
        // SHADOW — AM32's `duty*tim1_arr/2000` uses the VARIABLE, not a
        // register readback (main.c:1790-1791).
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let mut hal = m.motor();
        sched.commutation_interval.store(50, Ordering::Relaxed); // → ARR 1666
        variable_pwm_ride(&sched, &duty, 3332, &mut hal);
        assert_eq!(duty.tim1_arr.load(Ordering::Relaxed), 1666);
        crate::am32_isr::duty_apply(&drive, &duty, 1000, &mut hal);
        // base = 1000 * 1666 / 2000 = 833 (not running: no +1).
        assert_eq!(*m.duties.borrow(), [833]);
        // Both the ride and the apply wrote the SAME shadow value to ARR.
        assert_eq!(*m.carrier_arrs.borrow(), [1666, 1666]);
    }

    // --- bemf_timeout_rekick ---------------------------------------

    #[test]
    fn bemf_timeout_rekick_fires_only_over_threshold_and_running() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let (mut hal, obs) = (m.motor(), m.observer());
        // Below threshold: nothing.
        m.interval.set(BEMF_TIMEOUT_TICKS);
        bemf_timeout_rekick(&sched, &drive, &duty, &zct, &mut hal, &obs, true);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 0);
        // Over threshold but not running: nothing.
        m.interval.set(46_000);
        bemf_timeout_rekick(&sched, &drive, &duty, &zct, &mut hal, &obs, false);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 0);
        assert!(!m.called("mask_interrupts"));
    }

    #[test]
    fn bemf_timeout_rekick_high_input_keeps_running() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let (mut hal, obs) = (m.motor(), m.observer());
        m.interval.set(46_000);
        sched.commutation_interval.store(1000, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed);
        duty.input.store(100, Ordering::Relaxed); // ≥ 48
        drive.zero_crosses.store(50, Ordering::Relaxed);
        bemf_timeout_rekick(&sched, &drive, &duty, &zct, &mut hal, &obs, true);
        assert_eq!(drive.bemf_timeout_happened.load(Ordering::Relaxed), 1);
        assert!(m.called("mask_interrupts"));
        assert!(drive.old_routine.load(Ordering::Relaxed));
        // input ≥ 48 → running preserved, no 5000 reseed; blend uses
        // the surviving ci: (46000 + 3*1000)/4.
        assert!(drive.running.load(Ordering::Relaxed));
        assert_eq!(
            sched.commutation_interval.load(Ordering::Relaxed),
            am32::polling_blend(46_000, 1000)
        );
        // zero_crosses reset to 0 then incremented by zcfoundroutine.
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn bemf_timeout_rekick_low_input_kills_running_and_reseeds() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let (mut hal, obs) = (m.motor(), m.observer());
        m.interval.set(46_000);
        sched.commutation_interval.store(1000, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed);
        duty.input.store(40, Ordering::Relaxed); // < 48
        bemf_timeout_rekick(&sched, &drive, &duty, &zct, &mut hal, &obs, true);
        assert!(!drive.running.load(Ordering::Relaxed));
        // ci reseeded to 5000 BEFORE the blend (main.c:2503):
        // final ci = polling_blend(thiszc=46000, 5000) = 15250.
        assert_eq!(
            sched.commutation_interval.load(Ordering::Relaxed),
            am32::polling_blend(46_000, 5000)
        );
    }

    // --- desync_check_band -----------------------------------------

    #[test]
    fn desync_check_band_trips_resets_and_records() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let obs = m.observer();
        drive.desync_check.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(11, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed);
        drive.current_step.store(4, Ordering::Relaxed);
        sched.last_average_interval.store(1600, Ordering::Relaxed);
        sched.commutation_interval.store(500, Ordering::Relaxed);
        duty.input.store(100, Ordering::Relaxed); // > 47 → running=0
        desync_check_band(&sched, &drive, &duty, &obs, 1000);
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 0);
        assert_eq!(drive.desync_happened.load(Ordering::Relaxed), 1);
        assert!(!drive.running.load(Ordering::Relaxed));
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert_eq!(
            duty.last_duty_cycle.load(Ordering::Relaxed),
            MIN_STARTUP_DUTY / 2
        );
        // EV_DSY with sector = step-1 and the average interval.
        assert_eq!(*m.events.borrow(), [(EV_DSY, 3, 1000)]);
        // Bookkeeping (main.c:2297-2299).
        assert!(!drive.desync_check.load(Ordering::Relaxed));
        assert_eq!(sched.last_average_interval.load(Ordering::Relaxed), 1000);
    }

    #[test]
    fn desync_check_band_low_input_slow_ci_keeps_running() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let obs = m.observer();
        drive.desync_check.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(11, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed);
        drive.current_step.store(1, Ordering::Relaxed);
        sched.last_average_interval.store(1600, Ordering::Relaxed);
        sched.commutation_interval.store(500, Ordering::Relaxed); // ≤ 1000
        duty.input.store(0, Ordering::Relaxed); // ≤ 47
        desync_check_band(&sched, &drive, &duty, &obs, 1000);
        // Desync registered but running survives (main.c:2288-2290).
        assert_eq!(drive.desync_happened.load(Ordering::Relaxed), 1);
        assert!(drive.running.load(Ordering::Relaxed));
    }

    #[test]
    fn desync_check_band_not_due_clears_flag_only() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let obs = m.observer();
        drive.desync_check.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(11, Ordering::Relaxed);
        sched.last_average_interval.store(1000, Ordering::Relaxed);
        desync_check_band(&sched, &drive, &duty, &obs, 1000);
        // No desync, but flag cleared + lai updated (main.c:2297-2299).
        assert_eq!(drive.desync_happened.load(Ordering::Relaxed), 0);
        assert!(m.events.borrow().is_empty());
        assert!(!drive.desync_check.load(Ordering::Relaxed));
        // Gate closed entirely (desync_check false): lai NOT rewritten.
        sched.last_average_interval.store(1234, Ordering::Relaxed);
        desync_check_band(&sched, &drive, &duty, &obs, 999);
        assert_eq!(sched.last_average_interval.load(Ordering::Relaxed), 1234);
    }

    // --- set_input / set_input_arming ------------------------------

    #[test]
    fn set_input_arms_and_starts_motor_when_interrupt_mode() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let (mut hal, obs) = (m.motor(), m.observer());
        duty.duty_cycle_maximum.store(DUTY_FULL, Ordering::Relaxed);
        duty.uart_duty_input.store(1047, Ordering::Relaxed);
        drive.old_routine.store(false, Ordering::Relaxed); // interrupt mode
        set_input(&sched, &drive, &duty, &mut hal, &obs);
        assert_eq!(duty.input.load(Ordering::Relaxed), 1047);
        // Arm path: allOff then startMotor (main.c:1184-1189).
        assert!(m.called("all_off"));
        assert_eq!(m.roles.borrow().len(), 1); // start_motor→commutate
        assert_eq!(
            sched.commutation_interval.load(Ordering::Relaxed),
            STARTUP_INTERVAL_TICKS
        );
        assert!(drive.running.load(Ordering::Relaxed));
        assert_eq!(duty.last_duty_cycle.load(Ordering::Relaxed), MIN_STARTUP_DUTY);
        // Setpoint through the clamp band (startup window, zc=1 after
        // commutate... zc stays 0 here) → clamped to startup range.
        let sp = duty.duty_cycle_setpoint.load(Ordering::Relaxed);
        assert!((MIN_STARTUP_DUTY..=STARTUP_MAX_DUTY_CYCLE).contains(&sp));
    }

    #[test]
    fn set_input_arming_old_routine_skips_start_motor() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let (mut hal, obs) = (m.motor(), m.observer());
        sched.commutation_interval.store(INIT_INTERVAL_TICKS, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed); // polling mode
        set_input_arming(&sched, &drive, &duty, &mut hal, &obs, 1047);
        // all_off yes, but NO commutate/startMotor (main.c:1185 gate).
        assert!(m.called("all_off"));
        assert!(m.roles.borrow().is_empty());
        assert_eq!(sched.commutation_interval.load(Ordering::Relaxed), INIT_INTERVAL_TICKS);
        // Still armed.
        assert!(drive.running.load(Ordering::Relaxed));
        assert_eq!(duty.last_duty_cycle.load(Ordering::Relaxed), MIN_STARTUP_DUTY);
        // Armed + already running: second call is a no-op.
        m.clear_calls();
        set_input_arming(&sched, &drive, &duty, &mut hal, &obs, 1047);
        assert!(m.calls.borrow().is_empty());
    }

    #[test]
    fn set_input_arming_disarm_resets_when_not_running() {
        let (ss, ds, us, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            MockHal::new(),
        );
        let (sched, drive, duty) = (ss.sched(), ds.drive(), us.duty());
        let (mut hal, obs) = (m.motor(), m.observer());
        drive.zero_crosses.store(42, Ordering::Relaxed);
        drive.bad_count.store(3, Ordering::Relaxed);
        set_input_arming(&sched, &drive, &duty, &mut hal, &obs, 0);
        assert!(drive.old_routine.load(Ordering::Relaxed));
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 0);
        assert_eq!(drive.bad_count.load(Ordering::Relaxed), 0);
        assert!(m.called("all_off"));
        // Disarm input while RUNNING: the band leaves state alone
        // (deceleration is the ramp's job, main.c:1203 subset).
        m.clear_calls();
        drive.running.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(9, Ordering::Relaxed);
        set_input_arming(&sched, &drive, &duty, &mut hal, &obs, 0);
        assert!(m.calls.borrow().is_empty());
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 9);
    }

    // --- get_bemf_state --------------------------------------------

    #[test]
    fn get_bemf_state_counts_matches_and_resets_on_bad_run() {
        let (ds, m) = (DriveStore::default(), MockHal::new());
        let drive = ds.drive();
        // Level matches direction (rising, value=true): counter climbs.
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(true);
        get_bemf_state(&drive, &&m);
        get_bemf_state(&drive, &&m);
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 2);
        assert_eq!(drive.bad_count.load(Ordering::Relaxed), 0);
        // Mismatch: bad_count climbs, counter held until the
        // threshold run, then resets (main.c:833-851).
        m.comp_value.set(false);
        for _ in 0..BAD_COUNT_THRESHOLD {
            get_bemf_state(&drive, &&m);
        }
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 2); // held
        get_bemf_state(&drive, &&m); // bad run exceeds threshold
        assert_eq!(drive.bemf_counter.load(Ordering::Relaxed), 0); // reset
        assert_eq!(
            drive.bad_count.load(Ordering::Relaxed),
            BAD_COUNT_THRESHOLD + 1
        );
    }

    // --- zct write (batch gate + cs-guarded push), via the fixtures --

    #[test]
    fn zct_write_records_and_batch_skips() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        let obs = m.observer();
        drive.current_step.store(3, Ordering::Relaxed);
        sched.commutation_interval.store(300, Ordering::Relaxed); // above batch CI
        sched.this_zc.store(0x1234, Ordering::Relaxed);
        zct.write(&sched, &drive, &duty, obs.cs);
        assert_eq!(zs.records(), 1);
        assert_eq!(zs.comm_n.load(Ordering::Relaxed), 1);
        // Wire bytes: 5B A9 sync + step in the flags byte.
        assert_eq!(zs.ring[0][0].load(Ordering::Relaxed), 0x5B);
        assert_eq!(zs.ring[0][1].load(Ordering::Relaxed), 0xA9);
        assert_eq!(zs.ring[0][2].load(Ordering::Relaxed) & 0x07, 3);
        // Batch skip: at a boundary (n=50) with ci below the batch
        // threshold, cycle 1 (odd) is the skipped half — no record.
        zs.comm_n.store(50, Ordering::Relaxed);
        sched.commutation_interval.store(150, Ordering::Relaxed);
        zct.write(&sched, &drive, &duty, obs.cs);
        assert_eq!(zs.records(), 1); // unchanged
        assert!(zs.batching.load(Ordering::Relaxed));
    }
}
