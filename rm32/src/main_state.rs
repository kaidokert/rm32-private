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
use crate::tuning::{DesyncPolicy, TuningPolicy, orbit_current};
use crate::units::AdcCount;
use embedded_hal::digital::OutputPin;

use crate::shared_state::SharedState;

/// Compute variable PWM auto-reload value for mode 1 (interval-scaled).
pub(crate) fn variable_pwm_mode1<P: Timing>(commutation_interval: u32, timer1_max_arr: u16) -> u16 {
    let half = timer1_max_arr as i32 / 2;
    let full = timer1_max_arr as i32;
    let result = crate::functions::map(
        commutation_interval as i32,
        P::VARIABLE_PWM1_LO as i32,
        P::VARIABLE_PWM1_HI as i32,
        half,
        full,
    );
    result.clamp(half, full) as u16
}

/// Compute variable PWM auto-reload value for mode 2 (CPU-scaled).
pub(crate) fn variable_pwm_mode2<P: Timing>(average_interval: u32, cpu_mhz: u8) -> u16 {
    let bounded = if average_interval == 0 {
        P::VARIABLE_PWM2_HI
    } else {
        average_interval.clamp(P::VARIABLE_PWM2_LO, P::VARIABLE_PWM2_HI)
    };
    // PWM geometry uses the original 2 MHz interval scale, independent of
    // the port's interval timer. Bound first to keep very long intervals safe.
    // At a 2 MHz timer the conversion is the identity and folds away.
    let average_interval = if P::TIMER_HZ == 2_000_000 {
        bounded
    } else {
        (bounded as u64 * 2_000_000 / P::TIMER_HZ as u64) as u32
    };
    let scale = cpu_mhz as u32 / 9;
    (average_interval * scale) as u16
}

