//! ISR-level control logic — platform-independent, fully testable.
//!
//! All functions take `MotorContext<S, H>` for static dispatch.
//! No `&dyn` trait objects — the compiler monomorphizes to concrete MCU types,
//! eliminating vtable overhead in the 20kHz ISR.

use crate::commutation::Commutation;
use crate::constants::*;
use crate::control::context::MotorContext;
use crate::control::state::{BemfState, DutyState};
use crate::hal::{self, BemfSensor, Bridge, ComTimer, IntervalTimer, MotorHal, PhaseOutput};
use crate::motor_mode::MotorEvent;
use crate::shared_comm::SharedComm;

const COMMUTATE_KICK_DELAY_TICKS: u16 = 1;

/// 20kHz control loop tick.
///
/// Handles: throttle→setpoint mapping, arming, BEMF polling (old_routine),
/// ramp rate limiting, PWM output.
pub fn ten_khz_tick<S: SharedComm, H: MotorHal>(ctx: &mut MotorContext<S, H>) {
    // 1 kHz dispatch counter — ISR-side increment (AM32);
    // main reads and resets past PID_LOOP_DIVIDER. Incrementing here
    // keeps the 1 kHz rate correct regardless of main-loop iteration
    // rate.
    ctx.shared.one_khz_counter_inc();
    // Free-running tick count: the main loop's 1 s (consumed current) and
    // LED blink periods (AM32's `tenkhzcounter++` / `ledcounter++` here).
    ctx.shared.tick_count_inc();

    // AM32 interval telemetry (): with
    // telemetry_on_interval set, fire send_telemetry every
    // (30 - 1 + interval) ms — the config value doubles as a per-ESC
    // slot offset on shared telemetry wires.
    if ctx.config.telemetry_on_interval != 0 {
        let limit = telemetry_interval_ticks::<H>(ctx.config.telemetry_on_interval);
        if ctx.shared.telem_counter_check_and_inc(limit) {
            ctx.shared.set_send_telemetry(true);
        }
    }

    if !ctx.shared.running() {
        // Keep COMP masked while phases are not commutating.
        ctx.hal.bemf().mask_irq();
    }

    let action = ctx.shared.isr_action();
    let action_interval_count = ctx.hal.interval().count();
    match action {
        crate::shared_comm::IsrAction::AllOff => {
            ctx.hal.bridge().all_off();
            ctx.hal.bemf().mask_irq();
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
            // (ci = (thiszc + 3*ci)/4, ) so the restart
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
        ctx.hal.bridge().com_step(
            changeover,
            crate::bridge::drive_complementary(ctx.config.use_comp_pwm(), ctx.shared),
        );
        ctx.hal.bridge().latch();
        let ci = ctx.shared.commutation_interval();
        ctx.hal.com_timer().set_and_enable(ci as u16);
        route_bemf(ctx.hal.bemf(), ctx.commutation);
        ctx.hal.bemf().enable_irq();
        ctx.shared.set_changeover_step(0);
    }
    // Sync direction from shared (main loop may flip for bidirectional)
    ctx.commutation.forward = ctx.shared.forward();
    let tim1_arr = ctx.shared.tim1_arr();

    // Throttle → setpoint
    // Read adjusted_input (set by process_input: bidir-mapped or raw passthrough)
    let input = ctx.shared.adjusted_input();
    // AM32 setInput: with sine start, six-step begins at 47 + 80 and the
    // duty map starts at input 137 (below that the sine stepper drives).
    let sine_start = ctx.config.use_sine_start != 0;
    let start_threshold = THROTTLE_MIN_SIGNAL + if sine_start { 80 } else { 0 };
    if ctx.shared.armed() && !ctx.shared.stepper_sine() {
        if input >= start_threshold {
            let setpoint = ctx.duty.compute_setpoint(
                input,
                ctx.shared.zero_crosses(),
                ctx.config.stall_protection,
                sine_start,
            );
            ctx.shared.set_duty_cycle_setpoint(setpoint);
            if !ctx.shared.running() {
                ctx.hal.bridge().all_off();
                ctx.shared.transition(MotorEvent::StartMotor);
                ctx.duty.start_motor();
                let step = ctx.commutation.advance();
                let e_com = ctx
                    .commutation
                    .record_interval::<H>(ctx.shared.commutation_interval() as u16);
                ctx.shared.set_e_com_time(e_com);
                ctx.hal.bridge().com_step(
                    step,
                    crate::bridge::drive_complementary(ctx.config.use_comp_pwm(), ctx.shared),
                );
                route_bemf(ctx.hal.bemf(), ctx.commutation);
                ctx.shared
                    .set_commutation_interval(H::STARTUP_COMMUTATION_INTERVAL);
                ctx.hal
                    .interval()
                    .set_count(H::STARTUP_COMMUTATION_INTERVAL / 2);
            }
        } else {
            ctx.shared.set_duty_cycle_setpoint(0);
            if !ctx.shared.running() {
                // Match AM32 idle housekeeping: stale run counters must not carry
                // into the next startup attempt.
                ctx.shared.set_zero_crosses(0);
                ctx.bemf.reset_for_step();
            }
            if ctx.config.brake_on_stop == 2 {
                ctx.hal.bridge().com_step(
                    2,
                    crate::bridge::drive_complementary(ctx.config.use_comp_pwm(), ctx.shared),
                );
                let brake_duty = crate::fast_math::div2000_pwm(
                    ctx.config.active_brake_power as u32 * tim1_arr as u32,
                ) * 10;
                ctx.hal.bridge().set_duty_all(brake_duty as u16);
            }
            // AM32: below the six-step threshold the sine stepper takes
            // over (phase positions are seeded by the main-loop stepper).
            if sine_start {
                ctx.shared.set_stepper_sine(true);
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
            if *ctx.armed_timeout_count > H::ARMING_TIMEOUT_TICKS {
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

    // Ramp rate limiting. The average is read here, after `bemf_polling`
    // may have updated e_com this tick, and is also the COMP gate's latch
    // (see `IsrTiming::gate_avg`): one division per tick. Clamped at 0
    // only: AM32's 5000 is a one-shot desync reset, not a cap -- clamping
    // continuously halves the spin-up gate (4/4 climb failures measured).
    // The AllOff tick returns before this and skips one latch; AllOff
    // clears itself, so the next tick latches.
    let average_interval = crate::platform::e_com_us_to_avg_ticks::<H>(ctx.shared.e_com_time());
    ctx.shared.set_gate_avg(average_interval.max(0) as u32);
    ctx.duty.ramp_limit::<H>(
        ctx.shared.battery_voltage(),
        ctx.shared.commutation_interval(),
        ctx.shared.zero_crosses(),
        average_interval as u32,
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
    // Sine stepper driving (armed, throttle above idle): the main loop
    // writes the CCRs. AM32 main-loop stepper `do_once_sinemode`: on
    // entering slow stepping, stop commutation + BEMF interrupts and put all
    // phases on PWM; AM32 skips every tenKhz duty write while it steps.
    // Sine IDLE (unarmed / zero throttle) falls through to the brake chain
    // below, as AM32's stepper idle does per `brake_on_stop` (main.c: 1 ->
    // proportional brake, 2 -> comStep(2), else allOff).
    let sine_drive =
        ctx.shared.stepper_sine() && ctx.shared.armed() && input > THROTTLE_MIN_SIGNAL + 1;
    if !sine_drive {
        ctx.duty.sine_outputs_on = false;
    }
    if sine_drive {
        if input < SINE_SLOW_STEP_THROTTLE {
            if !ctx.duty.sine_outputs_on {
                ctx.hal.com_timer().disable_interrupt();
                ctx.hal.bemf().mask_irq();
                ctx.hal.bridge().set_duty_all(0);
                ctx.hal.bridge().all_pwm(crate::bridge::drive_complementary(
                    ctx.config.use_comp_pwm(),
                    ctx.shared,
                ));
                ctx.duty.sine_outputs_on = true;
            }
        } else {
            ctx.duty.sine_outputs_on = false;
        }
    } else if ctx.shared.armed() && ctx.shared.running() {
        ctx.hal
            .bridge()
            .set_duty_all(ctx.duty.pwm_compare(tim1_arr));
    } else if ctx.shared.prop_brake_active() {
        // SAFETY-CRITICAL ORDER: reconfigure the bridge for braking
        // BEFORE the brake duty lands (AM32 proportionalBrake(): all
        // high-sides output-off, all low-sides PWM). A near-ARR brake
        // compare applied to the mixed bridge state a stop leaves behind
        // drives a DC VBAT->winding->GND path — locked-rotor burn.
        // Re-asserted every tick like AM32's every-main-pass call.
        ctx.hal.bridge().proportional_brake();
        ctx.hal.bridge().set_duty_all(DutyState::brake_compare(
            ctx.config.drag_brake_strength,
            tim1_arr,
        ));
    } else {
        ctx.hal.bridge().all_off();
        ctx.hal.bridge().set_duty_all(0);
    }
    let final_duty = ctx.duty.finalize();
    ctx.shared.set_duty_cycle(final_duty);
    ctx.hal.bridge().set_period(tim1_arr);

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

/// Route the comparator to the phase floating in the current commutation
/// step, armed for its BEMF edge. Out of line, with the port's
/// `select_phase` inlined into this one copy: callers pass two pointers, as
/// the old `set_step` + `change_input` pair cost them two stores. Computing
/// the phase at each call site cost the M0 commutation ISR more (binz TIM14
/// +2 to +20 instructions, S6.1).
#[inline(never)]
fn route_bemf<B: BemfSensor>(sensor: &mut B, commutation: &Commutation) {
    sensor.select_phase(hal::floating_phase(commutation.step), commutation.rising);
}

/// What `on_bemf_irq` did with a comparator interrupt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BemfIrq {
    /// No edge pending (shared vector, or already handled): nothing done.
    Spurious,
    /// Gate open: pending cleared; the caller runs `bemf_zero_cross`.
    Open,
    /// Gate closed, comparator at the pre-ZC level: a noise blip, cleared
    /// (re-raised if the level flipped during the clear).
    Noise,
    /// Gate closed, comparator at the post-ZC level: left pending, so the
    /// interrupt re-enters until the gate opens. Bounded: the interval
    /// timer free-runs, so it passes the gate within `gate_avg / 2`.
    Camp,
}

/// The COMP interrupt's acceptance gate (AM32 `COMP_IRQHandler`,
/// stm32l4xx_it.c:276-290 / stm32g0xx_it.c): the gate opens once the
/// interval timer has run past half the control tick's latched average
/// (`IsrTiming::gate_avg`, stale on purpose; read directly as AM32 reads
/// `average_interval` -- the old ports' fresh e_com/3 fallback for an
/// unlatched 0 is gone: the tick latches from boot, and while the latch is
/// 0 so, nearly always, is e_com/3). Gate open on an edge -> `Open` (the
/// caller then runs `bemf_zero_cross`, which masks on accept and stays
/// armed on a persistence reject). Closed -> `Noise` or `Camp`.
///
/// The pre-ZC re-read after the clear (`repend`) closes the edge-swallow
/// race found in the L431 fall post-mortem (2026-07-26): a real crossing
/// landing between the level read and the clear would otherwise be
/// cleared with the blip.
///
/// `entered` runs once an edge is confirmed pending, before the gate (a
/// port's entry counter: there it costs what it did in the old wrapper;
/// counted after the gate, LLVM copies it into every gate path).
#[inline(always)]
pub fn on_bemf_irq<S, B, I, E>(
    shared: &S,
    sensor: &mut B,
    interval: &I,
    rising: bool,
    entered: E,
) -> BemfIrq
where
    S: crate::shared_comm::IsrTiming,
    B: BemfSensor,
    I: IntervalTimer,
    E: FnOnce(),
{
    if !sensor.pending() {
        return BemfIrq::Spurious;
    }
    entered();
    if interval.count() > shared.gate_avg() >> 1 {
        sensor.clear_pending();
        BemfIrq::Open
    } else if sensor.level() == rising {
        sensor.clear_pending();
        if sensor.level() != rising {
            sensor.repend();
        }
        BemfIrq::Noise
    } else {
        BemfIrq::Camp
    }
}

/// BEMF polling (old_routine path).
fn bemf_polling<S: SharedComm, H: MotorHal>(ctx: &mut MotorContext<S, H>) {
    ctx.hal.bemf().mask_irq();
    let comp_level = ctx.hal.bemf().level();
    let rising = ctx.commutation.rising;
    ctx.bemf.update(comp_level, rising);

    if ctx.bemf.zero_cross_detected(rising) {
        // Saturate, as the action path does above: a timer wider than 16 bits
        // must not wrap a long interval into a short one.
        let interval_count = ctx.hal.interval().count().min(u16::MAX as u32) as u16;
        ctx.hal.interval().set_count(0);
        let ci = ctx.shared.commutation_interval();
        let new_ci = ctx.bemf.record_zero_cross(interval_count, ci);
        ctx.shared.set_commutation_interval(new_ci);

        if ctx.shared.zero_crosses() < MIN_ZC_FOR_ADVANCE {
            let step = ctx.commutation.advance();
            let e_com = ctx
                .commutation
                .record_interval::<H>(ctx.shared.commutation_interval() as u16);
            ctx.shared.set_e_com_time(e_com);
            ctx.hal.bridge().com_step(
                step,
                crate::bridge::drive_complementary(ctx.config.use_comp_pwm(), ctx.shared),
            );
            ctx.hal.bridge().pulse_toggle(step);
            route_bemf(ctx.hal.bemf(), ctx.commutation);
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
pub fn commutation_timer_expired<P, S, C, B, T>(
    commutation: &mut Commutation,
    bemf: &mut BemfState,
    shared: &S,
    com_timer: &mut T,
    comp: &mut C,
    bridge: &mut B,
    comp_pwm: bool,
    bidirectional: bool,
    strict_changeover: bool,
) where
    P: Timing,
    S: SharedComm,
    C: hal::BemfSensor,
    B: hal::PhaseOutput,
    T: hal::ComTimer,
{
    com_timer.disable_interrupt();
    if !shared.running() {
        comp.mask_irq();
        return;
    }

    let step = commutation.advance();
    if commutation.desync_check() {
        shared.set_desync_check_pending(true);
        commutation.set_desync_check(false);
    }
    let ci0 = shared.commutation_interval();
    let e_com = commutation.record_interval::<P>(ci0 as u16);
    shared.set_e_com_time(e_com);
    bridge.com_step(step, crate::bridge::drive_complementary(comp_pwm, shared));
    bridge.pulse_toggle(step);
    route_bemf(comp, commutation);

    let exit_interval = if bidirectional {
        P::OLD_ROUTINE_EXIT_INTERVAL / 2
    } else {
        P::OLD_ROUTINE_EXIT_INTERVAL
    };
    // The two modes are written as exclusive branches (behaviour identical
    // to the former sequence of independent `if`s, which re-read
    // old_routine three times): only one of set_old_routine /
    // transition(BemfLocked) can run per commutation, and a static
    // worst-case path should not pay for both (binz WCET gate).
    if !shared.old_routine() {
        // Interrupt mode.
        let to_old = crate::platform::e_com_us_to_avg_ticks::<P>(e_com) as u32
            > exit_interval + P::OLD_ROUTINE_REENTRY_MARGIN;
        if to_old {
            shared.set_old_routine_isr0(true);
        }
        let new_ci = bemf.update_timing_from_timer(ci0);
        shared.set_commutation_interval(new_ci);
        // Polling/interrupt exclusivity (AM32 main.c commutate
        // am32_isr.rs:122-124): the comparator interrupt path is live ONLY
        // in interrupt mode. rm32 previously enabled unconditionally, so
        // both BEMF paths ran concurrently during old_routine —
        // double-commutation risk and inconsistent interval updates.
        if !to_old {
            comp.enable_irq();
        }
        bemf.reset_after_commutation();
        shared.increment_zero_crosses_isr0();
    } else {
        // Polling mode (old_routine): comparator interrupts stay off.
        bemf.reset_after_commutation();
        shared.increment_zero_crosses_isr0();
        let zc = shared.zero_crosses();
        // Polling→interrupt changeover (AM32): the
        // zc>=20 form applies ONLY with stall_protection / rc_car_reverse;
        // the normal path is `ci < changeover` ALONE. rm32 previously
        // required zc>=20 unconditionally — and since spin-up desyncs reset
        // zero_crosses, a descent through the changeover rarely survived 20
        // commutations: THE engage lottery (forensic: ci descending
        // 2676→853, 30/30 still polling, sawtooth zc resets).
        let changeover_met = if strict_changeover {
            zc >= OLD_ROUTINE_EXIT_ZC && ci0 <= exit_interval
        } else {
            ci0 < exit_interval
        };
        if changeover_met {
            shared.transition_isr0(MotorEvent::BemfLocked);
            // Changeover: arm the interrupt path now (AM32 zcfoundroutine
            // enables comparator interrupts at this exact transition).
            comp.enable_irq();
        }
    }
}

/// BEMF zero-cross detected (COMP ISR body).
///
/// **IMPORTANT for porting**: the noise-filter loop below early-returns
/// BEFORE `comp.mask_irq()` (which is the only path that clears the
/// EXTI pending bit on most MCUs). If the platform ISR wrapper relies on
/// this function to ack the EXTI line, the early-return causes NVIC to
/// re-fire the COMP ISR forever → ISR storm → main-loop starvation.
///
/// Mitigation: the platform-specific COMP ISR wrapper owns the EXTI pending
/// bit. F051 (`ADC_COMP`) and G431 (`COMP1_2_3`) ack it at entry; L431
/// (`COMP`) and G071 (`ADC_COMP`) follow AM32's half-average-interval gate:
/// ack when the gate is open or the edge is pre-ZC noise, and leave it
/// pending ("camp") until the gate opens otherwise. See each port's
/// `interrupts.rs`.
pub fn bemf_zero_cross<C: hal::BemfSensor, I: hal::IntervalTimer, T: hal::ComTimer>(
    commutation: &Commutation,
    bemf: &mut BemfState,
    comp: &mut C,
    interval: &mut I,
    com_timer: &mut T,
) -> bool {
    for _ in 0..bemf.filter_level() {
        if comp.filter_read() == commutation.rising() {
            return false;
        }
    }
    comp.mask_irq();
    let count = interval.count().min(u16::MAX as u32) as u16;
    interval.set_count(0);
    bemf.record_zc_timing(count);
    com_timer.set_and_enable(bemf.com_timer_delay());
    true
}
