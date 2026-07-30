//! ISR-level control logic — platform-independent, fully testable.
//!
//! All functions take `MotorContext<S, H>` for static dispatch.
//! No `&dyn` trait objects — the compiler monomorphizes to concrete MCU types,
//! eliminating vtable overhead in the 20kHz ISR.

use crate::commutation::Commutation;
use crate::constants::*;
use crate::control::context::MotorContext;
use crate::control::state::{BemfState, DutyState};
use crate::hal::{self, ComTimer, Comparator, IntervalTimer, MotorHal, PhaseOutput, PwmOutput};
use crate::motor_mode::MotorEvent;
use crate::shared_comm::SharedComm;

/// 20kHz control loop tick.
///
/// Handles: throttle→setpoint mapping, arming, BEMF polling (old_routine),
/// ramp rate limiting, PWM output.
pub fn ten_khz_tick<S: SharedComm, H: MotorHal>(ctx: &mut MotorContext<S, H>) {
    // 1 kHz dispatch counter — ISR-side increment, matches AM32 main.c:1317
    // (`one_khz_loop_counter++` inside tenKhzRoutine at 20 kHz). Main reads
    // and resets when it exceeds PID_LOOP_DIVIDER, firing the 1 kHz block
    // (ADC + PIDs). Placing the increment in the ISR makes the 1 kHz rate
    // correct regardless of main-loop iteration rate (main no longer wfi's
    // every iter — matches AM32's spinning while(1) at main.c:1843).
    ctx.shared.one_khz_counter_inc();
    // Defensive COMP-IRQ mask while not commutating. AM32 mirrors this by
    // calling maskPhaseInterrupts() at every stop/timeout site (~15 places
    // in main.c). We only mask on the AllOff path below, so StopMotor /
    // Disarm transitions (stuck rotor, desync, signal_timeout) can leak an
    // unmasked COMP into Armed-idle. With COMP at NVIC level 0 and TIM6 at
    // level 3, a comparator output bouncing on an undriven BEMF pin storms
    // COMP_IRQ and starves TIM6 indefinitely. Re-masking here every tick
    // when !running closes the leak from any of those paths.
    if !ctx.shared.running() {
        ctx.hal.comp().mask_interrupts();
    }
    // Process main→ISR action request (priority-ordered enum)
    match ctx.shared.isr_action() {
        crate::shared_comm::IsrAction::AllOff => {
            ctx.hal.phase().all_off();
            ctx.hal.comp().mask_interrupts();
        }
        crate::shared_comm::IsrAction::DutyKickDown => {
            // Desync recovery: restart ramps from min_startup/2.
            ctx.duty.kick_down();
        }
        crate::shared_comm::IsrAction::DutyKickHalf => {
            // Fast-rotor desync recovery: rotor is still locked (stay-
            // interrupt branch), so only shed half the torque.
            ctx.duty.kick_half();
        }
        crate::shared_comm::IsrAction::CommutateKick => {
            // BEMF-timeout recovery, AM32 zcfoundroutine semantics: re-arm
            // the possibly-dead COM timer to fire now so
            // commutation_timer_expired restarts the chain. The interval
            // reset this action SUBSUMES (single-slot fetch_max channel —
            // see main_state) happens at the END of this tick with the
            // ResetIntervalTimer path, after the count is published.
            //
            // zcfoundroutine timing update (main.c:1870-1874): the stalled
            // interval count (>45000) folds INTO the commutation interval
            // BEFORE the forced step — ci = (thiszc + 3*ci)/4 — so the
            // restart is AM32's slow crawl toward re-lock, with wait_time
            // and advance recomputed from the inflated ci. Without this,
            // the kick re-commutated at the PRE-FAULT cadence (ci ~200 at
            // 60% throttle): a full-duty blind slam on a rotor that just
            // lost sync — the transit-surge kill class at the 60% rung.
            let count = ctx.hal.interval().count() as u16;
            let ci = ctx.shared.commutation_interval();
            let new_ci = ctx.bemf.record_zero_cross(count, ci);
            ctx.shared.set_commutation_interval(new_ci);
            ctx.shared.set_fly_pending(false); // real arm, not the backup
            ctx.hal.com_timer().set_and_enable(1);
        }
        crate::shared_comm::IsrAction::ResetIntervalTimer => {
            // Handled at the end of this function (after publish)
        }
        crate::shared_comm::IsrAction::None => {}
    }
    // Sine changeover: main published a step for ISR to execute
    let changeover = ctx.shared.changeover_step();
    if changeover > 0 {
        ctx.commutation.set_step(changeover);
        ctx.hal.phase().com_step(changeover);
        ctx.hal.pwm().generate_update_event();
        let ci = ctx.shared.commutation_interval();
        ctx.shared.set_fly_pending(false); // real arm, not the backup
        ctx.hal.com_timer().set_and_enable(ci as u16);
        ctx.hal.comp().enable_interrupts();
        ctx.shared.set_changeover_step(0);
    }
    // Sync direction from shared (main loop may flip for bidirectional)
    ctx.commutation.forward = ctx.shared.forward();
    let tim1_arr = ctx.shared.tim1_arr();

    // Throttle → setpoint
    // Read adjusted_input (set by process_input: bidir-mapped or raw passthrough)
    let input = ctx.shared.adjusted_input();
    if ctx.shared.armed() && !ctx.shared.stepper_sine() {
        if input >= THROTTLE_MIN_SIGNAL {
            let setpoint = ctx.duty.compute_setpoint(
                input,
                ctx.shared.zero_crosses(),
                ctx.config.stall_protection,
            );
            ctx.shared.set_duty_cycle_setpoint(setpoint);
            if !ctx.shared.running() {
                ctx.hal.phase().all_off(); // clear phase outputs before startup
                ctx.shared.transition(MotorEvent::StartMotor);
                ctx.duty.start_motor();
                let step = ctx.commutation.advance();
                let e_com = ctx
                    .commutation
                    .record_interval(ctx.shared.commutation_interval() as u16);
                ctx.shared.set_e_com_time(e_com);
                ctx.hal.phase().com_step(step);
                ctx.hal.comp().set_step(step, ctx.commutation.rising);
                ctx.hal.comp().change_input();
                // AM32 startMotor seeds (main.c:954-955): a fat initial
                // commutation interval and a HALF-FULL interval timer. The
                // 5000 count is the load-bearing trick — it makes the first
                // ZC acceptance gate (CNT > average_interval/2) passable
                // immediately, so engage converges deterministically instead
                // of hovering at the changeover knife-edge (the observed
                // ~50% 'engage lottery'; the clone starts 8/8 with these).
                ctx.shared.set_commutation_interval(10000);
                ctx.hal.interval().set_count(5000);
                // Comparator interrupts deliberately NOT enabled here — the
                // clone's one divergence from AM32 main.c:958, measured
                // start-reliability-positive: startup runs pure polling and
                // the interrupt path arms at the BemfLocked changeover.
            }
        } else {
            ctx.shared.set_duty_cycle_setpoint(0);
            if ctx.config.brake_on_stop == 2 {
                ctx.hal.phase().com_step(2);
                let brake_duty = (ctx.config.active_brake_power as u32 * tim1_arr as u32
                    / DUTY_SCALE_MAX as u32)
                    * 10;
                ctx.hal.pwm().set_duty_all(brake_duty as u16);
            }
        }
    }

    // Core tick
    let setpoint = ctx.shared.duty_cycle_setpoint();
    ctx.duty.set_cycle(setpoint);
    ctx.shared.increment_signal_timeout();
    ctx.duty.increment_ramp_count();

    // Arming
    if !ctx.shared.armed() {
        if ctx.shared.input_set() && ctx.shared.adjusted_input() == 0 {
            *ctx.armed_timeout_count += 1;
            if *ctx.armed_timeout_count > ARMING_TIMEOUT_TICKS {
                ctx.shared.transition(MotorEvent::Arm);
                *ctx.armed_timeout_count = 0;
            }
        } else {
            *ctx.armed_timeout_count = 0;
        }
    }

    // Old routine BEMF polling
    if ctx.shared.old_routine() && ctx.shared.running() && !ctx.shared.stepper_sine() {
        bemf_polling(ctx);
    }

    // Ramp rate limiting
    let average_interval = (ctx.shared.e_com_time() / 3) as u32;
    ctx.duty.ramp_limit(
        ctx.shared.battery_voltage(),
        ctx.shared.commutation_interval(),
        ctx.shared.zero_crosses(),
        average_interval,
        ctx.voltage_based_ramp,
    );

    // Sync main→ISR published state (main computes, ISR applies).
    // Bench advance-lever ('Y'): nonzero override replaces auto_advance
    // (temp_advance) — the demag-margin intervention knob.
    let adv = {
        let ov = ctx.shared.bench_advance_override();
        if ov != 0 {
            ov
        } else {
            ctx.shared.auto_advance()
        }
    };
    ctx.bemf
        .sync_config(ctx.shared.filter_level(), adv, ctx.shared.min_bemf_counts());

    // Apply stall boost + duty/current ceilings
    let stall_boost = if ctx.shared.running() {
        ctx.shared.stall_protection_adjust()
    } else {
        0
    };
    ctx.duty.clamp_ceilings(
        stall_boost,
        ctx.shared.duty_maximum(),
        ctx.shared.current_limit_adjust(),
    );

    // PWM output
    if ctx.shared.armed() && ctx.shared.running() {
        ctx.hal.pwm().set_duty_all(ctx.duty.pwm_compare(tim1_arr));
    } else if ctx.shared.prop_brake_active() {
        ctx.hal.pwm().set_duty_all(DutyState::brake_compare(
            ctx.config.drag_brake_strength,
            tim1_arr,
        ));
    } else {
        ctx.hal.pwm().set_duty_all(0);
    }
    let final_duty = ctx.duty.finalize();
    ctx.shared.set_duty_cycle(final_duty);
    ctx.hal.pwm().set_auto_reload(tim1_arr);

    // Sync ISR→shared (Commutation owns truth, shared publishes for main loop)
    ctx.shared.set_forward(ctx.commutation.forward);
    ctx.shared
        .set_interval_timer_count(ctx.hal.interval().count());
    // Handle ResetIntervalTimer AFTER publish so the published value isn't
    // immediately overwritten. AllOff is handled at the top (before tick).
    // CommutateKick includes this reset (zcfoundroutine semantics) — its
    // COM-timer re-arm already ran at the top.
    {
        let act = ctx.shared.isr_action();
        if act == crate::shared_comm::IsrAction::ResetIntervalTimer
            || act == crate::shared_comm::IsrAction::CommutateKick
        {
            ctx.hal.interval().set_count(0);
        }
    }
    // Clear any pending action (AllOff was already executed at top)
    ctx.shared.clear_isr_action();
}

