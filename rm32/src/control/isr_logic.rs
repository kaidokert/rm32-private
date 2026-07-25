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
                // AM32 startMotor deliberately does NOT enable comparator
                // interrupts — startup runs pure polling; the interrupt
                // path arms at the BemfLocked changeover (main.c's
                // zcfoundroutine). rm32 previously enabled here, running
                // both BEMF paths concurrently from the first commutation.
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

    // Sync main→ISR published state (main computes, ISR applies)
    ctx.bemf.sync_config(
        ctx.shared.filter_level(),
        ctx.shared.auto_advance(),
        ctx.shared.min_bemf_counts(),
    );

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
    if ctx.shared.isr_action() == crate::shared_comm::IsrAction::ResetIntervalTimer {
        ctx.hal.interval().set_count(0);
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
            ctx.hal
                .com_timer()
                .set_and_enable(ctx.bemf.com_timer_delay());
        }
    }
}

/// Commutation timer expired (TIM14/TIM16 ISR body).
pub fn commutation_timer_expired<S, C, Ph, T>(
    commutation: &mut Commutation,
    bemf: &mut BemfState,
    shared: &S,
    com_timer: &mut T,
    comp: &mut C,
    phase: &mut Ph,
    bidirectional: bool,
) where
    S: SharedComm,
    C: hal::Comparator,
    Ph: hal::PhaseOutput,
    T: hal::ComTimer,
{
    com_timer.disable_interrupt();
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

    if !shared.old_routine() {
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
    bemf.reset_after_commutation();
    shared.increment_zero_crosses();

    let zc = shared.zero_crosses();
    let ci = shared.commutation_interval();
    // Bidir mode halves the changeover threshold for faster mode transition
    // during direction changes (C: polling_mode_changeover / 2)
    let exit_interval = if bidirectional {
        OLD_ROUTINE_EXIT_INTERVAL / 2
    } else {
        OLD_ROUTINE_EXIT_INTERVAL
    };
    if shared.old_routine() && zc >= OLD_ROUTINE_EXIT_ZC && ci <= exit_interval {
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
) {
    for _ in 0..bemf.filter_level() {
        if comp.output_level() == commutation.rising() {
            return;
        }
    }
    comp.mask_interrupts();
    let count = interval.count() as u16;
    interval.set_count(0);
    bemf.record_zc_timing(count);
    com_timer.set_and_enable(bemf.com_timer_delay());
}
