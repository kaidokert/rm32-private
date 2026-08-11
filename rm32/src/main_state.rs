//! Main loop exclusive state and logic.
//!
//! This runs in thread mode (non-ISR). Accesses shared state via atomics,
//! owns protection/telemetry/config exclusively.

use crate::config::EepromConfig;
use crate::constants::*;
use crate::control::state::{Measurements, PidState, ProtectionState, TimingState};
use crate::functions::get_abs_dif;
use crate::hal::{Adc, TelemetryUart};
use crate::shared_comm::IsrAction;
use crate::telemetry;
use crate::units::AdcCount;
use embedded_hal::digital::OutputPin;

use crate::shared_state::SharedState;

/// Compute variable PWM auto-reload value for mode 1 (interval-scaled).
pub(crate) fn variable_pwm_mode1(commutation_interval: u32, timer1_max_arr: u16) -> u16 {
    let half = timer1_max_arr as i32 / 2;
    let full = timer1_max_arr as i32;
    let result = crate::functions::map(commutation_interval as i32, 96, 200, half, full);
    result.clamp(half, full) as u16
}

/// Compute variable PWM auto-reload value for mode 2 (CPU-scaled).
pub(crate) fn variable_pwm_mode2(average_interval: u32, cpu_mhz: u8) -> u16 {
    let scale = cpu_mhz as u32 / 9;
    if average_interval < 100 && average_interval > 0 {
        (100 * scale) as u16
    } else if average_interval >= 250 || average_interval == 0 {
        (250 * scale) as u16
    } else {
        (average_interval * scale) as u16
    }
}

/// Wrong-phase-orbit discriminator: current far above the
/// duty-proportional norm (see ORBIT_TRIP_* in constants.rs). The line
/// was calibrated on the 8.16 V bench rail; current at a given duty
/// scales roughly with source voltage, so the line scales with the
/// measured vbat (a 12 V pack raises it ~1.5x — slam accel there is a
/// legitimate ~10 A operating point, clone-measured).
#[inline]
pub(crate) fn orbit_current(current_ma: i16, duty: u16, vbat_mv: u16) -> bool {
    let line = (duty as i32) * ORBIT_TRIP_SLOPE + ORBIT_TRIP_OFFSET_MA;
    let scaled = if vbat_mv > 6000 {
        line * (vbat_mv as i32) / ORBIT_CAL_VBAT_MV
    } else {
        line
    };
    (current_ma as i32) > scaled
}

/// Compute duty ceiling from eRPM and temperature limits.
/// Returns the more restrictive of the two (or 2000 if neither applies).
pub(crate) fn duty_ceiling(
    e_com_time: i32,
    motor_kv: u16,
    motor_poles: u8,
    degrees_celsius: i16,
    temperature_limit: u8,
) -> u16 {
    let k_erpm = if e_com_time > 0 {
        (600000 / e_com_time) / 10
    } else {
        0
    };
    // AM32-verbatim rpm levels (main.c:785-789), including the INTEGER
    // 32/poles inner division — this shape is load-bearing. With the bench
    // motor (kv 2220, 14 poles): low=11, high=92, floor 400 → the ceiling
    // at k_erpm 41 is ~992. The previous rm32 form (kv*poles/3200, floor
    // 600) gave ~1231 there — no clamp on a 60% commanded duty — which
    // removed AM32's anti-runaway feedback: when the rotor slows into a
    // wrong-phase orbit, the falling eRPM must pull the duty ceiling down
    // steeply or current ramps to the bench kill.
    let poles = (motor_poles as i32).max(1);
    let erpm_max = if motor_kv < 300 {
        // AM32: low_rpm_throttle_limit = 0 for very low-kv motors
        2000
    } else {
        let div = (32 / poles).max(1);
        let low_rpm = motor_kv as i32 / 100 / div;
        let high_rpm = motor_kv as i32 / 12 / div;
        if k_erpm > 0 && high_rpm > low_rpm {
            crate::functions::map(k_erpm, low_rpm, high_rpm, 400, 2000).clamp(1, 2000) as u16
        } else {
            2000
        }
    };

    let temp_max = if degrees_celsius > temperature_limit as i16 {
        crate::functions::map(
            degrees_celsius as i32,
            temperature_limit as i32 - 10,
            temperature_limit as i32 + 10,
            1000,
            1,
        )
        .clamp(1, 2000) as u16
    } else {
        2000
    };

    erpm_max.min(temp_max)
}

/// Marker type for boards without a custom LED.
pub struct NoLed;
impl OutputPin for NoLed {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl embedded_hal::digital::ErrorType for NoLed {
    type Error = core::convert::Infallible;
}

/// Main-loop exclusive state, generic over optional LED pin.
pub struct MainState<LED: OutputPin = NoLed> {
    pub protection: ProtectionState,
    pub(crate) measurements: Measurements,
    pub config: EepromConfig,

    // PID controllers (main computes adjustments, ISR applies)
    pub(crate) pid: PidState,