/// Compute duty ceiling from eRPM and temperature limits.
/// Returns the more restrictive of the two (or 2000 if neither applies).
/// Desync-check passes after polling mode before the detector compares
/// revolutions again: the first two interrupt-mode passes can still average
/// polled commutations (refactor_log Call 45).
const DESYNC_SETTLED_CHECKS: u8 = 3;

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
    let poles = (motor_poles as i32).max(1);
    let erpm_max = if motor_kv < 300 {
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
pub struct MainState<P: Timing, LED: OutputPin = NoLed> {
    pub protection: ProtectionState,
    pub(crate) measurements: Measurements<P>,
    pub config: EepromConfig,

    // PID controllers (main computes adjustments, ISR applies)
    pub(crate) pid: PidState,

    // Timing (main-loop side — ISR timing is in SharedComm)
    pub(crate) timing: TimingState,
    /// CPU MHz for variable PWM mode 2 scaling
    pub(crate) cpu_mhz: u8,
    pub cell_count: u8,
    pub(crate) motor_kv: u16,
    pub(crate) minimum_duty: u16,
    pub(crate) target_min_bemf_counts: u8,
    pub(crate) low_cell_volt_cutoff: u16,
    pub(crate) desync_check: bool,
    /// Lifetime desync-event count (AM32 `desync_happened`).
    pub desync_events: u32,
    /// Per-branch desync-response counters: which branch each desync
    /// fire took (fast-rotor stay-interrupt / current-veto demote /
    /// slow demote / already-polling).
    pub dsy_fast: u32,
    pub dsy_demote_cur: u32,
    /// Demote fired from interrupt mode with sane current.
    pub dsy_demote_slow: u32,
    /// Desync fired while already in polling mode.
    pub dsy_demote_old: u32,
    /// Detector inputs at the latest desync: last_average_interval,
    /// average_interval, commutation_interval, zero_crosses (bench).
    #[cfg(feature = "bench-diag")]
    pub dsy_snap: [u32; 4],
    /// The last four desync-check passes before the latest desync, oldest
    /// first: zero_crosses | polling << 31, average_interval, duty_cycle,
    /// duty_maximum (bench).
    #[cfg(feature = "bench-diag")]
    pub dsy_hist: [[u32; 4]; 4],
    #[cfg(feature = "bench-diag")]
    chk_hist: [[u32; 4]; 4],
    /// Interrupt-mode desync checks since polling ended (see the settling
    /// gate in `tick`).
    desync_checks_after_polling: u8,
    /// Polling ran since the last stop; the next interrupt-mode checks only
    /// re-seed the detector until settled.
    polling_mode_seen: bool,
    /// Wrong-phase-orbit trips (`OrbitTripConfig`) — lifetime count.
    pub orbit_trips: u32,
    /// Desync-detector re-arm threshold (zero_crosses). Normally the
    /// AM32 10; raised to the policy's `rearm_holdoff_zc` for one cycle after a
    /// fast-rotor fire so the stay-interrupt response cannot self-loop
    /// against a stale average during commanded transients.
    pub(crate) desync_rearm_zc: u32,
    /// Consecutive 1 kHz ticks with current above the orbit line.
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
    /// Position within the 200 ms LED period, in control ticks.
    pub(crate) led_phase: u32,
    /// The platform whose clocks the tick-unit constants are taken in.
    _platform: core::marker::PhantomData<P>,
    /// PWM timer max auto-reload (from PWM frequency config, overwritten by apply_motor_config)
    pub(crate) timer1_max_arr: u16,
    /// Last observed control clock; None anchors the first main service.
    last_control_tick: Option<u32>,
    // 1 kHz dispatch counter lives on SharedState (ISR increments at
    // tick rate, main reads + resets), matching AM32's
    // `one_khz_loop_counter` placement.
}

/// MCU-specific constants — properties of the silicon, not the board PCB.
#[derive(Clone, Copy, Debug)]
pub struct ChipParams {
    /// PWM timer auto-reload at default PWM frequency.
    pub timer1_max_arr: u16,
    /// CPU frequency in MHz.
    pub cpu_mhz: u8,
}

impl<P: Timing> MainState<P, NoLed> {
    /// Construct MainState from board config and chip constants.
    ///
    /// Used by both firmware and harness to ensure identical initialization.
    /// PID tuning, motor_kv, and other EEPROM-derived values are applied
    /// later via `apply_motor_config()`.
    pub fn new(board: &crate::board::BoardConfig, chip: ChipParams) -> Self {
        Self::new_with_led(board, chip, NoLed)
    }
}

impl<P: Timing, LED: OutputPin> MainState<P, LED> {
    /// Construct main-loop state with an optional custom status LED output.
    pub fn new_with_led(board: &crate::board::BoardConfig, chip: ChipParams, led: LED) -> Self {
        let () = P::VALID;
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
            #[cfg(feature = "bench-diag")]
            dsy_snap: [0; 4],
            #[cfg(feature = "bench-diag")]
            dsy_hist: [[0; 4]; 4],
            #[cfg(feature = "bench-diag")]
            chk_hist: [[0; 4]; 4],
            desync_checks_after_polling: 0,
            polling_mode_seen: false,
            orbit_trips: 0,
            desync_rearm_zc: 10,
            orbit_trip_count: 0,
            orbit_duty_snap: 0,
            orbit_duty_snap_age: 0,
            last_armed: false,
            just_armed: false,
            needs_reset: false,
            led,
            led_phase: 0,
            _platform: core::marker::PhantomData,
            last_control_tick: None,
        }
    }
    /// Read-only access to measurements.
    pub fn measurements(&self) -> &Measurements<P> {
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
    pub fn set_battery_voltage(&mut self, v: crate::units::CentiVolts) {
        self.measurements.set_battery_voltage(v);
    }

    /// Set actual_current measurement.
    pub fn set_actual_current(&mut self, v: crate::units::CentiAmps) {
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
        // One clock snapshot for all periodic work. Hold the previous observed
        // current through this interval BEFORE the ADC path replaces it below.
        // Delayed servicing retains elapsed time but cannot recover unobserved
        // current changes. Service at least once per full u32 clock wrap.
        let now = shared.control_ticks();
        let elapsed = self
            .last_control_tick
            .map_or(0, |last| now.wrapping_sub(last));
        self.last_control_tick = Some(now);
        self.measurements
            .consumed_charge
            .accumulate(self.measurements.actual_current, elapsed);
        let led_period = P::LED_BLINK_HALF_PERIOD as u32 * 2;
        if elapsed != 0 {
            let phase = self.led_phase + elapsed % led_period;
            self.led_phase = if phase >= led_period {
                phase - led_period
            } else {
                phase
            };
        }

        // e_com_time: read from SharedComm (ISR computes from per-step intervals)
        let e_com_time = shared.e_com_time();

        // Average interval
        self.timing.average_interval =
            crate::platform::e_com_us_to_avg_ticks::<P>(e_com_time) as u32;

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
        if shared.interval_timer_count() > P::BEMF_STALL_TIMER_THRESHOLD && shared.running() {
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
                shared.set_commutation_interval(P::DESYNC_RESET_INTERVAL);
            } else {
                shared.request_isr_action(IsrAction::CommutateKick);
            }
            shared.set_zero_crosses(0);
            // Active re-kick, ungated — AM32 calls zcfoundroutine() here
            // unconditionally. Also the dead-start escape: a standstill
            // window whose comparator level mismatches the expected
            // post-ZC level only advances via this timeout, so the kick
            // (interval reset + COM-timer re-arm) is AM32's implicit
            // open-loop crawl to the next window.
            let _ = was_interrupt_mode;
            shared.request_isr_action(crate::shared_comm::IsrAction::CommutateKick);
        }

        // Dynamic BEMF timeout threshold: lenient at low throttle
        if raw_input < BEMF_LENIENT_THROTTLE {
            self.protection.bemf_timeout = BEMF_TIMEOUT_LENIENT;
        } else {
            self.protection.bemf_timeout = BEMF_TIMEOUT_STRICT;
        }

        // Polling and interrupt commutation have different timing precision.
        // Let three checks sample interrupt-mode timing after polling before
        // comparing averages from the two timing paths. A stop forgets the
        // previous session's polling.
        if !shared.running() {
            self.desync_checks_after_polling = 0;
            self.polling_mode_seen = false;
        } else if shared.old_routine() {
            self.desync_checks_after_polling = 0;
            self.polling_mode_seen = true;
        }

        // Desync detection (re-arm gate is dynamic — see desync_rearm_zc)
        if self.desync_check && zc > self.desync_rearm_zc {
            self.desync_rearm_zc = 10;
            #[cfg(feature = "bench-diag")]
            {
                self.chk_hist.copy_within(1.., 0);
                self.chk_hist[3] = [
                    zc | ((shared.old_routine() as u32) << 31),
                    self.timing.average_interval,
                    shared.duty_cycle() as u32,
                    shared.duty_maximum() as u32,
                ];
            }
            let diff = get_abs_dif(
                self.timing.last_average_interval as i32,
                self.timing.average_interval as i32,
            );
            // Divergence from AM32: never compare across the polling ->
            // interrupt changeover. rm32 polls BEMF on the 20 kHz tick grid
            // (AM32: main-loop rate), so polled commutations run late; at
            // the changeover the timing snaps to correct, the next
            // revolution is much faster, and the detector read that step
            // as a desync (L431 25 % starts: ~50 % tripped at exactly the
            // first interrupt-mode check, zc 24, then restart-looped;
            // stock AM32 8/8 clean on the same bench). Each pass averages
            // the revolution since the previous pass, so the pass after
            // the changeover can still hold polled commutations: compare
            // only once both revolutions ran in interrupt mode; until then
            // a pass only re-seeds last_average_interval. Detection stays
            // active while polling, as in AM32.
            let settled = if shared.old_routine() || !self.polling_mode_seen {
                true
            } else {
                self.desync_checks_after_polling =
                    self.desync_checks_after_polling.saturating_add(1);
                if self.desync_checks_after_polling >= DESYNC_SETTLED_CHECKS {
                    self.polling_mode_seen = false;
                    true
                } else {
                    false
                }
            };
            if settled
                && diff > (self.timing.average_interval >> 1)
                && self.timing.average_interval < P::DESYNC_MAX_INTERVAL
            {
                // AM32's `average_interval = 5000` reset here is dead code
                // (placed after zero_crosses is zeroed) — match the
                // reference's behavior (no reset), not its intent;
                // "fixing" the ordering creates a desync echo that
                // double-kicks duty during recovery.
                #[cfg(feature = "bench-diag")]
                {
                    self.dsy_snap = [
                        self.timing.last_average_interval,
                        self.timing.average_interval,
                        shared.commutation_interval(),
                        zc,
                    ];
                    self.dsy_hist = self.chk_hist;
                }
                shared.set_zero_crosses(0);
                self.desync_events = self.desync_events.wrapping_add(1);
                let desync_from_interrupt_mode = !shared.old_routine();
                // Divergence from AM32: fast-rotor desyncs stay in interrupt
                // mode. AM32 demotes to polling and re-locks within ~1 ms
                // at main-loop rate; rm32 polls on the 20 kHz tick grid,
                // which cannot re-lock above ~800 Hz e, so a demoted fast
                // rotor coasts for seconds. The rotor is still locked at
                // these events — leaving the comparator armed re-locks on
                // the next real crossing. Requires sane current: an
                // elevated-current desync may be a wrong-phase orbit,
                // where the demote IS the phase reset.
                let current_sane = !orbit_current::<P::Tuning>(
                    self.measurements.actual_current.0,
                    shared.duty_cycle(),
                    self.measurements.battery_voltage.0,
                );
                let (fast_rotor, rearm_holdoff_zc) = match <P::Tuning as TuningPolicy>::DESYNC {
                    DesyncPolicy::Am32 => (false, 0),
                    DesyncPolicy::HalfKickStayInterrupt {
                        rearm_holdoff_zc, ..
                    } => (
                        current_sane && shared.commutation_interval() < P::DESYNC_STAY_INTERRUPT_CI,
                        rearm_holdoff_zc,
                    ),
                };
                // Duty kick (AM32: last_duty_cycle = min_startup/2,
                // unconditional). On the fast-rotor branch the rotor is
                // still locked, so only halve the duty —
                // a full crash costs ~15-20 ms of ramp recovery per event.
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
                    shared.request_duty_kick_half();
                } else {
                    shared.request_duty_kick_down();
                }
                // Detector holdoff (see desync_rearm_zc).
                if desync_from_interrupt_mode && fast_rotor {
                    self.desync_rearm_zc = rearm_holdoff_zc;
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
                        || shared.commutation_interval() > P::DESYNC_STOP_INTERVAL
                    {
                        shared.transition(crate::motor_mode::MotorEvent::StopMotor);
                    }
                }
            }
            self.desync_check = false;
            self.timing.last_average_interval = self.timing.average_interval;
        }

        // Signal timeout (AM32): armed 0.5 s → disarm +
        // reset; unarmed 2 s → reset. The reset sets SFTRSTF, dropping the
        // bootloader into its DFU loop (the passthrough/Configurator entry
        // path). input_set is cleared so re-detection runs; the unarmed
        // branch requires input_set so a chip that never saw a signal
        // does not reset-loop.
        let signal_timeout = shared.signal_timeout();
        if shared.armed() {
            if signal_timeout > P::SIGNAL_TIMEOUT_DISARM {
                shared.transition(crate::motor_mode::MotorEvent::Disarm);
                shared.set_input_set(false);
                self.needs_reset = true;
            }
        } else if shared.input_set() && signal_timeout > P::SIGNAL_TIMEOUT_UNARMED {
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

        // Armed-transition detection stays at tick rate so the edge isn't
        // missed by up to 1 ms; battery_voltage is populated long before
        // the first arm can happen.
        let armed = shared.armed();
        self.just_armed = armed && !self.last_armed;
        if self.just_armed && self.cell_count == 0 && self.config.low_voltage_cut_off == 1 {
            self.cell_count = (self.measurements.battery_voltage.0 / 370) as u8;
        }
        self.last_armed = armed;

        // 1 kHz dispatch: ADC + 3 PIDs + LVC (AM32's PROCESS_ADC_FLAG
        // block, , plus the PID block at ).
        // The counter increments in the tick ISR so the 1 kHz rate holds
        // regardless of main-loop iteration rate. duty_ceiling,
        // filter_level, auto_advance, min_bemf_counts and variable_pwm
        // stay outside this block at main-loop rate, matching AM32.
        if shared.one_khz_counter_check_and_reset(P::PID_LOOP_DIVIDER) {
            // ADC measurements — typed conversions via AdcCount
            let smoothed_v = AdcCount(self.measurements.voltage_filter.update(adc.raw_voltage()));
            let smoothed_c = AdcCount(self.measurements.current_filter.update(adc.raw_current()));
            self.measurements.battery_voltage = smoothed_v.centivolts::<P>();
            self.measurements.actual_current = smoothed_c.centiamps::<P>();
            self.measurements.degrees_celsius = if P::USE_NTC {
                crate::ntc::ntc_degrees(adc.raw_temperature())
            } else {
                adc.calc_temperature(adc.raw_temperature())
            };
            adc.start_conversion();

            // Publish measurements to shared state (ISR reads for EDT)
            shared.set_actual_current(self.measurements.actual_current.0);
            shared.set_battery_voltage(self.measurements.battery_voltage.0);
            shared.set_degrees_celsius(self.measurements.degrees_celsius.0);

            // Wrong-phase-orbit trip (`OrbitTripConfig`, rig tuning only):
            // sustained current far above the duty-proportional norm while
            // locked means the acceptance chain is clocking off switching
            // artifacts in a wrong phase — force the full desync response;
            // the demote IS the phase reset.
            if let Some(orbit) = <P::Tuning as TuningPolicy>::ORBIT_TRIP {
                self.orbit_duty_snap_age += 1;
                if self.orbit_duty_snap_age >= 100 {
                    self.orbit_duty_snap = shared.duty_cycle();
                    self.orbit_duty_snap_age = 0;
                }
                let transient =
                    get_abs_dif(shared.duty_cycle() as i32, self.orbit_duty_snap as i32)
                        > orbit.transient_duty as u32;
                if shared.running()
                    && shared.zero_crosses() > 1000
                    && !transient
                    && orbit_current::<P::Tuning>(
                        self.measurements.actual_current.0,
                        shared.duty_cycle(),
                        self.measurements.battery_voltage.0,
                    )
                {
                    self.orbit_trip_count += 1;
                    if self.orbit_trip_count > orbit.trip_ms {
                        self.orbit_trip_count = 0;
                        self.orbit_trips = self.orbit_trips.wrapping_add(1);
                        self.desync_events = self.desync_events.wrapping_add(1);
                        shared.set_zero_crosses(0);
                        shared.request_duty_kick_down();
                        shared.transition(crate::motor_mode::MotorEvent::DesyncFallback);
                        if shared.adjusted_input() > 47
                            || shared.commutation_interval() > P::DESYNC_STOP_INTERVAL
                        {
                            shared.transition(crate::motor_mode::MotorEvent::StopMotor);
                        }
                    }
                } else {
                    self.orbit_trip_count = 0;
                }
            }

            // Low voltage cutoff (AM32): 10 s sustained at
            // the 1 kHz rate. Mode 1: per-cell threshold; mode 2: absolute
            // threshold (0.5 V units).
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
                // Board targets and PID gains retain their established 0.5 us
                // units; changing the timer clock must not retune the controller.
                let ci = shared.commutation_interval();
                let interval = if P::TIMER_HZ == 2_000_000 {
                    ci as i32
                } else {
                    (ci as u64 * 2_000_000 / P::TIMER_HZ as u64) as i32
                };
                let boost = self.pid.tick_stall(interval);
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
            let voltage_cv = self.measurements.battery_voltage.0;
            let current_ca = self.measurements.actual_current.to_centiamps();
            telemetry::make_telem_package(
                &mut pkt,
                self.measurements.degrees_celsius.to_i8(),
                voltage_cv,
                current_ca,
                self.measurements.consumed_milliamp_hours(),
                self.timing.e_rpm, // already in units of 100 eRPM (600000/e_com_time)
            );
            telem.send(&pkt);
            shared.set_send_telemetry(false);
        }

        // Variable PWM — adjust tim1_arr based on commutation speed
        if self.config.variable_pwm == 1 {
            shared.set_tim1_arr(variable_pwm_mode1::<P>(
                shared.commutation_interval(),
                self.timer1_max_arr,
            ));
        } else if self.config.variable_pwm == 2 {
            shared.set_tim1_arr(variable_pwm_mode2::<P>(
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
        // Reclimb clamp (`ReclimbConfig`, rig tuning only): while the lock
        // is unconfirmed (fresh engage or post-fall recovery), cap the duty
        // ceiling so the reclimb toward a high commanded duty cannot surge.
        // Releases on confirmation.
        if let Some(reclimb) = <P::Tuning as TuningPolicy>::RECLIMB
            && shared.zero_crosses() < reclimb.confirm_zc
        {
            dmax = dmax.min(reclimb.duty_cap);
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
        let filter = if zc < 100 && shared.commutation_interval() > P::FILTER_SLOW_INTERVAL {
            12u8
        } else if shared.commutation_interval() < P::FILTER_FAST_INTERVAL {
            2
        } else {
            crate::functions::map(
                self.timing.average_interval as i32,
                P::FILTER_MAP_LO_INTERVAL as i32,
                P::FILTER_MAP_HI_INTERVAL as i32,
                3,
                12,
            ) as u8
        };
        shared.set_filter_level(filter);

        // Commutation advance (AM32): dynamic auto-advance
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

        // Custom LED: exact 100 ms low/high phases, continuing while off or
        // solid. Modulo phase retains missed periods without service-time drift.
        {
            let input = shared.adjusted_input();
            if (47..1947).contains(&input) {
                if self.led_phase >= P::LED_BLINK_HALF_PERIOD as u32 {
                    let _ = self.led.set_high();
                } else {
                    let _ = self.led.set_low();
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
    use crate::platform::TestPlatform;

    // --- Variable PWM mode 2 ---

    #[test]
    fn vpwm2_clamps_low() {
        // avg < 100 → floor at 100 * scale
        assert_eq!(
            variable_pwm_mode2::<TestPlatform>(50, 64),
            (100 * (64 / 9)) as u16
        );
    }

    #[test]
    fn vpwm2_clamps_high() {
        // avg >= 250 → ceiling at 250 * scale
        assert_eq!(
            variable_pwm_mode2::<TestPlatform>(300, 64),
            (250 * (64 / 9)) as u16
        );
    }

    #[test]
    fn vpwm2_scales_mid() {
        // 100 <= avg < 250 → avg * scale
        assert_eq!(
            variable_pwm_mode2::<TestPlatform>(150, 64),
            (150 * (64 / 9)) as u16
        );
    }

    #[test]
    fn vpwm2_zero_interval_clamps_high() {
        assert_eq!(
            variable_pwm_mode2::<TestPlatform>(0, 64),
            (250 * (64 / 9)) as u16
        );
    }

    // --- Variable PWM mode 1 ---

    #[test]
    fn vpwm1_fast_interval() {
        let arr = variable_pwm_mode1::<TestPlatform>(96, 1999);
        assert_eq!(arr, 999); // maps to max_arr/2
    }

    #[test]
    fn vpwm1_slow_interval() {
        let arr = variable_pwm_mode1::<TestPlatform>(200, 1999);
        assert_eq!(arr, 1999); // maps to max_arr
    }

    // --- Duty ceiling ---

    #[test]
    fn duty_ceiling_no_limits() {
        assert_eq!(duty_ceiling(0, 2000, 14, 25, 80), 2000);
    }

    #[test]
    fn duty_ceiling_matches_am32_rpm_limit_shape() {
        assert_eq!(duty_ceiling(1446, 2220, 14, 25, 141), 992);
        assert_eq!(duty_ceiling(600, 2220, 14, 25, 141), 2000);
    }

    #[test]
    fn duty_ceiling_skips_rpm_limit_for_low_kv_motors() {
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

    const ADC_FULL_SCALE: u16 = 4095;

    struct MockAdc {
        raw_voltage: u16,
        raw_current: u16,
    }
    impl MockAdc {
        fn new() -> Self {
            Self::with_raw(0, 0)
        }

        fn with_raw(raw_voltage: u16, raw_current: u16) -> Self {
            Self {
                raw_voltage,
                raw_current,
            }
        }

        fn with_raw_voltage(raw_voltage: u16) -> Self {
            Self::with_raw(raw_voltage, 0)
        }

        fn with_raw_current(raw_current: u16) -> Self {
            Self::with_raw(0, raw_current)
        }
    }
    impl crate::hal::Adc for MockAdc {
        fn start_conversion(&mut self) {}
        fn raw_voltage(&self) -> u16 {
            self.raw_voltage
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
        fn send(&mut self, _: &[u8]) {}
    }

    fn make_test_main_state() -> MainState<TestPlatform> {
        MainState::<TestPlatform>::new(
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
        let _ = MainState::<TestPlatform>::new(
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

        let shared = SharedState::new::<TestPlatform>();
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

        let shared = SharedState::new::<TestPlatform>();
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

        let shared = SharedState::new::<TestPlatform>();
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
            for _ in 0..<TestPlatform as Timing>::PID_LOOP_DIVIDER {
                shared.one_khz_counter_inc();
            }
            main.tick(&shared, &mut adc, &mut MockTelem);
        }

        assert_eq!(shared.current_limit_adjust(), motor_cfg.minimum_duty);
    }

    // --- LVC tests ---
    // REQ-PROT-LVC: Low voltage cutoff protection

    fn trip_one_khz(shared: &crate::shared_state::SharedState) {
        for _ in 0..<TestPlatform as Timing>::PID_LOOP_DIVIDER {
            shared.one_khz_counter_inc();
        }
    }

    #[test]
    fn min_bemf_counts_are_stricter_during_unidirectional_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_zero_crosses(4);

        let mut main = make_test_main_state();
        main.config.bi_direction = 0;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.min_bemf_counts(), 6);
    }

    #[test]
    fn min_bemf_counts_are_stricter_during_bidirectional_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_zero_crosses(4);

        let mut main = make_test_main_state();
        main.config.bi_direction = 1;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.min_bemf_counts(), 4);
    }

    #[test]
    fn min_bemf_counts_use_target_after_startup() {
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
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
        let shared = SharedState::new::<TestPlatform>();
        shared.set_zero_crosses(4);

        let mut main = MainState::<TestPlatform>::new(
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
        use crate::shared_comm::Commands;
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        // Threshold = 3 * 330 = 990mV. Set voltage below.
        main.set_battery_voltage(crate::units::CentiVolts(500));
        // Pre-fill count near threshold
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        // LVC now runs inside the 1 kHz block.
        for _ in 0..<TestPlatform as Timing>::PID_LOOP_DIVIDER {
            shared.one_khz_counter_inc();
        }

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // LVC should have triggered: count exceeded threshold → disarm
        assert!(
            !shared.armed(),
            "motor should be disarmed after LVC trigger"
        );
        assert_eq!(Commands::isr_action(&shared), IsrAction::AllOff);
    }

    #[test]
    fn lvc_does_not_run_before_one_khz_dispatch() {
        use crate::motor_mode::MotorMode;
        use crate::shared_comm::Commands;
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(shared.armed());
        assert_eq!(Commands::isr_action(&shared), IsrAction::None);
    }

    #[test]
    fn lvc_mode1_recovery_inhibit() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;
        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 1;
        main.cell_count = 3;
        main.low_cell_volt_cutoff = 330;
        // Trigger LVC first
        main.set_battery_voltage(crate::units::CentiVolts(500));
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        // Now raise voltage above threshold
        shared.set_motor_mode(MotorMode::OldRoutine); // re-arm for test
        main.set_battery_voltage(crate::units::CentiVolts(1500));
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
        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2; // absolute mode
        main.config.absolute_voltage_cutoff = 100; // threshold (raw EEPROM value)
        main.cell_count = 0; // no cells — doesn't matter for mode 2
        main.set_battery_voltage(crate::units::CentiVolts(50)); // below threshold
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);
        // LVC now runs inside the 1 kHz block.
        for _ in 0..<TestPlatform as Timing>::PID_LOOP_DIVIDER {
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
        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2;
        main.config.absolute_voltage_cutoff = 100;
        main.set_battery_voltage(crate::units::CentiVolts(150)); // above threshold
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
        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);
        shared.set_input_set(true);
        // Push signal timeout past armed threshold
        for _ in 0..=<TestPlatform as Timing>::SIGNAL_TIMEOUT_DISARM {
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
        let shared = SharedState::new::<TestPlatform>();
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

    #[test]
    fn lvc_mode2_above_threshold_does_not_disarm() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2;
        main.config.absolute_voltage_cutoff = 100;
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);

        trip_one_khz(&shared);
        main.tick(
            &shared,
            &mut MockAdc::with_raw_voltage(ADC_FULL_SCALE),
            &mut MockTelem,
        );

        assert!(shared.armed());
    }

    #[derive(Default)]
    struct CapturedTelemetry([u8; 10]);

    impl crate::hal::TelemetryUart for CapturedTelemetry {
        fn send(&mut self, packet: &[u8]) {
            self.0.copy_from_slice(packet);
        }
    }

    #[derive(Default)]
    struct TestLed(bool);

    impl embedded_hal::digital::ErrorType for TestLed {
        type Error = core::convert::Infallible;
    }
    impl OutputPin for TestLed {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.0 = false;
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.0 = true;
            Ok(())
        }
    }

    #[test]
    fn accounting_regression_main_calls_without_time_do_not_consume_charge() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        let mut main = make_test_main_state();
        main.set_actual_current(crate::units::CentiAmps(1000)); // 10 A
        let mut telem = CapturedTelemetry::default();
        for _ in 0..20001 {
            main.tick(&shared, &mut MockAdc::new(), &mut telem);
        }
        shared.set_send_telemetry(true);
        main.tick(&shared, &mut MockAdc::new(), &mut telem);
        assert_eq!(u16::from_be_bytes([telem.0[5], telem.0[6]]), 0);
    }

    #[test]
    fn accounting_regression_main_calls_without_time_do_not_blink() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_adjusted_input(100);
        let mut main = MainState::<TestPlatform, _>::new_with_led(
            &crate::board::BoardConfig::DEFAULT,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
            TestLed::default(),
        );
        for _ in 0..2001 {
            main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        }
        assert!(
            !main.led.0,
            "LED phase must not advance without control ticks"
        );
    }

    fn advance_control_clock(shared: &SharedState, ticks: u32) {
        for _ in 0..ticks {
            shared.increment_control_ticks();
        }
    }

    fn send_charge(main: &mut MainState<TestPlatform>, shared: &SharedState) -> u16 {
        let mut telem = CapturedTelemetry::default();
        shared.set_send_telemetry(true);
        main.tick(shared, &mut MockAdc::new(), &mut telem);
        u16::from_be_bytes([telem.0[5], telem.0[6]])
    }

    #[test]
    fn charge_first_service_anchors_without_backfilling_startup_time() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        advance_control_clock(&shared, 72000);
        let mut main = make_test_main_state();
        main.set_actual_current(crate::units::CentiAmps(100));
        assert_eq!(send_charge(&mut main, &shared), 0);
        advance_control_clock(&shared, 72000);
        assert_eq!(send_charge(&mut main, &shared), 1); // 1 A * 3.6 s
        assert_eq!(send_charge(&mut main, &shared), 1);
    }

    #[test]
    fn charge_and_kiss_are_independent_of_main_service_cadence() {
        for interval in [1, 20, 2000, 72000] {
            let shared = SharedState::new::<crate::platform::TestPlatform>();
            let mut main = make_test_main_state();
            main.set_actual_current(crate::units::CentiAmps(100));
            main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
            for _ in 0..72000 / interval {
                advance_control_clock(&shared, interval);
                main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
            }
            assert_eq!(send_charge(&mut main, &shared), 1, "interval={interval}");
        }
    }

    #[test]
    fn charge_holds_previous_sample_until_adc_observation() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        let mut main = make_test_main_state();
        main.set_actual_current(crate::units::CentiAmps(100)); // 1 A held
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        // Make the next ADC observation full scale after current filtering:
        // 163.80 A by AM32's `raw * 80` (devel keeps AM32's arithmetic; main's
        // exact 220/273 form gives 165 A -- rm32-private#10).
        for _ in 0..50 {
            main.measurements.current_filter.update(4095);
        }
        advance_control_clock(&shared, 72000); // 3.6 s at the prior 1 A
        trip_one_khz(&shared);
        shared.set_send_telemetry(true);
        let mut telem = CapturedTelemetry::default();
        main.tick(&shared, &mut MockAdc::with_raw_current(4095), &mut telem);
        assert_eq!(u16::from_be_bytes([telem.0[5], telem.0[6]]), 1);
        assert_eq!(main.measurements.actual_current.0, 16380);
        advance_control_clock(&shared, 72000); // next 3.6 s at 163.80 A
        assert_eq!(send_charge(&mut main, &shared), 164);
    }

    #[test]
    fn charge_wrap_negative_current_and_wire_saturation() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        let mut main = make_test_main_state();
        // 72000 elapsed ticks across a wrap: 1 A * 3.6 s = 1 mAh.
        main.last_control_tick = Some(0u32.wrapping_sub(72000));
        main.set_actual_current(crate::units::CentiAmps(100));
        assert_eq!(send_charge(&mut main, &shared), 1);
        main.set_actual_current(crate::units::CentiAmps(-100));
        advance_control_clock(&shared, 72000);
        assert_eq!(send_charge(&mut main, &shared), 1);
        main.set_actual_current(crate::units::CentiAmps(i16::MAX));
        main.last_control_tick = Some(shared.control_ticks().wrapping_add(1));
        assert_eq!(send_charge(&mut main, &shared), u16::MAX); // largest valid gap
        advance_control_clock(&shared, 20);
        assert_eq!(send_charge(&mut main, &shared), u16::MAX);
    }

    fn make_led_main() -> MainState<TestPlatform, TestLed> {
        MainState::<TestPlatform, _>::new_with_led(
            &crate::board::BoardConfig::DEFAULT,
            ChipParams {
                timer1_max_arr: 1999,
                cpu_mhz: 64,
            },
            TestLed::default(),
        )
    }

    #[test]
    fn led_exact_phase_boundaries_and_delayed_service() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_adjusted_input(100);
        let mut main = make_led_main();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        let mut previous = 0;
        for (ticks, high) in [
            (1999, false),
            (2000, true),
            (3999, true),
            (4000, false),
            (402001, true),
            (404000, false),
        ] {
            advance_control_clock(&shared, ticks - previous);
            main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
            assert_eq!(main.led.0, high, "tick={ticks}");
            previous = ticks;
        }
    }

    #[test]
    fn led_wrap_and_throttle_gates_preserve_continuous_phase() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        let mut main = make_led_main();
        main.led_phase = 1999;
        main.last_control_tick = Some(u32::MAX - 1999);
        shared.set_adjusted_input(100);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(main.led.0); // 3999 ticks into the period after clock wrap
        shared.increment_control_ticks();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(!main.led.0); // exact period boundary
        // Phase advances even while throttle selects solid/off output.
        shared.set_adjusted_input(2000);
        advance_control_clock(&shared, 2000);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        for (input, high) in [
            (0, false),
            (46, false),
            (47, true),
            (1946, true),
            (1947, false),
            (1948, true),
        ] {
            shared.set_adjusted_input(input);
            main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
            assert_eq!(main.led.0, high, "input={input}");
        }
        shared.set_adjusted_input(0);
        advance_control_clock(&shared, 6000);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        shared.set_adjusted_input(100);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(!main.led.0);
    }

    #[test]
    fn lvc_latch_inhibits_counter_recovery() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);

        let mut main = make_test_main_state();
        main.config.low_voltage_cut_off = 2;
        main.config.absolute_voltage_cutoff = 100;
        main.protection.set_low_voltage_count(LVC_NORMAL_THRESHOLD);

        trip_one_khz(&shared);
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        assert!(!shared.armed());
        let latched_count = main.protection.low_voltage_count();
        assert!(latched_count > LVC_NORMAL_THRESHOLD);

        shared.set_motor_mode(MotorMode::OldRoutine);
        trip_one_khz(&shared);
        main.tick(
            &shared,
            &mut MockAdc::with_raw_voltage(ADC_FULL_SCALE),
            &mut MockTelem,
        );

        assert_eq!(main.protection.low_voltage_count(), latched_count);
        assert!(!shared.armed());
    }

    /// REQ-RESET-ON-TIMEOUT: When signal_timeout fires (armed >0.5s or
    /// unarmed >2s), MainState::tick must request a system reset via
    /// `needs_reset`. Matches AM32's two NVIC_SystemReset() calls in the
    /// signal-timeout block (Src/main.c:2388, :2401) — the only way the AM32
    /// bootloader DFU loop ever activates from a running firmware, which is
    /// what BF's passthrough mode and the AM32 Configurator depend on.
    #[test]
    fn signal_timeout_armed_requests_reset() {
        use crate::motor_mode::MotorMode;
        use crate::shared_state::SharedState;

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::OldRoutine);
        shared.set_input_set(true);
        let mut main = make_test_main_state();
        assert!(!main.needs_reset, "starts not requesting reset");
        for _ in 0..=<TestPlatform as Timing>::SIGNAL_TIMEOUT_DISARM {
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
        let shared = SharedState::new::<TestPlatform>();
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
    // The changeover from sine mode to sensorless BLDC is split between
    // SystemTick::apply_sine_changeover (commutation_interval, zero_crosses,
    // prop_brake_active, average/last_average_interval, changeover step) and
    // the tick ISR (step, com_step, PWM update event, COM timer, comparator
    // arming), not MainState::tick(). These tests document what the
    // changeover MUST set, based on AM32 Src/main.c:2731-2746.
    //
    // Still missing against AM32:
    //   - SET_INTERVAL_TIMER_COUNT(9000)
    //   - last_duty_cycle = stall_protect_minimum_duty if stall_protection ON
    // (AM32's commutate() also advances the step before comStep; rm32
    // applies changeover_step directly — not compared here.)

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

        let shared = SharedState::new::<TestPlatform>();
        shared.set_motor_mode(MotorMode::Disarmed);
        assert!(!shared.input_set());
        for _ in 0..=<TestPlatform as Timing>::SIGNAL_TIMEOUT_UNARMED {
            shared.increment_signal_timeout();
        }

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(!main.needs_reset);
    }

    #[test]
    fn polling_to_interrupt_timing_step_is_not_a_desync() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::OldRoutine);
        shared.set_adjusted_input(0);
        shared.set_commutation_interval(100);
        shared.set_e_com_time(1200);

        let mut main = make_test_main_state();
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        shared.set_motor_mode(crate::motor_mode::MotorMode::Running);
        shared.set_zero_crosses(20);
        main.desync_check = true;
        main.timing.last_average_interval = 1000;
        main.timing.average_interval = 400;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert!(
            !shared.old_routine(),
            "mode-change timing must only reseed the detector"
        );
    }

    #[test]
    fn desync_detection_remains_active_while_polling() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::OldRoutine);
        shared.set_adjusted_input(0);
        shared.set_zero_crosses(20);
        shared.set_commutation_interval(100);
        shared.set_e_com_time(1500);

        let mut main = make_test_main_state();
        main.config.bi_direction = 1;
        main.desync_check = true;
        main.timing.last_average_interval = 1000;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.zero_crosses(), 0);
        assert!(shared.take_duty_kick_down());
    }

    #[test]
    fn stable_interrupt_timing_change_triggers_recovery() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::OldRoutine);
        shared.set_adjusted_input(0);
        // Above DESYNC_STAY_INTERRUPT_CI: the rig tuning's fast-rotor
        // half-kick branch must not apply; below DESYNC_STOP_INTERVAL.
        shared.set_commutation_interval(700);
        shared.set_e_com_time(1200);

        let mut main = make_test_main_state();
        main.config.bi_direction = 1;
        main.timing.last_average_interval = 400;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        shared.set_motor_mode(crate::motor_mode::MotorMode::Running);
        shared.set_zero_crosses(20);
        for (check, average) in [1000, 1000, 400].into_iter().enumerate() {
            shared.set_e_com_time((average * 3) as i32);
            main.desync_check = true;
            main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
            if (check as u8) + 1 < DESYNC_SETTLED_CHECKS {
                assert_eq!(shared.motor_mode(), crate::motor_mode::MotorMode::Running);
            }
        }

        assert_eq!(
            shared.motor_mode(),
            crate::motor_mode::MotorMode::OldRoutine
        );
        assert!(shared.take_duty_kick_down());
        shared.transition(crate::motor_mode::MotorEvent::BemfLocked);
        assert_eq!(shared.motor_mode(), crate::motor_mode::MotorMode::Running);
    }

    #[test]
    fn stopped_motor_restarts_desync_settling_window() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::Armed);
        let mut main = make_test_main_state();
        main.desync_checks_after_polling = DESYNC_SETTLED_CHECKS;
        main.polling_mode_seen = true;

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(main.desync_checks_after_polling, 0);
        assert!(!main.polling_mode_seen);
    }

    fn run_desync_case(
        zc: u32,
        previous: u32,
        average: u32,
    ) -> (crate::motor_mode::MotorMode, bool) {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::Running);
        shared.set_adjusted_input(0);
        shared.set_zero_crosses(zc);
        // Above DESYNC_STAY_INTERRUPT_CI: the rig tuning's fast-rotor
        // half-kick branch must not apply; below DESYNC_STOP_INTERVAL.
        shared.set_commutation_interval(700);
        shared.set_e_com_time((average * 3) as i32);
        let mut main = make_test_main_state();
        main.config.bi_direction = 1;
        main.desync_checks_after_polling = DESYNC_SETTLED_CHECKS;
        main.desync_check = true;
        main.timing.last_average_interval = previous;
        main.timing.average_interval = average;
        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);
        (shared.motor_mode(), shared.take_duty_kick_down())
    }

    #[test]
    fn desync_detection_obeys_zero_cross_and_interval_boundaries() {
        use crate::motor_mode::MotorMode;
        let platform_max = <crate::platform::TestPlatform as Timing>::DESYNC_MAX_INTERVAL;

        assert_eq!(run_desync_case(10, 1000, 500), (MotorMode::Running, false));
        assert_eq!(run_desync_case(11, 750, 500), (MotorMode::Running, false));
        assert_eq!(
            run_desync_case(11, 0, platform_max),
            (MotorMode::Running, false)
        );
        assert_eq!(run_desync_case(11, 751, 500), (MotorMode::OldRoutine, true));
    }

    #[test]
    fn bemf_timeout_requests_commutation_restart_while_running() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();
        shared.set_motor_mode(crate::motor_mode::MotorMode::Running);
        shared.set_adjusted_input(100);
        shared.set_interval_timer_count(
            <crate::platform::TestPlatform as Timing>::BEMF_STALL_TIMER_THRESHOLD + 1,
        );
        let mut main = make_test_main_state();

        main.tick(&shared, &mut MockAdc::new(), &mut MockTelem);

        assert_eq!(shared.isr_action(), IsrAction::CommutateKick);
        assert_eq!(shared.zero_crosses(), 0);
        assert!(shared.old_routine());
    }
}
