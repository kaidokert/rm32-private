//! AM32 ISR servicing bodies — COMP / COM(TIM16) / TIM6 tenKhz. The
//! `#[interrupt]` trampolines stay with the program (vector table in
//! `minz/examples/am32_clone.rs`); these take their world (the
//! cohesion clusters + `&Hal`) as parameters and reach hardware ONLY
//! through the [`crate::am32_hal`] traits (static dispatch, no `dyn`)
//! — which is what makes every body host-testable below. Bodies moved
//! verbatim; line-cited against AM32 `Src/main.c` +
//! `Mcu/l431/Src/stm32l4xx_it.c`.

use core::sync::atomic::Ordering;

use crate::am32;
use crate::am32_control::{commutate, get_bemf_state, safety_kill, zcfoundroutine};
use crate::am32_hal::{
    CompCtl, ComTimer, ComTimerExt, Cs, Hal, InjAdc, IntervalTimer, LoopTimer, MotorPwm, Recorder,
};
use crate::am32_loop::{
    Bench, Drive, Duty, Sched, TEMP_ADVANCE, duty_ramp, uart_deadman_tick,
};
use crate::blackbox::EV_ACC;
use crate::zct_trace::ZctTrace;

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
pub fn comp_isr(
    sched: &Sched,
    drive: &Drive,
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) {
    if hal.comp.exti_pending() {
        // if INTERVAL_TIMER->CNT > average_interval>>1  (it.c:280)
        if hal.interval.count() > (sched.average_interval.load(Ordering::Relaxed) >> 1) {
            hal.comp.clear_pending(); // it.c:281
            interrupt_routine(sched, drive, hal); // it.c:282
        } else {
            // gate closed: clear ONLY if the level sits at the pre-ZC
            // level (AM32: getCompOutputLevel()==rising; minz-inverted →
            // comp2::value() != rising). Else LEAVE PENDING (their camp:
            // a post-ZC crossing re-fires until the gate opens).
            if hal.comp.value() != drive.rising.load(Ordering::Relaxed) {
                hal.comp.clear_pending(); // it.c:284-285
            }
        }
    }
}

/// interruptRoutine — main.c:918-948.
#[inline]
pub fn interrupt_routine(
    sched: &Sched,
    drive: &Drive,
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) {
    // persistence: reject while the level is still pre-ZC (main.c:932-940;
    // `getCompOutputLevel()==rising` → return, inverted to `value != rising`).
    let filter = drive.filter_level.load(Ordering::Relaxed);
    let rising = drive.rising.load(Ordering::Relaxed);
    for _ in 0..filter {
        if hal.comp.value() != rising {
            return;
        }
    }
    // `cs` copied out of the bundle first: the closure below mutates
    // `hal.interval`/`hal.com`, which cannot overlap a live borrow of
    // `hal.cs` (the ref itself is Copy, so this ends the borrow).
    let cs = hal.cs;
    cs.free(|| {
        hal.comp.mask_phase_interrupts(); // main.c:942
        sched.last_zc.store(sched.this_zc.load(Ordering::Relaxed), Ordering::Relaxed); // :943
        let t = hal.interval.count() as u16; // :944 thiszctime = INTERVAL_TIMER_COUNT
        sched.this_zc.store(t, Ordering::Relaxed);
        hal.interval.set_count(0); // :945
        hal.com
            .set_and_enable(sched.wait_time.load(Ordering::Relaxed).wrapping_add(1)); // :946
    });
    hal.bb.record(EV_ACC, (drive.current_step.load(Ordering::Relaxed) - 1) as u8, sched.this_zc.load(Ordering::Relaxed));
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
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) {
    hal.com.com_clear_flag(); // ack TIM16 UIF (TIM1.UIE is off, so this is the COM tick)
    hal.com.disable_interrupt(); // main.c:898
    commutate(sched, drive, hal); // :899
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
    zct.write(sched, drive, duty, hal.cs); // ZC_TRACE main.c:908
    if !drive.old_routine.load(Ordering::Relaxed) {
        hal.comp.enable_comp_interrupts(); // main.c:910-912
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
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) {
    hal.lt.clear_flag();

    // duty_cycle = duty_cycle_setpoint (main.c:1611); tenkhzcounter++ (:1612)
    let setpoint = duty.duty_cycle_setpoint.load(Ordering::Relaxed) as i32;
    drive.tenkhz_counter.store(drive.tenkhz_counter.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);

    if !duty.killed.load(Ordering::Relaxed) {
        let duty_val = duty_ramp(sched, drive, duty, setpoint);
        let running = duty_apply(drive, duty, duty_val, hal);
        polling_bemf_check(sched, drive, duty, zct, hal, running);
    }

    uart_deadman_tick(duty, bench);
    adc_harvest_and_safety(drive, duty, bench, hal);
}

/// Duty apply band (main.c:1771-1791): CCR write path. Returns the
/// single `running` load so the caller's polling band reuses it
/// (preserves the original one-load ordering).
#[inline]
pub fn duty_apply(
    drive: &Drive,
    duty: &Duty,
    duty_val: u16,
    hal: &Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) -> bool {
    let tim1_arr = hal.pwm.max_duty() as u32;
    let running = drive.running.load(Ordering::Relaxed);
    let input = duty.input.load(Ordering::Relaxed);
    let base = (duty_val as u32 * tim1_arr) / 2000;
    let adjusted = if running && input > 47 { base + 1 } else { base };
    duty.last_duty_cycle.store(duty_val, Ordering::Relaxed); // main.c:1789
    hal.pwm.set_carrier_arr(tim1_arr as u16); // SET_AUTO_RELOAD_PWM (:1790)
    hal.pwm.set_duty(adjusted as u16); // SET_DUTY_CYCLE_ALL (:1791)
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
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
    running: bool,
) {
    if drive.old_routine.load(Ordering::Relaxed) && running {
        hal.comp.mask_phase_interrupts(); // main.c:1681
        get_bemf_state(drive, hal.comp); // :1682
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
                zcfoundroutine(sched, drive, zct, duty, hal);
            }
        }
    }
}