    // Timing (main-loop side — ISR timing is in SharedComm)
    pub(crate) timing: TimingState,
    // Board-level hardware constants (set once from BoardConfig, never change)
    pub(crate) voltage_divider: u16,
    pub(crate) millivolt_per_amp: u16,
    pub(crate) current_offset: i16,
    pub(crate) use_ntc: bool,
    /// CPU MHz for variable PWM mode 2 scaling
    pub(crate) cpu_mhz: u8,
    pub cell_count: u8,
    pub(crate) motor_kv: u16,
    pub(crate) minimum_duty: u16,
    pub(crate) target_min_bemf_counts: u8,
    pub(crate) low_cell_volt_cutoff: u16,
    pub(crate) desync_check: bool,
    /// Lifetime desync-event count (AM32 `desync_happened`). Every fire
    /// of the desync detector — the chop instrument: each event costs a
    /// duty kick-down (~15-20 ms torque hole), so events/minute IS the
    /// perceived chop rate. Read by the bench 'i' info line.
    pub desync_events: u32,
    /// Per-branch desync-response counters (instrument decisions, not
    /// outcomes): which branch each desync fire took. `dsy_fast` =
    /// fast-rotor stay-interrupt (kick-half, comparator stays armed);
    /// `dsy_demote_cur` = demote because the orbit_current sanity veto
    /// fired (current above the duty-proportional line at the event);
    /// `dsy_demote_slow` = demote for any other reason (ci >=
    /// DESYNC_STAY_INTERRUPT_CI, or already in polling mode).
    pub dsy_fast: u32,
    pub dsy_demote_cur: u32,
    /// Demote fired FROM interrupt mode with sane current (ci >=
    /// DESYNC_STAY_INTERRUPT_CI at the event).
    pub dsy_demote_slow: u32,
    /// Desync fired while ALREADY in polling mode (old_routine=1) —
    /// the cascade's tail, not its head.
    pub dsy_demote_old: u32,
    /// Wrong-phase-orbit trips (see ORBIT_TRIP_MA) — lifetime count.
    pub orbit_trips: u32,
    /// Desync-detector re-arm threshold (zero_crosses). Normally the
    /// AM32-verbatim 10; raised to DESYNC_REARM_HOLDOFF_ZC for one
    /// cycle after a fast-rotor fire. Without this, the stay-interrupt
    /// response self-loops during commanded transients: fire ->
    /// kick-half -> decel moves avg against a reference that went
    /// stale while zc<=10 -> refire at zc=11 (measured: steps program
    /// dsy=289 vs clone's 10 — the clone's full demote pauses the
    /// pipeline instead). Reset to 10 once a check passes the gate.
    pub(crate) desync_rearm_zc: u32,
    /// Consecutive 1 kHz ticks with current above ORBIT_TRIP_MA.
    pub(crate) orbit_trip_count: u16,
    /// Duty snapshot (refreshed every ~100 ms) for orbit-trip
    /// commanded-transient suppression.
    pub(crate) orbit_duty_snap: u16,
    pub(crate) orbit_duty_snap_age: u16,
    pub(crate) last_armed: bool,
    /// Set on the tick when arming transition happens
    pub just_armed: bool,
    /// Set when signal timeout handling requests a firmware reset.
    pub needs_reset: bool,
    /// Custom LED pin (NoLed if board has no custom LED)
    pub(crate) led: LED,
    pub(crate) led_counter: u16,
    /// TIM1 max auto-reload (from PWM frequency config, overwritten by apply_motor_config)
    pub(crate) timer1_max_arr: u16,
    /// Main-loop tick counter for consumed current accumulation
    pub(crate) ten_khz_counter: u32,
    // 1 kHz dispatch counter lives on SharedState (ISR increments at 20 kHz,
    // main reads + resets), matching AM32's `one_khz_loop_counter` placement
    // in `tenKhzRoutine` at main.c:1317.
}

/// MCU-specific constants — properties of the silicon, not the board PCB.
#[derive(Clone, Copy, Debug)]
pub struct ChipParams {
    /// TIM1 auto-reload at default PWM frequency.
    pub timer1_max_arr: u16,
    /// CPU frequency in MHz.
    pub cpu_mhz: u8,
}

impl MainState<NoLed> {
    /// Construct MainState from board config and chip constants.
    ///
    /// Used by both firmware and harness to ensure identical initialization.
    /// PID tuning, motor_kv, and other EEPROM-derived values are applied
    /// later via `apply_motor_config()`.
    pub fn new(board: &crate::board::BoardConfig, chip: ChipParams) -> Self {
        assert!(
            board.min_bemf_counts <= u8::MAX / 2,
            "min_bemf_counts must fit derived startup thresholds"
        );
        Self {
            protection: ProtectionState::default(),
            measurements: Measurements::default(),
            config: EepromConfig::default(),
            pid: PidState::with_stall_target(board.stall_protect_interval),
            timing: TimingState::default(),
            timer1_max_arr: chip.timer1_max_arr,
            voltage_divider: board.voltage_divider,
            millivolt_per_amp: board.millivolt_per_amp,
            current_offset: board.current_offset,
            use_ntc: board.use_ntc,
            cpu_mhz: chip.cpu_mhz,
            cell_count: 0,
            motor_kv: 2000,
            minimum_duty: 0,
            target_min_bemf_counts: board.min_bemf_counts,
            low_cell_volt_cutoff: 330,
            desync_check: false,
            desync_events: 0,
            dsy_fast: 0,
            dsy_demote_cur: 0,
            dsy_demote_slow: 0,
            dsy_demote_old: 0,
            orbit_trips: 0,
            desync_rearm_zc: 10,
            orbit_trip_count: 0,
            orbit_duty_snap: 0,
            orbit_duty_snap_age: 0,
            last_armed: false,
            just_armed: false,
            needs_reset: false,
            led: NoLed,
            led_counter: 0,
            ten_khz_counter: 0,
        }
    }
}

impl<LED: OutputPin> MainState<LED> {
    /// Read-only access to measurements.
    pub fn measurements(&self) -> &Measurements {
        &self.measurements
    }

    /// Read-only access to timing state.
    pub fn timing(&self) -> &TimingState {
        &self.timing
    }

    /// Mutable access to timing state.
    pub fn timing_mut(&mut self) -> &mut TimingState {
        &mut self.timing
    }

    /// Read-only access to PID state.
    pub fn pid(&self) -> &PidState {
        &self.pid
    }

    // --- Harness config injection setters ---

    /// Set use_current_limit on the PID controller.
    pub fn set_use_current_limit(&mut self, v: bool) {
        self.pid.set_use_current_limit(v);
    }

    /// Set use_speed_control on the PID controller.
    pub fn set_use_speed_control(&mut self, v: bool) {
        self.pid.set_use_speed_control(v);
    }

    /// Set average_interval on timing state.
    pub fn set_average_interval(&mut self, v: u32) {
        self.timing.set_average_interval(v);
    }

    /// Set last_average_interval on timing state.
    pub fn set_last_average_interval(&mut self, v: u32) {
        self.timing.set_last_average_interval(v);
    }

    /// Set battery_voltage measurement.
    pub fn set_battery_voltage(&mut self, v: crate::units::MilliVolts) {
        self.measurements.set_battery_voltage(v);
    }

    /// Set actual_current measurement.
    pub fn set_actual_current(&mut self, v: crate::units::MilliAmps) {
        self.measurements.set_actual_current(v);
    }

    /// Read motor_kv.
    pub fn motor_kv(&self) -> u16 {
        self.motor_kv
    }

    /// Set motor_kv.
    pub fn set_motor_kv(&mut self, v: u16) {
        self.motor_kv = v;
    }

    /// Read desync_check flag.
    pub fn desync_check(&self) -> bool {
        self.desync_check
    }

    /// Set desync_check flag.
    pub fn set_desync_check(&mut self, v: bool) {
        self.desync_check = v;
    }

