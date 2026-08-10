//! SharedComm trait — abstraction over ISR↔main shared state.
//!
//! On real hardware, this is implemented via atomics (SharedState).
//! For testing, TestShared implements it with Cell fields.
//!
//! Decomposed into sub-traits by data-flow direction:
//! - `MotorState`: motor mode state machine (bidirectional)
//! - (future) `IsrTiming`: ISR→main timing data
//! - (future) `MainControl`: main→ISR control data

use crate::motor_mode::{MotorEvent, MotorMode};

/// Action requested from main loop to ISR context.
///
/// Priority-ordered: AllOff supersedes ResetIntervalTimer (if both are
/// needed, the motor is being killed so the timer reset is moot).
/// Stored as AtomicU8 in SharedState. Main writes via `request_isr_action`;
/// ISR reads via `isr_action`, executes, and clears to None.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum IsrAction {
    /// No pending action.
    None = 0,
    /// Reset interval timer to 0 (stall handler, matches C's zcfoundroutine).
    ResetIntervalTimer = 1,
    /// Fast-rotor desync recovery (KEPT DIVERGENCE): halve the applied
    /// duty (floor min_startup/2) instead of crashing to min_startup/2.
    /// Used only on the stay-interrupt desync branch where the rotor is
    /// known to still be locked — recovery from duty/2 re-slews at the
    /// high-rpm ramp in ~2 ms instead of a ~15-20 ms crawl from ~55 (the
    /// audible chop, and the surge that fed supply-sag feedback).
    DutyKickHalf = 2,
    /// Desync recovery: drop the applied duty to min_startup/2 so the
    /// restart ramps from low (AM32 desync handling; minz
    /// am32_control.rs:284).
    DutyKickDown = 3,
    /// BEMF-timeout recovery: re-arm the commutation chain NOW (the COM
    /// timer may be dead after a timeout — AM32's zcfoundroutine actively
    /// re-commutates; rm32's previous recovery was passive).
    CommutateKick = 4,
    /// Kill all FETs + mask comparator interrupts (LVC, stuck rotor).
    /// MUST stay the highest value — `request_isr_action` uses fetch_max
    /// for priority, and a kill outranks every recovery action.
    AllOff = 5,
}

impl IsrAction {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            x if x == Self::ResetIntervalTimer as u8 => Self::ResetIntervalTimer,
            x if x == Self::DutyKickHalf as u8 => Self::DutyKickHalf,
            x if x == Self::DutyKickDown as u8 => Self::DutyKickDown,
            x if x == Self::CommutateKick as u8 => Self::CommutateKick,
            x if x == Self::AllOff as u8 => Self::AllOff,
            _ => Self::None,
        }
    }
}

/// Motor mode state machine — bidirectional ISR↔main.
///
/// Only two methods require implementation: `motor_mode()` and `set_motor_mode()`.
/// All convenience getters/setters and the `transition()` method are derived.
pub trait MotorState {
    fn motor_mode(&self) -> MotorMode;
    fn set_motor_mode(&self, mode: MotorMode);

    /// Apply a state transition event atomically.
    fn transition(&self, event: MotorEvent) {
        let new = self.motor_mode().transition(event);
        if new != self.motor_mode() {
            self.set_motor_mode(new);
        }
    }

    fn armed(&self) -> bool {
        self.motor_mode().is_armed()
    }
    fn running(&self) -> bool {
        self.motor_mode().is_running()
    }
    fn old_routine(&self) -> bool {
        self.motor_mode().is_old_routine()
    }
    fn stepper_sine(&self) -> bool {
        self.motor_mode().is_stepper_sine()
    }

    fn set_armed(&self, v: bool) {
        if v && !self.armed() {
            self.set_motor_mode(MotorMode::Armed);
        } else if !v {
            self.set_motor_mode(MotorMode::Disarmed);
        }
    }
    fn set_running(&self, v: bool) {
        if v && !self.running() {
            self.set_motor_mode(MotorMode::OldRoutine);
        } else if !v && self.running() {
            self.set_motor_mode(MotorMode::Armed);
        }
    }
    fn set_old_routine(&self, v: bool) {
        if v && self.running() {
            self.set_motor_mode(MotorMode::OldRoutine);
        } else if !v && self.old_routine() {
            self.set_motor_mode(MotorMode::Running);
        }
    }
    fn set_stepper_sine(&self, v: bool) {
        if v {
            self.set_motor_mode(MotorMode::StepperSine);
        } else if self.stepper_sine() {
            self.set_motor_mode(MotorMode::Armed);
        }
    }
}

/// ISR-produced timing and state data consumed by the main loop.
///
/// ISR writes these values each commutation step or tick;
/// main loop reads them for RPM calculation, stall detection,
/// speed gating, and desync detection.
pub trait IsrTiming {
    fn zero_crosses(&self) -> u32;
    fn set_zero_crosses(&self, v: u32);
    fn increment_zero_crosses(&self);
    fn commutation_interval(&self) -> u32;
    fn set_commutation_interval(&self, v: u32);
    fn e_com_time(&self) -> i32;
    fn set_e_com_time(&self, _v: i32) {}
    fn interval_timer_count(&self) -> u32 {
        0
    }
    fn set_interval_timer_count(&self, _v: u32) {}
    fn signal_timeout(&self) -> u16;
    fn increment_signal_timeout(&self);

    /// Current duty cycle (ISR writes each tick, main reads for bidir speed gate).
    fn duty_cycle(&self) -> u16 {
        0
    }
    fn set_duty_cycle(&self, _v: u16) {}