/// BEMF polling (old_routine path).
fn bemf_polling<S: SharedComm, H: MotorHal>(ctx: &mut MotorContext<S, H>) {
    ctx.hal.comp().mask_interrupts();
    let comp_level = ctx.hal.comp().output_level();
    let rising = ctx.commutation.rising;
    ctx.bemf.update(comp_level, rising);

    if ctx.bemf.zero_cross_detected(rising) {
        let interval_count = ctx.hal.interval().count() as u16;
        ctx.hal.interval().set_count(0);
        let ci = ctx.shared.commutation_interval();
        let new_ci = ctx.bemf.record_zero_cross(interval_count, ci);
        ctx.shared.set_commutation_interval(new_ci);

        if ctx.shared.zero_crosses() < MIN_ZC_FOR_ADVANCE {
            let step = ctx.commutation.advance();
            let e_com = ctx
                .commutation
                .record_interval(ctx.shared.commutation_interval() as u16);
            ctx.shared.set_e_com_time(e_com);
            ctx.hal.phase().com_step(step);
            ctx.hal.phase().pulse_toggle(step);
            ctx.hal.comp().set_step(step, ctx.commutation.rising);
            ctx.hal.comp().change_input();
            ctx.bemf.reset_for_step();
            ctx.shared.increment_zero_crosses();
        } else {
            ctx.shared.set_fly_pending(false); // real arm, not the backup
            ctx.hal
                .com_timer()
                .set_and_enable(ctx.bemf.com_timer_delay());
        }
    }
}