    /// Apply EEPROM-derived motor configuration.
    ///
    /// Called after loading config from flash (firmware) or after `load_eeprom`
    /// command (harness). Updates PID tuning, motor KV, voltage cutoff, and
    /// current limit flag from the derived `MotorConfig`.
    pub fn apply_motor_config(&mut self, motor_cfg: &crate::config::MotorConfig) {
        self.pid.set_current_gains(
            motor_cfg.current_kp,
            motor_cfg.current_ki,
            motor_cfg.current_kd,
        );
        self.motor_kv = motor_cfg.motor_kv;
        self.minimum_duty = motor_cfg.minimum_duty;
        self.low_cell_volt_cutoff = motor_cfg.low_cell_volt_cutoff;
        self.timer1_max_arr = motor_cfg.timer1_max_arr;
        self.pid.set_use_current_limit(
            self.config.current_limit > 0 && self.config.current_limit < 100,
        );
    }

    /// Main loop iteration. Reads shared atomics, updates main-exclusive state.
    pub(crate) fn tick(
        &mut self,
        shared: &SharedState,
        adc: &mut dyn Adc,
        telem: &mut dyn TelemetryUart,
    ) {
        // e_com_time: read from SharedComm (ISR computes from per-step intervals)
        let e_com_time = shared.e_com_time();

        // Average interval
        self.timing.average_interval = (e_com_time / 3) as u32;

        // BEMF timeout clearing — check whether the user has released the throttle.
        // For unidirectional: newinput == 0 means stick centered.
        // For bidirectional: newinput near SERVO_CENTER means stick centered.
        // process_input zeros adjusted_input on latch, so we can't use it directly.
        let zc = shared.zero_crosses();
        let raw_input = shared.newinput();
        let stick_released = if self.config.bi_direction != 0 && shared.dshot() {
            // DShot bidir: zero means no throttle (commands are 1-47)
            raw_input == 0
        } else if self.config.bi_direction != 0 {
            // Servo bidir: dead band around center means stick released
            let db = (self.config.servo_dead_band as u16) << 1;
            let center = crate::constants::SERVO_CENTER;
            raw_input >= center.saturating_sub(db) && raw_input <= center + db
        } else {
            raw_input == 0
        };
        if zc > 1000 || stick_released {
            self.protection.bemf_timeout_happened = 0;
        }
        if zc > 100 && raw_input < 200 && !(self.config.bi_direction != 0 && shared.dshot()) {
            // Skip for DShot bidir: raw_input 48-199 is active reverse throttle
            self.protection.bemf_timeout_happened = 0;
        }
        if self.config.use_sine_start != 0
            && raw_input < crate::constants::SINE_BEMF_CLEAR_THROTTLE
            && !(self.config.bi_direction != 0 && shared.dshot())
        {
            self.protection.bemf_timeout_happened = 0;
        }
        // Stall detection: if interval timer exceeds threshold, motor has stalled.
        // C: if (INTERVAL_TIMER_COUNT > 45000 && running == 1)
        if shared.interval_timer_count() > BEMF_STALL_TIMER_THRESHOLD && shared.running() {
            // Was the chain in interrupt mode when the timeout hit? Decides
            // the recovery below; must be read BEFORE set_old_routine.
            let was_interrupt_mode = !shared.old_routine();
            // Only increment if not already latched (102 = confirmed stuck)
            if self.protection.bemf_timeout_happened != BEMF_FAULT_LATCHED {
                self.protection.bemf_timeout_happened =
                    self.protection.bemf_timeout_happened.saturating_add(1);
            }
            shared.set_old_routine(true);
            if shared.adjusted_input() < THROTTLE_MIN_SIGNAL {
                shared.request_isr_action(IsrAction::ResetIntervalTimer);
                shared.transition(crate::motor_mode::MotorEvent::StopMotor);
                shared.set_commutation_interval(DESYNC_RESET_INTERVAL);
            } else {
                shared.request_isr_action(IsrAction::CommutateKick);
            }
            shared.set_zero_crosses(0);
            // Active re-kick, UNGATED — AM32 calls zcfoundroutine() here
            // unconditionally (main.c stall block), and this is also the
            // dead-start escape: a standstill window whose static
            // comparator level mismatches the expected post-ZC level can
            // NEVER accept — only the 22.5 ms timeout advances it, and
            // without a real commutation the same stuck window repeats
            // forever (the observed 18-20 Hz dead-start class, REF at
            // timeout pace). The kick = interval reset + COM-timer re-arm
            // = one forced step to the NEXT window, AM32's implicit
            // open-loop crawl. (An earlier state-gate here came from a
            // single 7/8-vs-3/8 engage bundle; paired ABAB showed that
            // swing was lottery noise — the reference is ungated.)
            let _ = was_interrupt_mode;
            shared.request_isr_action(crate::shared_comm::IsrAction::CommutateKick);
        }

        // Dynamic BEMF timeout threshold: lenient at low throttle
        if raw_input < BEMF_LENIENT_THROTTLE {
            self.protection.bemf_timeout = BEMF_TIMEOUT_LENIENT;
        } else {
            self.protection.bemf_timeout = BEMF_TIMEOUT_STRICT;
        }

        // Desync detection (re-arm gate is dynamic — see desync_rearm_zc)
        if self.desync_check && zc > self.desync_rearm_zc {
            self.desync_rearm_zc = 10;
            let diff = get_abs_dif(
                self.timing.last_average_interval as i32,
                self.timing.average_interval as i32,
            );
            if diff > (self.timing.average_interval >> 1)
                && self.timing.average_interval < DESYNC_MAX_INTERVAL
            {
                // AM32 has `if (zero_crosses > 100) average_interval = 5000`
                // HERE — but places it AFTER zeroing zero_crosses, so it is
                // DEAD CODE and never executes (changelog 1.91 intent,
                // botched). rm32 originally "fixed" the ordering, which
                // CREATED a desync echo AM32 never has: last_average_interval
                // becomes 5000 while the real interval is ~200, so the
                // |last-avg| > avg/2 test re-fires ~10 crossings later and
                // kicks duty down a second time just as recovery starts.
                // Parity = match the reference's BEHAVIOR (no reset), not its
                // intent. (Bench 07-26: clone desyncs at 60-80% are invisible
                // <50ms blips; rm32's echoed double-kick fed the 1-2.4s churn.)
                shared.set_zero_crosses(0);
                self.desync_events = self.desync_events.wrapping_add(1);
                let desync_from_interrupt_mode = !shared.old_routine();
                // KEPT DIVERGENCE (fast-rotor desync stays in interrupt
                // mode). AM32 demotes to polling + running=0 here and its
                // main-loop-rate zcfoundroutine re-locks within ~1 ms, so
                // its desyncs at 60-80% are invisible <50 ms blips (clone
                // control, 07-26: zc resets every 1-3 s at duty>1195, speed
                // never leaves 1700-1900 Hz). rm32's polling lives on the
                // 20 kHz tick grid — at 1700 Hz e a window is ~2 samples and
                // the 3-count persistence cannot fit, so a demoted rotor
                // coasts to ~170 Hz before polling re-locks: each desync
                // cost 1-2.4 s of churn + a restart current surge. The
                // level-history probe shows the rotor never actually slips
                // at these events (extended-demag sensing gap, odd-step
                // polarity-locked), so with the comparator left armed the
                // next real crossing re-locks immediately — the clone's
                // OUTCOME, reached within rm32's architecture. Polling
                // demotion still applies below the tick-grid bandwidth.
                // Sane current required: an elevated-current desync means
                // the wrong-phase orbit may already hold — the demote IS
                // the phase reset, never skip it then.
                let current_sane = !orbit_current(
                    self.measurements.actual_current.0,
                    shared.duty_cycle(),
                    self.measurements.battery_voltage.0,
                );
                let fast_rotor =
                    current_sane && shared.commutation_interval() < DESYNC_STAY_INTERRUPT_CI;
                // Duty kick (AM32: last_duty_cycle = min_startup/2,
                // unconditional). On the fast-rotor branch the rotor is
                // still locked, so only HALVE the duty (kept divergence):
                // the full crash to ~55 recovers through the startup ramp
                // profile for ~15-20 ms — the audible chop — and its
                // recovery surge fed the supply-sag feedback loop.
                // Branch instrumentation (decisions, not outcomes): which
                // response path this fire takes, and why.
                if desync_from_interrupt_mode && fast_rotor {
                    self.dsy_fast = self.dsy_fast.wrapping_add(1);
                } else if desync_from_interrupt_mode && !current_sane {
                    self.dsy_demote_cur = self.dsy_demote_cur.wrapping_add(1);
                } else if desync_from_interrupt_mode {
                    self.dsy_demote_slow = self.dsy_demote_slow.wrapping_add(1);
                } else {
                    self.dsy_demote_old = self.dsy_demote_old.wrapping_add(1);
                }
                if desync_from_interrupt_mode && fast_rotor {
                    shared.request_isr_action(crate::shared_comm::IsrAction::DutyKickHalf);
                } else {
                    shared.request_isr_action(crate::shared_comm::IsrAction::DutyKickDown);
                }
                // Detector holdoff (see desync_rearm_zc): independent
                // bisect axis (bit2) — applies on any fast-rotor fire.
                if desync_from_interrupt_mode && fast_rotor {
                    self.desync_rearm_zc = DESYNC_REARM_HOLDOFF_ZC;
                }
                if !(desync_from_interrupt_mode && fast_rotor) {
                    // DesyncFallback first: Running→OldRoutine (sets
                    // old_routine=1). Then StopMotor conditionally:
                    // OldRoutine→Armed (sets running=0). Order matters:
                    // StopMotor before DesyncFallback would go Running→
                    // Armed, blocking DesyncFallback (Armed has no
                    // transition).
                    shared.transition(crate::motor_mode::MotorEvent::DesyncFallback);
                    if (self.config.bi_direction == 0 && shared.adjusted_input() > 47)
                        || shared.commutation_interval() > 1000
                    {
                        shared.transition(crate::motor_mode::MotorEvent::StopMotor);
                    }
                }
            }
            self.desync_check = false;
            self.timing.last_average_interval = self.timing.average_interval;
        }

        // Signal timeout — matches AM32 C `Src/main.c:1892-1918`:
        //   Armed: 0.5s (10000 ticks @ 20kHz) → disarm + request system reset
        //   Unarmed: 2s (40000 ticks)        → request system reset
        // The reset (NVIC_SystemReset on the C side, `SCB::sys_reset` here)
        // sets SFTRSTF; the AM32 bootloader sees that and skips its
        // first-chance signal-pin check, falling into the DFU loop. That's
        // what makes the BF-passthrough → AM32 Configurator flow work — BF
        // stops sending DSHOT during passthrough, the ESC times out, resets
        // into bootloader DFU, and the Configurator's BLHeli protocol talks
        // to the bootloader, not the running firmware.
        //
        // Also clear input_set so re-detection runs if the reset doesn't
        // actually fire for some reason (host-test path, IWDG-disabled bench
        // build that polls the flag from a stuck main loop, etc).
        // Signal timeout thresholds fire only after the counter exceeds the limit.
        let signal_timeout = shared.signal_timeout();
        if shared.armed() {
            if signal_timeout > crate::constants::SIGNAL_TIMEOUT_DISARM {
                shared.transition(crate::motor_mode::MotorEvent::Disarm);
                shared.set_input_set(false);
                self.needs_reset = true;
            }
        } else if shared.input_set() && signal_timeout > crate::constants::SIGNAL_TIMEOUT_UNARMED {
            shared.set_input_set(false);
            self.needs_reset = true;
        }

        // eRPM
        if !shared.stepper_sine() && e_com_time > 0 {
            self.timing.e_rpm = if shared.running() {
                (600000 / e_com_time) as u16
            } else {
                0
            };
        }

        // Armed-transition detection stays at 20 kHz so we don't miss the
        // edge by up to 1 ms. battery_voltage used inside is updated by the
        // 1 kHz block below; on the first armed transition, battery_voltage
        // is already populated because the firmware runs for seconds before
        // BF starts sending PWM.
        let armed = shared.armed();
        self.just_armed = armed && !self.last_armed;
        if self.just_armed && self.cell_count == 0 && self.config.low_voltage_cut_off == 1 {
            self.cell_count = (self.measurements.battery_voltage.0 / 370) as u8;
        }
        self.last_armed = armed;

        // 1 kHz dispatch: ADC + 3 PIDs + LVC. Matches AM32 main.c:2010-2081
        // (the PROCESS_ADC_FLAG block) plus the PID block at main.c:1397.
        // PID_LOOP_DIVIDER=20 means this block runs every 20th 20 kHz TIM6
        // tick = once per millisecond. Previously these all ran at 20 kHz
        // (20× AM32 rate); see RATE_DIVERGENCE_REPORT.md.
        //
        // Counter increment lives in `ten_khz_tick` (TIM6 ISR), matching
        // AM32 main.c:1317. Main reads + resets here. This way the 1 kHz
        // rate is correct regardless of main-loop iteration rate (no longer
        // gated by wfi — matches AM32's spinning while(1) at main.c:1843).
        //
        // NOT in this block (matches AM32, which runs them every main iter
        // OUTSIDE the PROCESS_ADC_FLAG block): duty_ceiling (main.c:2096),
        // filter_level (main.c:2112), auto_advance (main.c:2121),
        // min_bemf_counts (main.c:1862), variable_pwm (main.c:1877). They
        // run at our main-loop rate (~75 kHz post-wfi-removal).
        if shared.one_khz_counter_check_and_reset(crate::constants::PID_LOOP_DIVIDER) {
            // ADC measurements — typed conversions via AdcCount
            let smoothed_v = AdcCount(self.measurements.voltage_filter.update(adc.raw_voltage()));
            let smoothed_c = AdcCount(self.measurements.current_filter.update(adc.raw_current()));
            self.measurements.battery_voltage = smoothed_v.to_millivolts(self.voltage_divider);
            self.measurements.actual_current =
                smoothed_c.to_milliamps(self.current_offset, self.millivolt_per_amp);
            self.measurements.degrees_celsius = if self.use_ntc {
                crate::ntc::ntc_degrees(adc.raw_temperature())
            } else {
                adc.calc_temperature(adc.raw_temperature())
            };
            adc.start_conversion();

            // Publish measurements to shared state (ISR reads for EDT)
            shared.set_actual_current(self.measurements.actual_current.0);
            shared.set_battery_voltage(self.measurements.battery_voltage.0);
            shared.set_degrees_celsius(self.measurements.degrees_celsius.0);

            // Wrong-phase-orbit trip (kept divergence; see ORBIT_TRIP_*):
            // sustained current far above the duty-proportional norm while
            // locked means the BEMF acceptance chain is clocking itself off
            // switching artifacts in a wrong phase register — plausible z,
            // huge current, no desync-detector jump. Force the full AM32
            // desync response; the demote IS the phase reset.
            self.orbit_duty_snap_age += 1;
            if self.orbit_duty_snap_age >= 100 {
                self.orbit_duty_snap = shared.duty_cycle();
                self.orbit_duty_snap_age = 0;
            }
            let transient = get_abs_dif(shared.duty_cycle() as i32, self.orbit_duty_snap as i32)
                > ORBIT_TRANSIENT_DUTY as u32;
            if shared.running()
                && shared.zero_crosses() > 1000
                && !transient
                && orbit_current(
                    self.measurements.actual_current.0,
                    shared.duty_cycle(),
                    self.measurements.battery_voltage.0,
                )
            {
                self.orbit_trip_count += 1;
                if self.orbit_trip_count > ORBIT_TRIP_MS {
                    self.orbit_trip_count = 0;
                    self.orbit_trips = self.orbit_trips.wrapping_add(1);
                    self.desync_events = self.desync_events.wrapping_add(1);
                    shared.set_zero_crosses(0);
                    shared.request_isr_action(crate::shared_comm::IsrAction::DutyKickDown);
                    shared.transition(crate::motor_mode::MotorEvent::DesyncFallback);
                    if shared.adjusted_input() > 47 || shared.commutation_interval() > 1000 {
                        shared.transition(crate::motor_mode::MotorEvent::StopMotor);
                    }
                }
            } else {
                self.orbit_trip_count = 0;
            }

            // Low voltage cutoff (AM32 main.c:2045-2071). Counter increments
            // at 1 kHz now → LVC_NORMAL_THRESHOLD=10000 = 10 sec sustained
            // low voltage (was previously 10000/20kHz = 0.5 sec — 20× faster
            // than AM32 design).
            // Mode 1: per-cell threshold (cell_count * low_cell_volt_cutoff)
            // Mode 2: absolute threshold (absolute_voltage_cutoff in 0.5V)
            if self.config.low_voltage_cut_off != 0 {
                let threshold = if self.config.low_voltage_cut_off == 2 {
                    self.config.absolute_voltage_cutoff as u16
                } else {
                    self.cell_count as u16 * self.low_cell_volt_cutoff
                };
                if self.measurements.battery_voltage.0 < threshold && threshold > 0 {
                    self.protection.low_voltage_count += 1;
                } else if !self.protection.low_voltage_cutoff {
                    self.protection.low_voltage_count = 0;
                }
                let lvc_limit = if shared.stepper_sine() {
                    LVC_STARTUP_THRESHOLD
                } else {
                    LVC_NORMAL_THRESHOLD
                };
                if self.protection.low_voltage_count > lvc_limit {
                    self.protection.low_voltage_cutoff = true;
                    shared.request_isr_action(crate::shared_comm::IsrAction::AllOff);
                    shared.transition(crate::motor_mode::MotorEvent::Disarm);
                }
            }

            // Stall protection PID — boosts duty at low RPM for crawlers/RC cars
            if self.config.stall_protection != 0 && shared.running() {
                let boost = self.pid.tick_stall(shared.commutation_interval() as i32);
                shared.set_stall_protection_adjust(boost);
            }

            // Current limit PID — reduces duty when current exceeds limit
            {
                let target = self.config.current_limit as i32 * 200;
                let min_duty = self.minimum_duty.min(i16::MAX as u16) as i16;
                let ceiling = self.pid.tick_current_limit(
                    self.measurements.actual_current.0,
                    target,
                    min_duty,
                    shared.running(),
                );
                shared.set_current_limit_adjust(ceiling);
            }

            // Speed control PID — closed-loop RPM control
            if let Some(override_input) =
                self.pid
                    .tick_speed_control(shared.e_com_time(), zc, shared.running())
            {
                shared.set_newinput(override_input.clamp(48, 2047));
            }
        }

        // Telemetry send
        if shared.send_telemetry() {
            let mut pkt = [0u8; 10];
            let voltage_cv = self.measurements.battery_voltage.to_centivolts();
            let current_ca = self.measurements.actual_current.to_centiamps();
            telemetry::make_telem_package(
                &mut pkt,
                self.measurements.degrees_celsius.to_i8(),
                voltage_cv,
                current_ca,
                (self.measurements.consumed_current / 1000) as u16, // µAh → mAh
                self.timing.e_rpm, // already in units of 100 eRPM (600000/e_com_time)
            );
            telem.send_dma(&pkt);
            shared.set_send_telemetry(false);
        }

        // Consumed current accumulation (1s interval at ~20kHz)
        // TODO: counter incremented in main loop (variable rate), not ISR.
        // Matches C firmware behavior but integration is approximate.
        self.ten_khz_counter += 1;
        if self.ten_khz_counter > 20000 {
            self.measurements.consumed_current += self.measurements.actual_current.0 as i32;
            self.ten_khz_counter = 0;
        }

        // Variable PWM — adjust tim1_arr based on commutation speed
        if self.config.variable_pwm == 1 {
            shared.set_tim1_arr(variable_pwm_mode1(
                shared.commutation_interval(),
                self.timer1_max_arr,
            ));
        } else if self.config.variable_pwm == 2 {
            shared.set_tim1_arr(variable_pwm_mode2(
                self.timing.average_interval,
                self.cpu_mhz,
            ));
        } else {
            // variable_pwm=0: publish the EEPROM-derived ARR so ISR uses it
            shared.set_tim1_arr(self.timer1_max_arr);
        }

        // eRPM + temperature duty ceiling
        let mut dmax = duty_ceiling(
            e_com_time,
            self.motor_kv,
            self.config.motor_poles,
            self.measurements.degrees_celsius.0,
            self.config.temperature_limit,
        );
        // Reclimb clamp (KEPT DIVERGENCE, minz blind-amp-clamp shape):
        // while the lock is unconfirmed (zero_crosses below threshold —
        // fresh engage OR post-fall recovery), cap the ceiling so the
        // reclimb toward a high commanded duty cannot surge. Measured
        // need: after a fall at 60-70% commanded, the recovery reclimb
        // slewed straight to duty 1412+ mid-re-spin, pulling 4.2-4.7A
        // -> PSU sag to 5.0-5.3V -> VBAT guard kill (the clone never
        // falls, so AM32's map alone never faces this). Cut demand while
        // the estimator is uncertain; the cap releases on confirmation
        // and the normal ramp takes duty to commanded.
        if shared.zero_crosses() < RECLIMB_CONFIRM_ZC {
            dmax = dmax.min(RECLIMB_DUTY_CAP);
        }
        shared.set_duty_maximum(dmax);

        // Require stronger BEMF confirmation during startup.
        if zc < 5 {
            let counts = if self.config.bi_direction != 0 {
                self.target_min_bemf_counts + 1
            } else {
                self.target_min_bemf_counts * 2
            };
            shared.set_min_bemf_counts(counts);
        } else {
            shared.set_min_bemf_counts(self.target_min_bemf_counts);
        }

        // Filter level — dynamic based on motor speed
        let filter = if zc < 100 && shared.commutation_interval() > 500 {
            12u8
        } else if shared.commutation_interval() < 50 {
            2
        } else {
            crate::functions::map(self.timing.average_interval as i32, 100, 500, 3, 12) as u8
        };
        shared.set_filter_level(filter);

        // Commutation advance (AM32 main.c:900-905): dynamic auto-advance
        // scales with duty; otherwise the STATIC advance_level-derived
        // temp_advance applies. rm32 previously published nothing in the
        // static case, leaving the ISR at 0° advance — the largest single
        // divergence vs AM32 at factory settings (which run 15°).
        // NOTE: BemfState::sync_config treats 0 as "no update"; a static
        // advance of genuinely 0 matches the ISR-side init value, so the
        // only unreachable transition is a runtime nonzero→0 change.
        if self.config.auto_advance != 0 {
            let level =
                crate::functions::map(shared.duty_cycle_setpoint() as i32, 100, 2000, 13, 23) as u8;
            shared.set_auto_advance(level);
        } else {
            shared.set_auto_advance(self.config.temp_advance());
        }

        // Note: send_esc_info_flag is checked and cleared by firmware main.rs
        // after sending the actual packet. MainState does not own this flag.

        // Custom LED: blink with throttle, solid when high
        {
            let input = shared.adjusted_input();
            self.led_counter = self.led_counter.wrapping_add(1);
            if (47..1947).contains(&input) {
                if self.led_counter > crate::constants::LED_BLINK_HALF_PERIOD {
                    let _ = self.led.set_high();
                } else {
                    let _ = self.led.set_low();
                }
                if self.led_counter > crate::constants::LED_BLINK_HALF_PERIOD * 2 {
                    self.led_counter = 0;
                }
            } else if input > crate::constants::LED_HIGH_THROTTLE {
                let _ = self.led.set_high();
            } else {
                let _ = self.led.set_low();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Variable PWM mode 2 ---

    #[test]
    fn vpwm2_clamps_low() {
        // avg < 100 → floor at 100 * scale
        assert_eq!(variable_pwm_mode2(50, 64), (100 * (64 / 9)) as u16);
    }

    #[test]
    fn vpwm2_clamps_high() {
        // avg >= 250 → ceiling at 250 * scale
        assert_eq!(variable_pwm_mode2(300, 64), (250 * (64 / 9)) as u16);
    }

    #[test]
    fn vpwm2_scales_mid() {
        // 100 <= avg < 250 → avg * scale
        assert_eq!(variable_pwm_mode2(150, 64), (150 * (64 / 9)) as u16);
    }

    #[test]
    fn vpwm2_zero_interval_clamps_high() {
        assert_eq!(variable_pwm_mode2(0, 64), (250 * (64 / 9)) as u16);
    }

    // --- Variable PWM mode 1 ---

    #[test]
    fn vpwm1_fast_interval() {
        let arr = variable_pwm_mode1(96, 1999);
        assert_eq!(arr, 999); // maps to max_arr/2
    }

    #[test]
    fn vpwm1_slow_interval() {
        let arr = variable_pwm_mode1(200, 1999);
        assert_eq!(arr, 1999); // maps to max_arr
    }

    // --- Duty ceiling ---

    #[test]
    fn duty_ceiling_no_limits() {
        assert_eq!(duty_ceiling(0, 2000, 14, 25, 80), 2000);
    }

    #[test]
    fn duty_ceiling_bench_regime_am32_verbatim() {
        // Bench motor (kv byte 55 → 2220, 14 poles): AM32 main.c:785-789
        // gives low=11k/high=92k eRPM, floor 400. At the wrong-phase-orbit
        // operating point (k_erpm=41, e_com≈1446 µs) the ceiling must clamp
        // a 60% command (1216) — this feedback is what unwinds the runaway.
        let dc = duty_ceiling(1446, 2220, 14, 25, 141);
        assert!(dc < 1216, "ceiling {} must clamp 60% duty", dc);
        assert_eq!(dc, 992); // map(41, 11, 92, 400, 2000)
        // At the healthy 60% operating point (f_e 1667 Hz → e_com 600 µs,
        // k_erpm 100 > high_rpm) there is no clamp.
        assert_eq!(duty_ceiling(600, 2220, 14, 25, 141), 2000);
        // Very low-kv motors: AM32 disables the limiter entirely.
        assert_eq!(duty_ceiling(1446, 280, 14, 25, 141), 2000);
    }

    #[test]
    fn duty_ceiling_temp_reduces() {
        let dc = duty_ceiling(0, 2000, 14, 85, 80);
        assert!(dc < 2000, "expected reduced duty, got {}", dc);
    }

    #[test]
    fn duty_ceiling_high_poles_no_panic() {
        // motor_poles > 32 must not divide by zero
        let dc = duty_ceiling(1000, 2000, 40, 25, 80);
        assert!(dc > 0);
    }

    #[test]
    fn duty_ceiling_takes_minimum() {
        // Both limits active → should return the lower one
        let dc = duty_ceiling(100, 2000, 14, 85, 80);
        assert!(dc < 2000);
    }

    // --- Stall detection (BEMF timeout increment) ---

    struct MockAdc {
        raw_current: u16,
    }
    impl MockAdc {
        fn new() -> Self {
            Self { raw_current: 0 }
        }

        fn with_raw_current(raw_current: u16) -> Self {
            Self { raw_current }
        }
    }
    impl crate::hal::Adc for MockAdc {
        fn start_conversion(&mut self) {}
        fn raw_voltage(&self) -> u16 {
            0
        }
        fn raw_current(&self) -> u16 {
            self.raw_current
        }
        fn raw_temperature(&self) -> u16 {
            0
        }
        fn calc_temperature(&self, _: u16) -> crate::units::DegreesCelsius {
            crate::units::DegreesCelsius(25)
        }
    }

    struct MockTelem;
    impl crate::hal::TelemetryUart for MockTelem {
        fn send_dma(&mut self, _: &[u8]) {}
    }

    fn make_test_main_state() -> MainState {
        MainState::new(
            &crate::board::BoardConfig::DEFAULT,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
        )
    }

    #[test]
    #[should_panic(expected = "min_bemf_counts must fit derived startup thresholds")]
    fn rejects_min_bemf_counts_that_overflow_startup_thresholds() {
        let board = crate::board::BoardConfig {
            min_bemf_counts: 128,
            ..crate::board::BoardConfig::DEFAULT
        };
        let _ = MainState::new(
            &board,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
        );
    }

    #[test]
    fn stall_detection_increments_timeout() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine); // running=true
        shared.set_interval_timer_count(50000); // > 45000 threshold
        shared.set_adjusted_input(100); // above throttle min

        let mut main = make_test_main_state();
        assert_eq!(main.protection.bemf_timeout_happened, 0);

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(
            main.protection.bemf_timeout_happened > 0,
            "bemf_timeout_happened should increment on stall"
        );
    }

    #[test]
    fn stall_detection_does_not_trigger_below_threshold() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);
        shared.set_interval_timer_count(40000); // below 45000

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert_eq!(main.protection.bemf_timeout_happened, 0);
    }