    /// Motor direction (ISR syncs from commutation, input flips on bidir).
    fn forward(&self) -> bool {
        true
    }
    fn set_forward(&self, _v: bool) {}

    /// Increment 1 kHz dispatch counter (TIM6 ISR side, 20 kHz). Matches
    /// AM32's `one_khz_loop_counter++` at main.c:1317.
    fn one_khz_counter_inc(&self) {}
    /// Main-side: returns true and resets counter if it has exceeded
    /// `divider` (typically PID_LOOP_DIVIDER = 20). Matches AM32's check
    /// at main.c:1397.
    fn one_khz_counter_check_and_reset(&self, _divider: u8) -> bool {
        false
    }

    /// ISR-side (20 kHz): increment the interval-telemetry counter; if it
    /// has exceeded `limit`, reset it and return true (fire telemetry).
    /// Matches AM32's `telem_ms_count` block at main.c:1664-1672.
    fn telem_counter_check_and_inc(&self, limit: u16) -> bool;
}

/// Main-loop-produced control data consumed by the ISR.
///
/// Main loop computes throttle mapping, PID outputs, PWM config,
/// and measurement data; ISR reads them each tick.
pub trait MainControl {
    fn adjusted_input(&self) -> u16;
    fn set_adjusted_input(&self, v: u16);
    fn duty_cycle_setpoint(&self) -> u16;
    fn set_duty_cycle_setpoint(&self, v: u16);

    fn stall_protection_adjust(&self) -> u16 {
        0
    }
    fn set_stall_protection_adjust(&self, _v: u16) {}

    /// Current limit duty ceiling (main PID publishes, ISR clamps duty).
    fn current_limit_adjust(&self) -> u16 {
        2000
    }
    fn set_current_limit_adjust(&self, _v: u16) {}

    /// Proportional brake active (main sets, ISR reads for brake-on-stop).
    fn prop_brake_active(&self) -> bool {
        false
    }
    fn set_prop_brake_active(&self, _v: bool) {}

    /// ISR action request from main loop.
    ///
    /// Variant values are ordered by priority; implementations keep the
    /// highest pending action and clear only the action the ISR handled.
    fn isr_action(&self) -> IsrAction {
        IsrAction::None
    }
    fn request_isr_action(&self, _action: IsrAction) {}
    fn clear_isr_action(&self, _action: IsrAction) {}

    /// Public-API compatibility shims (upstream models all-off as its own
    /// one-shot channel; here it routes through the priority-ordered
    /// IsrAction request so recovery actions and all-off can't race).
    fn all_off_request(&self) -> bool {
        self.isr_action() == IsrAction::AllOff
    }
    fn request_all_off(&self) {
        self.request_isr_action(IsrAction::AllOff);
    }
    fn clear_all_off_request(&self) {
        self.clear_isr_action(IsrAction::AllOff);
    }

    /// Sine changeover step request (0 = none, 1-6 = execute changeover with step).
    /// Main sets during sine changeover; ISR applies com_step + enables interrupts.
    fn changeover_step(&self) -> u8;
    fn set_changeover_step(&self, step: u8);

    /// Desync check flag (ISR sets on BEMF zero-cross, main clears after processing).
    fn desync_check_pending(&self) -> bool {
        false
    }
    fn set_desync_check_pending(&self, _v: bool) {}

    /// TIM1 auto-reload value (variable PWM). Main publishes, ISR applies.
    fn tim1_arr(&self) -> u16 {
        1999
    }
    fn set_tim1_arr(&self, _v: u16) {}

    /// Max duty cycle (eRPM/temperature limiting). Main publishes, ISR applies.
    fn duty_maximum(&self) -> u16 {
        2000
    }
    fn set_duty_maximum(&self, _v: u16) {}

    /// BEMF filter level. Main computes based on motor speed, ISR uses for ZC detection.
    fn filter_level(&self) -> u8 {
        5
    }
    fn set_filter_level(&self, _v: u8) {}

    /// Min BEMF counts for zero-cross acceptance. Main adjusts during startup.
    fn min_bemf_counts(&self) -> u8 {
        2
    }
    fn set_min_bemf_counts(&self, _v: u8) {}

    /// Auto advance level. Main computes from duty cycle, ISR uses for timing.
    fn auto_advance(&self) -> u8 {
        0
    }
    fn set_auto_advance(&self, _v: u8) {}

    /// Measurement publish for EDT telemetry (main writes, ISR reads).
    fn battery_voltage(&self) -> u16 {
        0
    }
    fn set_actual_current(&self, _v: i16) {}
    fn set_battery_voltage(&self, _v: u16) {}
    fn set_degrees_celsius(&self, _v: i16) {}
}

/// Remaining shared state — input detection, flags, protocol.
///
/// Requires `MotorState`, `IsrTiming`, and `MainControl`.
pub trait SharedComm: MotorState + IsrTiming + MainControl {
    fn input_set(&self) -> bool;
    fn set_input_set(&self, v: bool);
    fn dshot_telemetry(&self) -> bool;

    /// Whether detected input is DShot (vs servo). ISR transfer handler sets this.
    fn is_dshot(&self) -> bool {
        false
    }
    fn set_is_dshot(&self, _v: bool) {}

    fn newinput(&self) -> u16;
    fn set_newinput(&self, v: u16);

    fn send_telemetry(&self) -> bool;
    fn set_send_telemetry(&self, v: bool);

    fn save_settings_flag(&self) -> bool {
        false
    }
    fn set_save_settings_flag(&self, _v: bool) {}
    fn send_esc_info_flag(&self) -> bool {
        false
    }
    fn set_send_esc_info_flag(&self, _v: bool) {}
}