/// Commutation timer expired (TIM14/TIM16 ISR body).
#[allow(clippy::too_many_arguments)]
pub fn commutation_timer_expired<S, C, Ph, T>(
    commutation: &mut Commutation,
    bemf: &mut BemfState,
    shared: &S,
    com_timer: &mut T,
    comp: &mut C,
    phase: &mut Ph,
    bidirectional: bool,
    strict_changeover: bool,
) where
    S: SharedComm,
    C: hal::Comparator,
    Ph: hal::PhaseOutput,
    T: hal::ComTimer,
{
    com_timer.disable_interrupt();
    // Flywheel ('O'): was THIS fire the backup arm (no ZC accepted since
    // the last commutation)? Consume the flag either way.
    let fly_fire = shared.fly_pending();
    shared.set_fly_pending(false);
    if fly_fire {
        shared.bench_fly_fired();
    }
    let step = commutation.advance();
    // Publish desync_check flag to SharedState (main reads it for desync detection)
    if commutation.desync_check() {
        shared.set_desync_check_pending(true);
        commutation.set_desync_check(false);
    }
    let e_com = commutation.record_interval(shared.commutation_interval() as u16);
    shared.set_e_com_time(e_com);
    phase.com_step(step);
    phase.pulse_toggle(step);
    comp.set_step(step, commutation.rising);
    comp.change_input();

    // Bidir mode halves the changeover threshold for faster mode transition
    // during direction changes (C: polling_mode_changeover / 2)
    let exit_interval = if bidirectional {
        OLD_ROUTINE_EXIT_INTERVAL / 2
    } else {
        OLD_ROUTINE_EXIT_INTERVAL
    };

    let was_interrupt_mode = !shared.old_routine();

    // Mid-run polling demotion (AM32 commutate, main.c:878-881): if the
    // average interval has inflated past the changeover threshold + 500,
    // fall back to polling mode. This is AM32's per-commutation escape
    // from a deep desync that never trips the BEMF timeout; without it a
    // slowed rotor stays in interrupt mode indefinitely. Runs BEFORE the
    // comp re-enable gate below (a demoted step must not re-arm the
    // comparator) but does NOT suppress this step's interval update —
    // AM32's two-tap in PeriodElapsedCallback is unconditional.
    if was_interrupt_mode && (e_com / 3) as u32 > exit_interval + 500 {
        shared.set_old_routine(true);
    }

    if was_interrupt_mode {
        let new_ci = bemf.update_timing_from_timer(shared.commutation_interval());
        shared.set_commutation_interval(new_ci);
    }

    // Polling/interrupt exclusivity (AM32 main.c commutate + minz
    // am32_isr.rs:122-124): the comparator interrupt path is live ONLY in
    // interrupt mode. rm32 previously enabled unconditionally, so both
    // BEMF paths ran concurrently during old_routine — double-commutation
    // risk and inconsistent interval updates.
    if !shared.old_routine() {
        comp.enable_interrupts();
    }
    // FLYWHEEL ('O', bench divergence under test): also arm a backup
    // forced commutation at ~1.5x ci. A missed/invisible ZC then costs
    // one blended-late step instead of freezing the step/mux (no
    // commutation -> no re-mux -> ~1 e-rev of silence -> desync
    // cascade — the 98-100% wall's amplifier). A real accept overwrites
    // this arm (and clears fly_pending in the firmware accept path).
    if shared.bench_flywheel() != 0 && !shared.old_routine() && shared.running() {
        let ci = shared.commutation_interval();
        let backup = (ci + (ci >> 1)).clamp(30, 60000) as u16;
        com_timer.set_and_enable(backup);
        shared.set_fly_pending(true);
    }
    bemf.reset_after_commutation();
    shared.increment_zero_crosses();

    let zc = shared.zero_crosses();
    let ci = shared.commutation_interval();
    // Polling→interrupt changeover (AM32 main.c:1903-1913): the
    // zc>=20 form applies ONLY with stall_protection / rc_car_reverse;
    // the normal path is `ci < changeover` ALONE. rm32 previously
    // required zc>=20 unconditionally — and since spin-up desyncs reset
    // zero_crosses, a descent through the changeover rarely survived 20
    // commutations: THE engage lottery (forensic: ci descending
    // 2676→853, 30/30 still polling, sawtooth zc resets).
    let changeover_met = if strict_changeover {
        zc >= OLD_ROUTINE_EXIT_ZC && ci <= exit_interval
    } else {
        ci < exit_interval
    };
    // `!was_interrupt_mode`: the promote belongs to the polling path
    // (AM32 zcfoundroutine) — a step that just DEMOTED above must not
    // re-promote in the same commutation.
    if !was_interrupt_mode && shared.old_routine() && changeover_met {
        shared.transition(MotorEvent::BemfLocked);
        // Changeover: arm the interrupt path now (AM32 zcfoundroutine
        // enables comparator interrupts at this exact transition).
        comp.enable_interrupts();
    }
}