    #[test]
    fn current_limit_uses_derived_minimum_duty() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.minimum_duty_cycle = 5;
        main.config.current_limit = 1;
        main.config.current_p = 100;
        main.config.rc_car_reverse = 1;
        main.config.normalize_after_load();

        let motor_cfg = main.config.derive_motor_config(1999, 60, 1, false);
        main.apply_motor_config(&motor_cfg);

        let mut adc = MockAdc::with_raw_current(4095);
        for _ in 0..1000 {
            // The current-limit PID runs in the 1 kHz dispatch block.
            for _ in 0..crate::constants::PID_LOOP_DIVIDER {
                shared.one_khz_counter_inc();
            }
            main.tick(&shared, &mut adc, &mut MockTelem);
        }

        assert_eq!(shared.current_limit_adjust(), motor_cfg.minimum_duty);
    }

    // --- LVC tests ---
    // REQ-PROT-LVC: Low voltage cutoff protection

    #[test]
    fn min_bemf_counts_are_stricter_during_unidirectional_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_zero_crosses(4);

        let mut main = make_test_main_state();
        main.config.bi_direction = 0;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.min_bemf_counts(), 6);
    }

    #[test]
    fn min_bemf_counts_are_stricter_during_bidirectional_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_zero_crosses(4);

        let mut main = make_test_main_state();
        main.config.bi_direction = 1;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.min_bemf_counts(), 4);
    }

    #[test]
    fn min_bemf_counts_use_target_after_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_zero_crosses(5);

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.min_bemf_counts(), 3);
    }

    #[test]
    fn min_bemf_counts_follow_configured_target() {
        use crate::shared_state::SharedState;

        let board = crate::board::BoardConfig {
            min_bemf_counts: 5,
            ..crate::board::BoardConfig::DEFAULT
        };
        let shared = SharedState::new();
        shared.set_zero_crosses(4);

        let mut main = MainState::new(
            &board,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
        );

        main.config.bi_direction = 0;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert_eq!(shared.min_bemf_counts(), 10);

        main.config.bi_direction = 1;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert_eq!(shared.min_bemf_counts(), 6);

        shared.set_zero_crosses(5);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert_eq!(shared.min_bemf_counts(), 5);
    }

    #[test]
    fn lvc_mode1_per_cell_triggers_disarm() {
        use crate::motor_mode::MotorMode;
        use crate::shared_comm::MainControl;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        // Threshold = 3 * 330 = 990mV. Set voltage below.
        main.set_battery_voltage(crate::units::MilliVolts(500));
        // Pre-fill count near threshold
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        // LVC now runs inside the 1 kHz block.
        for _ in 0..crate::constants::PID_LOOP_DIVIDER {
            shared.one_khz_counter_inc();
        }

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // LVC should have triggered: count exceeded threshold → disarm
        assert!(
            !shared.armed(),
            "motor should be disarmed after LVC trigger"
        );
        assert_eq!(MainControl::isr_action(&shared), IsrAction::AllOff);
    }

    #[test]
    fn lvc_does_not_run_before_one_khz_dispatch() {
        use crate::motor_mode::MotorMode;
        use crate::shared_comm::MainControl;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(shared.armed());
        assert_eq!(MainControl::isr_action(&shared), IsrAction::None);
    }

    #[test]
    fn lvc_mode1_recovery_inhibit() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        // Trigger LVC first
        main.set_battery_voltage(crate::units::MilliVolts(500));
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // Now raise voltage above threshold
        shared.set_motor_mode(MotorMode::OldRoutine); // re-arm for test
        main.set_battery_voltage(crate::units::MilliVolts(1500));
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // Count should NOT reset because low_voltage_cutoff latch is set
        assert!(
            main.protection.low_voltage_count() > 0,
            "count should not reset after LVC latch — recovery inhibited"
        );
    }

    #[test]
    fn lvc_mode2_absolute_cutoff() {
        // Mode 2: absolute voltage cutoff using EEPROM threshold directly.
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2; // absolute mode
        main.config.absolute_voltage_cutoff = 100; // threshold (raw EEPROM value)
        main.cell_count = 0; // no cells — doesn't matter for mode 2
        main.set_battery_voltage(crate::units::MilliVolts(50)); // below threshold
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        // LVC now runs inside the 1 kHz block.
        for _ in 0..crate::constants::PID_LOOP_DIVIDER {
            shared.one_khz_counter_inc();
        }
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // Mode 2: battery (50) < absolute_voltage_cutoff (100) → LVC triggers
        assert!(
            !shared.armed(),
            "mode 2 should disarm when voltage below absolute threshold"
        );
    }

    #[test]
    fn lvc_mode2_above_threshold_no_disarm() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2;
        main.config.absolute_voltage_cutoff = 100;
        main.set_battery_voltage(crate::units::MilliVolts(150)); // above threshold
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(
            shared.armed(),
            "mode 2 should not disarm when voltage above threshold"
        );
    }

    // --- Signal timeout tests ---
    // REQ-SAFE-SIGNAL_TIMEOUT

    #[test]
    fn signal_timeout_disarms_when_armed() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);
        shared.set_input_set(true);
        // Push signal timeout past armed threshold
        for _ in 0..=crate::constants::SIGNAL_TIMEOUT_DISARM {
            shared.increment_signal_timeout();
        }

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(!shared.armed(), "should disarm after signal timeout");
        assert!(!shared.input_set(), "should clear input_set");
    }

    #[test]
    fn signal_timeout_unarmed_resets_input() {
        // C firmware has a 2-second unarmed timeout that resets input detection.
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::Disarmed);
        shared.set_input_set(true);
        // Push signal timeout past unarmed threshold (40000)
        for _ in 0..45000u32 {
            shared.increment_signal_timeout();
        }

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // Unarmed timeout resets input_set so protocol can be re-detected
        assert!(
            !shared.input_set(),
            "unarmed signal timeout should reset input_set"
        );
    }

    /// REQ-RESET-ON-TIMEOUT: When signal_timeout fires (armed >0.5s or
    /// unarmed >2s), MainState::tick must request a system reset via
    /// `set_needs_reset(true)`. Matches AM32's NVIC_SystemReset() at
    /// Src/main.c:1904 and :1917 — the only way the AM32 bootloader DFU
    /// loop ever activates from a running firmware, which is what BF's
    /// passthrough mode and the AM32 Configurator depend on.
    #[test]
    fn signal_timeout_armed_requests_reset() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::OldRoutine);
        shared.set_input_set(true);
        let mut main = make_test_main_state();
        assert!(!main.needs_reset, "starts not requesting reset");
        for _ in 0..=crate::constants::SIGNAL_TIMEOUT_DISARM {
            shared.increment_signal_timeout();
        }
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(
            main.needs_reset,
            "armed timeout (>0.5s) must request system reset"
        );
    }

    #[test]
    fn signal_timeout_unarmed_requests_reset() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::Disarmed);
        shared.set_input_set(true);
        let mut main = make_test_main_state();
        assert!(!main.needs_reset, "starts not requesting reset");
        for _ in 0..45000u32 {
            shared.increment_signal_timeout();
        }
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(
            main.needs_reset,
            "unarmed timeout (>2s) must request system reset"
        );
    }

    // --- Sine-to-BLDC changeover tests ---
    // REQ-MOTOR-SINE_STEPPER_CONTROL
    //
    // The changeover from sine mode to sensorless BLDC is handled in the
    // firmware main loop (rm32_stm32/src/bin/main.rs), not in MainState::tick().
    // These tests document what the changeover MUST set, based on C code
    // (main.c lines 2238-2255). The firmware handler currently misses 8 of 13
    // state assignments.
    //
    // Missing in Rust firmware changeover handler:
    //   - average_interval = 9000
    //   - last_average_interval = average_interval
    //   - SET_INTERVAL_TIMER_COUNT(9000)
    //   - prop_brake_active = 0
    //   - step = changeover_step (ignored via `..`)
    //   - commutate() — first commutation step
    //   - generatePwmTimerEvent()
    //   - stall_protect_minimum_duty applied if stall_protection ON
    //
    // These can't be unit-tested here because they're firmware-specific
    // (HAL calls like commutate() and generatePwmTimerEvent() have no
    // platform-independent equivalent). They need a hardware smoke test
    // or a firmware-level integration test.

    // --- EDT arming/disarming tests ---
    // Coverage gap #5: EDT disarm on zero-throttle
    //
    // Three bugs found:
    //
    // 1. EDT disarm on zero throttle NOT IMPLEMENTED:
    //    C: if (tocheck==0 && EDT_ARM_ENABLE) EDT_ARMED=0
    //    Rust: zero throttle sets newinput=0 but doesn't clear edt_armed
    //
    // 2. EDT_ARM_ENABLE not derived from EEPROM config:
    //    C: input_type == EDTARM_IN (4) → EDT_ARM_ENABLE=1
    //    Rust: EdtArm enum variant exists but nobody checks it
    //
    // 3. edt_arm_enable parameter sourced wrong:
    //    C: separate global from EEPROM config
    //    Rust firmware: passes extended_telemetry() (wrong — that's
    //    "EDT currently active", not "EDT arming enabled by config")
    //
    // The EDT throttle gate test (edt_armed_throttle_gate.txt) covers
    // the gate itself but not the disarm-on-zero path.

    #[test]
    fn signal_timeout_unarmed_without_prior_input_does_not_reset() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new();
        shared.set_motor_mode(MotorMode::Disarmed);
        assert!(!shared.input_set());
        for _ in 0..=crate::constants::SIGNAL_TIMEOUT_UNARMED {
            shared.increment_signal_timeout();
        }

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(!main.needs_reset);
    }
}