/// Observer ADC harvest + the two bench-safety KILLS (non-AM32;
/// they only stop the loop, never modulate it).
#[inline]
pub fn adc_harvest_and_safety(
    drive: &Drive,
    duty: &Duty,
    bench: &Bench,
    hal: &mut Hal<
        '_,
        impl MotorPwm,
        impl CompCtl,
        impl IntervalTimer,
        impl ComTimer + ComTimerExt,
        impl Recorder,
        impl Cs,
        impl InjAdc,
        impl LoopTimer,
    >,
) {
    let (_a, _b, cur, vbat) = hal.adc.inj_read();
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
        safety_kill(drive, duty, hal, 1);
    }
    if vbat < VBAT_ABS_FLOOR_RAW && drive.running.load(Ordering::Relaxed) {
        safety_kill(drive, duty, hal, 2);
    }
}

// ===============================================================
// Host tests — the shared MockHal/fixtures from am32_hal::mock.
// ===============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::am32_hal::mock::{BenchStore, DriveStore, DutyStore, MockHal, SchedStore, ZctStore};
    use crate::blackbox::EV_REF;

    // --- comp_isr ---------------------------------------------------

    #[test]
    fn comp_isr_not_pending_does_nothing() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        m.interval.set(1000);
        comp_isr(&sched, &drive, &mut m.hal());
        assert!(m.calls.borrow().is_empty());
        assert!(m.events.borrow().is_empty());
        // Not even the gate read happened: INTERVAL model untouched.
        assert_eq!(m.interval.get(), 1000);
    }

    #[test]
    fn comp_isr_gate_open_clears_pending_and_runs_interrupt_routine() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        m.pending.set(true);
        m.interval.set(200); // INTERVAL_TIMER->CNT
        sched.average_interval.store(100, Ordering::Relaxed); // gate = 50
        // Accept path setup: value == rising through the whole filter.
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(true);
        drive.filter_level.store(5, Ordering::Relaxed);
        drive.current_step.store(3, Ordering::Relaxed);
        sched.this_zc.store(42, Ordering::Relaxed);
        sched.wait_time.store(300, Ordering::Relaxed);
        comp_isr(&sched, &drive, &mut m.hal());
        // it.c:281-282 order: clear, then interruptRoutine's sequence.
        assert_eq!(
            *m.calls.borrow(),
            ["clear_pending", "mask_phase_interrupts", "set_interval_cnt", "set_and_enable_com_int"]
        );
        assert!(!m.pending.get());
        // lastzc shift + timestamp + interval reset (main.c:943-945).
        assert_eq!(sched.last_zc.load(Ordering::Relaxed), 42);
        assert_eq!(sched.this_zc.load(Ordering::Relaxed), 200);
        assert_eq!(m.interval.get(), 0);
        // COM armed at waitTime+1 (main.c:946).
        assert_eq!(*m.com_arrs.borrow(), [301]);
        // EV_ACC with sector = step-1 and thiszctime.
        assert_eq!(*m.events.borrow(), [(EV_ACC, 2, 200)]);
    }

    #[test]
    fn comp_isr_gate_closed_pre_zc_level_clears_only() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        m.pending.set(true);
        m.interval.set(100); // CNT=100 ≤ gate 500 → closed
        sched.average_interval.store(1000, Ordering::Relaxed);
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(false); // value != rising → pre-ZC level
        comp_isr(&sched, &drive, &mut m.hal());
        // Ack only — no interruptRoutine (it.c:284-285).
        assert_eq!(*m.calls.borrow(), ["clear_pending"]);
        assert!(!m.pending.get());
        assert!(m.events.borrow().is_empty());
    }

    #[test]
    fn comp_isr_gate_closed_post_zc_leaves_pending_set() {
        // The AM32 camp (it.c:280-286): gate closed + level already
        // post-ZC → do NOT ack; the IRQ re-fires until the gate opens
        // and the SAME crossing is then accepted.
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        m.pending.set(true);
        m.interval.set(100); // gate closed
        sched.average_interval.store(1000, Ordering::Relaxed);
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(true); // value == rising → post-ZC level
        comp_isr(&sched, &drive, &mut m.hal());
        assert!(!m.called("clear_pending"));
        assert!(m.pending.get()); // LEFT SET
        assert!(m.calls.borrow().is_empty());
        assert!(m.events.borrow().is_empty());
    }

    // --- interrupt_routine ------------------------------------------

    #[test]
    fn interrupt_routine_persistence_rejects_on_any_flip() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        drive.filter_level.store(5, Ordering::Relaxed);
        drive.rising.store(true, Ordering::Relaxed);
        // Reads 3 and 4 flip below the expected post-ZC level.
        *m.comp_seq.borrow_mut() = vec![true, true, false];
        sched.this_zc.store(42, Ordering::Relaxed);
        interrupt_routine(&sched, &drive, &mut m.hal());
        // Rejected: no mask / timestamp / arm / event (main.c:932-940).
        assert!(m.calls.borrow().is_empty());
        assert!(m.events.borrow().is_empty());
        assert_eq!(sched.this_zc.load(Ordering::Relaxed), 42);
        assert_eq!(sched.last_zc.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn interrupt_routine_accept_masks_shifts_timestamps_and_arms() {
        let (ss, ds, m) = (SchedStore::default(), DriveStore::default(), MockHal::new());
        let (sched, drive) = (ss.sched(), ds.drive());
        drive.filter_level.store(5, Ordering::Relaxed);
        drive.rising.store(false, Ordering::Relaxed);
        m.comp_value.set(false); // value == rising → hold through filter
        drive.current_step.store(1, Ordering::Relaxed);
        m.interval.set(777);
        sched.this_zc.store(55, Ordering::Relaxed);
        sched.wait_time.store(1000, Ordering::Relaxed);
        interrupt_routine(&sched, &drive, &mut m.hal());
        // Ordered accept sequence (main.c:942-946).
        assert_eq!(
            *m.calls.borrow(),
            ["mask_phase_interrupts", "set_interval_cnt", "set_and_enable_com_int"]
        );
        assert_eq!(sched.last_zc.load(Ordering::Relaxed), 55);
        assert_eq!(sched.this_zc.load(Ordering::Relaxed), 777);
        assert_eq!(m.interval.get(), 0);
        assert_eq!(*m.com_arrs.borrow(), [1001]); // waitTime + 1
        assert_eq!(*m.events.borrow(), [(EV_ACC, 0, 777)]);
    }

    // --- tim1_up_tim16_isr (the COM tick) ---------------------------

    #[test]
    fn tim1_up_tim16_isr_full_com_sequence() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        drive.current_step.store(1, Ordering::Relaxed);
        drive.old_routine.store(false, Ordering::Relaxed);
        drive.zero_crosses.store(5, Ordering::Relaxed);
        sched.commutation_interval.store(1000, Ordering::Relaxed);
        sched.last_zc.store(400, Ordering::Relaxed);
        sched.this_zc.store(600, Ordering::Relaxed);
        tim1_up_tim16_isr(&sched, &drive, &zct, &duty, &mut m.hal());
        // Sequence order (main.c:896-916): ack, disable, commutate.
        assert_eq!(m.calls.borrow()[0], "com_clear_flag");
        assert_eq!(m.calls.borrow()[1], "disable_com_timer_int");
        assert_eq!(m.calls.borrow()[2], "set_roles_for_step");
        assert_eq!(m.calls.borrow()[3], "change_comp_input");
        // Blend arithmetic matches am32::blend_interval (main.c:900);
        // commutate ran BEFORE the blend so ci_old is the pre-tick value.
        let ci = am32::blend_interval(1000, 400, 600);
        assert_eq!(sched.commutation_interval.load(Ordering::Relaxed), ci);
        // waitTime derivation stored (main.c:902-906).
        let advance = am32::advance_of(ci, TEMP_ADVANCE);
        assert_eq!(
            sched.wait_time.load(Ordering::Relaxed),
            am32::wait_time(ci, advance) as u16
        );
        // ZC_TRACE row written (main.c:908) — after commutate's EV_REF.
        assert_eq!(zs.records(), 1);
        assert_eq!(m.events.borrow()[0].0, EV_REF);
        // Interrupt mode → comp re-enabled (main.c:910-912).
        assert!(m.called("enable_comp_interrupts"));
        // zero_crosses incremented (main.c:913-915).
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 6);
    }

    #[test]
    fn tim1_up_tim16_isr_old_routine_skips_comp_enable_and_zc_saturates() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        drive.current_step.store(2, Ordering::Relaxed);
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.zero_crosses.store(10000, Ordering::Relaxed);
        tim1_up_tim16_isr(&sched, &drive, &zct, &duty, &mut m.hal());
        // old_routine → NO comp re-enable (main.c:910 gate).
        assert!(!m.called("enable_comp_interrupts"));
        // zc saturates at 10000 (main.c:913).
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 10000);
    }

    // --- tim6_dacunder_isr + bands ----------------------------------

    #[test]
    fn tim6_killed_skips_duty_and_polling_but_deadman_and_harvest_run() {
        let (ss, ds, us, bs, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            BenchStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, bench, zct) =
            (ss.sched(), ds.drive(), us.duty(), bs.bench(), zs.zct());
        duty.killed.store(true, Ordering::Relaxed);
        // Would take the polling path if the killed gate leaked.
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.running.store(true, Ordering::Relaxed);
        m.inj.set((1, 2, 3, 900)); // vbat above floor
        tim6_dacunder_isr(&sched, &drive, &duty, &bench, &zct, &mut m.hal());
        // Flag acked first; counter ticks (main.c:1612).
        assert_eq!(m.calls.borrow()[0], "tim6_clear_flag");
        assert_eq!(drive.tenkhz_counter.load(Ordering::Relaxed), 1);
        // Killed gate: no duty apply, no polling band.
        assert!(!m.called("set_duty"));
        assert!(!m.called("set_carrier_arr"));
        assert!(!m.called("mask_phase_interrupts"));
        assert_eq!(duty.ramp_count.load(Ordering::Relaxed), 0);
        // Deadman + harvest still run.
        assert_eq!(bench.uart_deadman_ticks.load(Ordering::Relaxed), 1);
        assert_eq!(bench.i_raw.load(Ordering::Relaxed), 3);
        assert_eq!(bench.vbat_raw.load(Ordering::Relaxed), 900);
        assert_eq!(bench.oc_cnt.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn tim6_not_killed_ramps_and_applies_duty() {
        let (ss, ds, us, bs, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            BenchStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, bench, zct) =
            (ss.sched(), ds.drive(), us.duty(), bs.bench(), zs.zct());
        m.inj.set((0, 0, 0, 900));
        tim6_dacunder_isr(&sched, &drive, &duty, &bench, &zct, &mut m.hal());
        // duty_ramp ran (ramp_count) and the CCR path fired.
        assert_eq!(duty.ramp_count.load(Ordering::Relaxed), 1);
        assert!(m.called("set_carrier_arr"));
        assert!(m.called("set_duty"));
    }

    #[test]
    fn duty_apply_plus_one_only_when_running_and_input_over_47() {
        let (ds, us, m) = (DriveStore::default(), DutyStore::default(), MockHal::new());
        let (drive, duty) = (ds.drive(), us.duty());
        // running && input>47 → +1 (main.c:1782-1787 comp_pwm bump).
        drive.running.store(true, Ordering::Relaxed);
        duty.input.store(48, Ordering::Relaxed);
        assert!(duty_apply(&drive, &duty, 1000, &m.hal()));
        // base = 1000*3332/2000 = 1666, adjusted 1667.
        assert_eq!(*m.duties.borrow(), [1667]);
        assert_eq!(*m.carrier_arrs.borrow(), [3332]);
        assert_eq!(duty.last_duty_cycle.load(Ordering::Relaxed), 1000);
        // input == 47: no +1.
        duty.input.store(47, Ordering::Relaxed);
        duty_apply(&drive, &duty, 1000, &m.hal());
        assert_eq!(m.duties.borrow()[1], 1666);
        // not running: no +1 (and returns false).
        drive.running.store(false, Ordering::Relaxed);
        duty.input.store(100, Ordering::Relaxed);
        assert!(!duty_apply(&drive, &duty, 1000, &m.hal()));
        assert_eq!(m.duties.borrow()[2], 1666);
    }

    #[test]
    fn polling_bemf_check_gates_on_old_routine_and_running() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        // Interrupt mode: band skipped entirely (main.c:1679 gate).
        drive.old_routine.store(false, Ordering::Relaxed);
        polling_bemf_check(&sched, &drive, &duty, &zct, &mut m.hal(), true);
        assert!(m.calls.borrow().is_empty());
        // old_routine but not running: also skipped.
        drive.old_routine.store(true, Ordering::Relaxed);
        polling_bemf_check(&sched, &drive, &duty, &zct, &mut m.hal(), false);
        assert!(m.calls.borrow().is_empty());
    }

    #[test]
    fn polling_bemf_check_accepts_over_threshold_rising_uses_min_bemf_up() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.current_step.store(1, Ordering::Relaxed);
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(true); // matches rising → bemf_counter climbs
        drive.bemf_counter.store(10, Ordering::Relaxed);
        // rising → min_bemf_up is the threshold (main.c:1684-1689);
        // min_bemf_down high proves it is NOT consulted.
        drive.min_bemf_up.store(3, Ordering::Relaxed);
        drive.min_bemf_down.store(1000, Ordering::Relaxed);
        sched.commutation_interval.store(100, Ordering::Relaxed);
        m.interval.set(100);
        polling_bemf_check(&sched, &drive, &duty, &zct, &mut m.hal(), true);
        // Band prelude: mask + getBemfState (main.c:1681-1682).
        assert_eq!(m.calls.borrow()[0], "mask_phase_interrupts");
        // Accept: zcfoundroutine ran once (com_set_arr + commutate).
        assert_eq!(m.calls.borrow().iter().filter(|c| **c == "com_set_arr").count(), 1);
        assert_eq!(m.roles.borrow().len(), 1);
        assert_eq!(drive.zero_crosses.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn polling_bemf_check_zcfound_latch_and_falling_threshold() {
        let (ss, ds, us, zs, m) = (
            SchedStore::default(),
            DriveStore::default(),
            DutyStore::default(),
            ZctStore::new(),
            MockHal::new(),
        );
        let (sched, drive, duty, zct) = (ss.sched(), ds.drive(), us.duty(), zs.zct());
        drive.old_routine.store(true, Ordering::Relaxed);
        drive.current_step.store(1, Ordering::Relaxed);
        // zcfound latched: counter over threshold must NOT re-accept
        // (main.c:1683 gate).
        drive.zcfound.store(true, Ordering::Relaxed);
        drive.rising.store(true, Ordering::Relaxed);
        m.comp_value.set(true);
        drive.bemf_counter.store(100, Ordering::Relaxed);
        drive.min_bemf_up.store(3, Ordering::Relaxed);
        polling_bemf_check(&sched, &drive, &duty, &zct, &mut m.hal(), true);
        assert!(!m.called("com_set_arr"));
        assert!(m.roles.borrow().is_empty());
        // Falling window: min_bemf_down is the threshold.
        drive.zcfound.store(false, Ordering::Relaxed);
        drive.rising.store(false, Ordering::Relaxed);
        m.comp_value.set(false); // matches !rising → counts
        drive.bemf_counter.store(10, Ordering::Relaxed);
        drive.min_bemf_up.store(1000, Ordering::Relaxed);
        drive.min_bemf_down.store(3, Ordering::Relaxed);
        m.interval.set(100);
        polling_bemf_check(&sched, &drive, &duty, &zct, &mut m.hal(), true);
        assert!(m.called("com_set_arr"));
    }

    #[test]
    fn adc_harvest_oc_trip_kills_with_reason_1() {
        let (ds, us, bs, m) = (
            DriveStore::default(),
            DutyStore::default(),
            BenchStore::default(),
            MockHal::new(),
        );
        let (drive, duty, bench) = (ds.drive(), us.duty(), bs.bench());
        // Window completes this tick with an over-threshold average
        // (the trip is strict `avg > OC_KILL_RAW_AVG`).
        bench.oc_cnt.store(OC_WINDOW_TICKS - 1, Ordering::Relaxed);
        bench
            .oc_acc
            .store((OC_KILL_RAW_AVG + 1) * OC_WINDOW_TICKS, Ordering::Relaxed);
        m.inj.set((0, 0, 500, 900)); // current sample; vbat healthy
        adc_harvest_and_safety(&drive, &duty, &bench, &mut m.hal());
        assert_eq!(bench.i_raw.load(Ordering::Relaxed), 500);
        // safety_kill(reason 1): all_off + latched kill + freeze.
        assert!(m.called("all_off"));
        assert!(m.called("mask_phase_interrupts"));
        assert!(m.called("disable_com_timer_int"));
        assert!(m.frozen.get());
        assert!(duty.killed.load(Ordering::Relaxed));
        assert_eq!(duty.kill_reason.load(Ordering::Relaxed), 1);
        // Accumulator reset by the completed window.
        assert_eq!(bench.oc_acc.load(Ordering::Relaxed), 0);
        assert_eq!(bench.oc_cnt.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn adc_harvest_vbat_floor_kills_only_when_running() {
        let (ds, us, bs, m) = (
            DriveStore::default(),
            DutyStore::default(),
            BenchStore::default(),
            MockHal::new(),
        );
        let (drive, duty, bench) = (ds.drive(), us.duty(), bs.bench());
        m.inj.set((0, 0, 0, VBAT_ABS_FLOOR_RAW - 1));
        // Not running: below-floor vbat is harvested but no kill.
        adc_harvest_and_safety(&drive, &duty, &bench, &mut m.hal());
        assert_eq!(bench.vbat_raw.load(Ordering::Relaxed), VBAT_ABS_FLOOR_RAW - 1);
        assert!(!duty.killed.load(Ordering::Relaxed));
        assert!(!m.called("all_off"));
        // Running: reason-2 kill.
        drive.running.store(true, Ordering::Relaxed);
        adc_harvest_and_safety(&drive, &duty, &bench, &mut m.hal());
        assert!(m.called("all_off"));
        assert!(duty.killed.load(Ordering::Relaxed));
        assert_eq!(duty.kill_reason.load(Ordering::Relaxed), 2);
    }
}