/// BEMF zero-cross detected (COMP ISR body).
///
/// **IMPORTANT for porting**: the noise-filter loop below early-returns
/// BEFORE `comp.mask_interrupts()` (which is the only path that clears the
/// EXTI pending bit on most MCUs). If the platform ISR wrapper relies on
/// this function to ack the EXTI line, the early-return causes NVIC to
/// re-fire the COMP ISR forever → ISR storm → main-loop starvation.
///
/// Mitigation: the platform-specific COMP ISR wrapper must ack EXTI.PR1
/// (or the equivalent rising/falling pending registers) at ISR entry,
/// BEFORE calling `handle_comp`. L431 does this; F051/G071/G431 do NOT
/// (as of this writing — same latent bug exists there, just hasn't been
/// observed yet). See `mcu_l431/interrupts.rs::COMP()` for the pattern.
pub fn bemf_zero_cross<C: hal::Comparator, I: hal::IntervalTimer, T: hal::ComTimer>(
    commutation: &Commutation,
    bemf: &mut BemfState,
    comp: &mut C,
    interval: &mut I,
    com_timer: &mut T,
) -> bool {
    for _ in 0..bemf.filter_level() {
        if comp.output_level() == commutation.rising() {
            return false;
        }
    }
    comp.mask_interrupts();
    let count = interval.count() as u16;
    interval.set_count(0);
    bemf.record_zc_timing(count);
    com_timer.set_and_enable(bemf.com_timer_delay());
    true
}
