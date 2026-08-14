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

const COMMUTATE_KICK_DELAY_TICKS: u16 = 1;
const STARTUP_COMMUTATION_INTERVAL: u32 = 10000;
const STARTUP_INTERVAL_TIMER_COUNT: u32 = STARTUP_COMMUTATION_INTERVAL / 2;

/// 20kHz control loop tick.
///
/// Handles: throttle→setpoint mapping, arming, BEMF polling (old_routine),
/// ramp rate limiting, PWM output.
pub fn ten_khz_tick<S: SharedComm, H: MotorHal>(ctx: &mut MotorContext<S, H>) {
    // 1 kHz dispatch counter — ISR-side increment (AM32 main.c:1317);
    // main reads and resets past PID_LOOP_DIVIDER. Incrementing here
    // keeps the 1 kHz rate correct regardless of main-loop iteration
    // rate.
    ctx.shared.one_khz_counter_inc();

    // AM32 interval telemetry (main.c:1664-1672): with
    // telemetry_on_interval set, fire send_telemetry every
    // (30 - 1 + interval) ms — the config value doubles as a per-ESC
    // slot offset on shared telemetry wires.
    if ctx.config.telemetry_on_interval != 0 {
        let limit = telemetry_interval_ticks(ctx.config.telemetry_on_interval);
        if ctx.shared.telem_counter_check_and_inc(limit) {
            ctx.shared.set_send_telemetry(true);
        }
    }
    // Defensive COMP-IRQ mask while not commutating (AM32 calls
    // maskPhaseInterrupts() at every stop/timeout site). A comparator
    // bouncing on an undriven BEMF pin at higher NVIC priority would
    // otherwise storm and starve the tick ISR; re-masking every
    // non-running tick closes every stop-path leak at once.
    if !ctx.shared.running() {
        ctx.hal.comp().mask_interrupts();
    }
    // Process main→ISR action request (priority-ordered enum)
    let action = ctx.shared.isr_action();
    let action_interval_count = ctx.hal.interval().count();
    match action {
        crate::shared_comm::IsrAction::AllOff => {
            ctx.hal.phase().all_off();
            ctx.hal.comp().mask_interrupts();
            ctx.shared
                .clear_isr_action(crate::shared_comm::IsrAction::AllOff);
            return;
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
            // the possibly-dead COM timer so commutation_timer_expired
            // restarts the chain. The stalled interval count folds into
            // the commutation interval BEFORE the forced step
            // (ci = (thiszc + 3*ci)/4, main.c:1870-1874) so the restart
            // is AM32's slow crawl toward re-lock — without it the kick
            // re-commutates at the pre-fault cadence, a full-duty blind
            // slam on a rotor that just lost sync.
            if ctx.shared.running() {
                let count = action_interval_count.min(u16::MAX as u32) as u16;
                let ci = ctx.shared.commutation_interval();
                let new_ci = ctx.bemf.record_zero_cross(count, ci);
                ctx.shared.set_commutation_interval(new_ci);
                ctx.hal
                    .com_timer()
                    .set_and_enable(COMMUTATE_KICK_DELAY_TICKS);
            }
            ctx.hal.interval().set_count(0);
            ctx.shared
                .clear_isr_action(crate::shared_comm::IsrAction::CommutateKick);
        }
        crate::shared_comm::IsrAction::ResetIntervalTimer => {
            ctx.hal.interval().set_count(0);
            ctx.shared
                .clear_isr_action(crate::shared_comm::IsrAction::ResetIntervalTimer);
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
        ctx.hal.comp().set_step(changeover, ctx.commutation.rising);
        ctx.hal.comp().change_input();
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
                ctx.hal.phase().all_off();
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
                ctx.shared
                    .set_commutation_interval(STARTUP_COMMUTATION_INTERVAL);
                ctx.hal.interval().set_count(STARTUP_INTERVAL_TIMER_COUNT);
            }
        } else {
            ctx.shared.set_duty_cycle_setpoint(0);
            // AM32 !running housekeeping (main.c:1256-1259): while at
            // zero throttle and not running, continuously scrub the run
            // counters. Without this, zero_crosses carries across runs,
            // polluting the stuck-rotor fault-clear and the zc-gated
            // startup boost.
            if !ctx.shared.running() {
                ctx.shared.set_zero_crosses(0);
                ctx.bemf.reset_for_step();
            }
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
        // SAFETY-CRITICAL ORDER: reconfigure the bridge for braking
        // BEFORE the brake duty lands (AM32 proportionalBrake(): all
        // high-sides output-off, all low-sides PWM). A near-ARR brake
        // compare applied to the mixed bridge state a stop leaves behind
        // drives a DC VBAT->winding->GND path — locked-rotor burn.
        // Re-asserted every tick like AM32's every-main-pass call.
        ctx.hal.phase().proportional_brake();
        ctx.hal.pwm().set_duty_all(DutyState::brake_compare(
            ctx.config.drag_brake_strength,
            tim1_arr,
        ));
    } else {
        ctx.hal.phase().all_off();
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
        if action == crate::shared_comm::IsrAction::ResetIntervalTimer
            || action == crate::shared_comm::IsrAction::CommutateKick
        {
            ctx.hal.interval().set_count(0);
        }
    }
    // Clear only the action handled above; a newer higher-priority action
    // posted during this tick must remain pending.
    ctx.shared.clear_isr_action(action);
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
    let step = commutation.advance();
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

    // Polling/interrupt exclusivity (AM32 main.c commutate
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
