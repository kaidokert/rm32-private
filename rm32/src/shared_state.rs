//! Shared state between ISR and main loop — all atomic, lock-free.
//!
//! Uses `portable-atomic` for cross-architecture support:
//! - On Cortex-M4+: hardware LDREX/STREX for lock-free CAS
//! - On Cortex-M0+ (and other cores without CAS): the port's portable-atomic backend
//!
//! Acquire/Release ordering for cross-context data passing.

use crate::motor_mode::MotorMode;
use portable_atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};

/// Store ordering — ensures writes are visible to other contexts.
const REL: Ordering = Ordering::Release;
/// Load ordering — ensures we see all prior writes from other contexts.
const ACQ: Ordering = Ordering::Acquire;
const CONFIG_WRITE_QUEUE_CAPACITY: u8 = 8;
const CONFIG_WRITE_QUEUE_LEN: u8 = CONFIG_WRITE_QUEUE_CAPACITY + 1;

/// Shared state accessed by both ISR and main loop contexts.
/// All fields are atomic — no locks or critical sections needed.
#[repr(C)]
pub struct SharedState {
    // Motor state machine (single atomic replaces armed/running/old_routine/stepper_sine)
    motor_mode: AtomicU8,
    input_set: AtomicBool,
    send_telemetry: AtomicBool,
    dshot: AtomicBool,
    servo_pwm: AtomicBool,
    dshot_telemetry: AtomicBool,
    save_settings_flag: AtomicBool,
    send_esc_info_flag: AtomicBool,
    pending_servo_calibration: AtomicBool,
    pending_servo_low_threshold: AtomicU8,
    pending_servo_high_threshold: AtomicU8,
    config_write_queue: [AtomicU16; CONFIG_WRITE_QUEUE_LEN as usize],
    config_write_head: AtomicU8,
    config_write_tail: AtomicU8,

    // Timing (ISR writes, main reads)
    zero_crosses: AtomicU32,
    commutation_interval: AtomicU32,

    // Input (DMA ISR writes, main/tenKhz reads)
    newinput: AtomicU16,
    adjusted_input: AtomicU16,

    // Control (main writes setpoint, ISR reads)
    duty_cycle_setpoint: AtomicU16,
    duty_cycle: AtomicU16, // ISR writes, main reads (bidir speed gate)
    forward: AtomicBool,   // direction: ISR reads, main writes on bidir change
    signal_timeout: AtomicU32,
    zero_input_count: AtomicU16,

    // Telemetry (main computes, ISR reads for speed PID)
    e_com_time: AtomicU32, // stored as u32, interpreted as i32
    // core (not portable_atomic): plain load/store exist on every target,
    // and portable_atomic wraps even a relaxed M0 load in PRIMASK -- a
    // critical section on the priority-0 COMP path.
    gate_avg: core::sync::atomic::AtomicU32,

    // Stall protection (main computes, ISR applies to duty)
    stall_protection_adjust: AtomicU16,
    current_limit_adjust: AtomicU16, // main PID publishes, ISR clamps duty

    // Measurements (main writes, ISR reads for EDT)
    actual_current: AtomicU16,  // mA, stored as u16
    battery_voltage: AtomicU16, // cV (10 mV)
    degrees_celsius: AtomicU16, // stored as u16, interpreted as i16

    // ISR→main: interval timer count for stall detection
    interval_timer_count: AtomicU32,

    // Main→ISR published control (main computes, ISR applies)
    tim1_arr: AtomicU16,              // variable PWM auto-reload
    duty_maximum: AtomicU16,          // eRPM/temperature throttle restriction
    filter_level: AtomicU8,           // BEMF comparator filter samples
    min_bemf_counts: AtomicU8,        // min zero-cross detection threshold
    auto_advance: AtomicU8,           // commutation timing advance level
    prop_brake_active: AtomicBool,    // proportional brake engaged (main sets, ISR reads)
    isr_action: AtomicU8,             // main→ISR action request (IsrAction enum)
    changeover_step: AtomicU8,        // sine changeover step (0=none, 1-6=pending)
    desync_check_pending: AtomicBool, // ISR sets on BEMF zero-cross, main reads+clears
    // Control-tick count (one per tick ISR). A clock, not a diagnostic:
    // ports without a cycle counter derive bench time from it.
    control_ticks: AtomicU32,
    // Tone channel: pending tone id (rm32::tone), 0 = none. Set by the
    // DSHOT-command ISR (beacons) or main (arming tune); consumed by
    // the tick's tone stepper.
    tone_request: AtomicU8,
    // 1 kHz dispatch counter — incremented by the tick ISR, read +
    // reset by main past PID_LOOP_DIVIDER (AM32's one_khz_loop_counter
    // placement).
    one_khz_counter: AtomicU8,
    telem_counter: AtomicU16,
}

impl SharedState {
    /// Boot state; the initial commutation interval is taken in `P`'s timer.
    pub const fn new<P: crate::constants::Timing>() -> Self {
        Self {
            motor_mode: AtomicU8::new(MotorMode::Disarmed as u8),
            input_set: AtomicBool::new(false),
            send_telemetry: AtomicBool::new(false),
            dshot: AtomicBool::new(false),
            servo_pwm: AtomicBool::new(false),
            dshot_telemetry: AtomicBool::new(false),
            save_settings_flag: AtomicBool::new(false),
            send_esc_info_flag: AtomicBool::new(false),
            pending_servo_calibration: AtomicBool::new(false),
            pending_servo_low_threshold: AtomicU8::new(0),
            pending_servo_high_threshold: AtomicU8::new(0),
            config_write_queue: [const { AtomicU16::new(0) }; CONFIG_WRITE_QUEUE_LEN as usize],
            config_write_head: AtomicU8::new(0),
            config_write_tail: AtomicU8::new(0),
            zero_crosses: AtomicU32::new(0),
            commutation_interval: AtomicU32::new(P::INITIAL_COMMUTATION_INTERVAL),
            newinput: AtomicU16::new(0),
            adjusted_input: AtomicU16::new(0),
            duty_cycle_setpoint: AtomicU16::new(0),
            duty_cycle: AtomicU16::new(0),
            forward: AtomicBool::new(true),
            signal_timeout: AtomicU32::new(0),
            zero_input_count: AtomicU16::new(0),
            e_com_time: AtomicU32::new(0),
            gate_avg: core::sync::atomic::AtomicU32::new(0),
            interval_timer_count: AtomicU32::new(0),
            stall_protection_adjust: AtomicU16::new(0),
            current_limit_adjust: AtomicU16::new(2000),
            actual_current: AtomicU16::new(0),
            battery_voltage: AtomicU16::new(0),
            degrees_celsius: AtomicU16::new(0),
            tim1_arr: AtomicU16::new(P::PWM_BASE_PERIOD),
            duty_maximum: AtomicU16::new(2000),
            filter_level: AtomicU8::new(5),
            min_bemf_counts: AtomicU8::new(2),
            auto_advance: AtomicU8::new(0),
            prop_brake_active: AtomicBool::new(false),
            isr_action: AtomicU8::new(0), // IsrAction::None
            changeover_step: AtomicU8::new(0),
            desync_check_pending: AtomicBool::new(false),
            control_ticks: AtomicU32::new(0),
            tone_request: AtomicU8::new(0),
            one_khz_counter: AtomicU8::new(0),
            telem_counter: AtomicU16::new(0),
        }
    }

    // --- Bench debug counters (bidir DSHOT investigation) ---
    /// Increment 1 kHz dispatch counter (control-tick ISR side, 20 kHz).
    pub fn one_khz_counter_inc(&self) {
        self.one_khz_counter
            .fetch_update(REL, ACQ, |cur| Some(cur.saturating_add(1)))
            .ok();
    }
    /// Read the 1 kHz dispatch counter and reset to 0 if it has reached
    /// `divider`. Returns true if the 1 kHz block should fire this iter.
    /// Main-loop side.
    pub fn one_khz_counter_check_and_reset(&self, divider: u8) -> bool {
        self.one_khz_counter
            .fetch_update(REL, ACQ, |count| (count >= divider).then_some(0))
            .is_ok()
    }
    pub fn telem_counter_check_and_inc(&self, limit: u16) -> bool {
        self.telem_counter
            .fetch_update(REL, ACQ, |count| {
                let next = count.saturating_add(1);
                if next >= limit { Some(0) } else { Some(next) }
            })
            .map(|count| count.saturating_add(1) >= limit)
            .unwrap_or(false)
    }

    // --- Motor mode ---

    pub fn motor_mode(&self) -> MotorMode {
        MotorMode::from_u8(self.motor_mode.load(ACQ))
    }
    pub fn set_motor_mode(&self, mode: MotorMode) {
        self.motor_mode.store(mode as u8, REL);
    }

    /// Atomic state transition via CAS.
    /// On M4: hardware LDREX/STREX. On M0: portable-atomic disables interrupts.
    pub fn transition(&self, event: crate::motor_mode::MotorEvent) {
        self.motor_mode
            .fetch_update(REL, ACQ, |cur| {
                let mode = MotorMode::from_u8(cur);
                let new = mode.transition(event);
                if new != mode { Some(new as u8) } else { None }
            })
            .ok();
    }

    // Convenience getters (delegate to motor_mode)
    pub fn armed(&self) -> bool {
        self.motor_mode().is_armed()
    }
    pub fn running(&self) -> bool {
        self.motor_mode().is_running()
    }
    pub fn old_routine(&self) -> bool {
        self.motor_mode().is_old_routine()
    }
    pub fn stepper_sine(&self) -> bool {
        self.motor_mode().is_stepper_sine()
    }

    // Convenience setters — atomic CAS via portable-atomic.
    // Hardware LDREX/STREX on M4, interrupt-free fallback on M0.
    pub fn set_armed(&self, v: bool) {
        self.motor_mode
            .fetch_update(REL, ACQ, |cur| {
                let mode = MotorMode::from_u8(cur);
                if v && !mode.is_armed() {
                    Some(MotorMode::Armed as u8)
                } else if !v {
                    Some(MotorMode::Disarmed as u8)
                } else {
                    None
                }
            })
            .ok();
    }
    pub fn set_running(&self, v: bool) {
        self.motor_mode
            .fetch_update(REL, ACQ, |cur| {
                let mode = MotorMode::from_u8(cur);
                if v && !mode.is_running() {
                    Some(MotorMode::OldRoutine as u8)
                } else if !v && mode.is_running() {
                    Some(MotorMode::Armed as u8)
                } else {
                    None
                }
            })
            .ok();
    }
    pub fn set_old_routine(&self, v: bool) {
        self.motor_mode
            .fetch_update(REL, ACQ, |cur| {
                let mode = MotorMode::from_u8(cur);
                if v && mode.is_running() {
                    Some(MotorMode::OldRoutine as u8)
                } else if !v && mode.is_old_routine() {
                    Some(MotorMode::Running as u8)
                } else {
                    None
                }
            })
            .ok();
    }
    pub fn set_stepper_sine(&self, v: bool) {
        self.motor_mode
            .fetch_update(REL, ACQ, |cur| {
                let mode = MotorMode::from_u8(cur);
                if v {
                    Some(MotorMode::StepperSine as u8)
                } else if mode.is_stepper_sine() {
                    Some(MotorMode::Armed as u8)
                } else {
                    None
                }
            })
            .ok();
    }

    // --- Bool accessors ---

    pub fn input_set(&self) -> bool {
        self.input_set.load(ACQ)
    }
    pub fn set_input_set(&self, v: bool) {
        self.input_set.store(v, REL);
    }

    pub fn send_telemetry(&self) -> bool {
        self.send_telemetry.load(ACQ)
    }
    pub fn set_send_telemetry(&self, v: bool) {
        self.send_telemetry.store(v, REL);
    }

    pub fn dshot(&self) -> bool {
        self.dshot.load(ACQ)
    }
    pub fn set_dshot(&self, v: bool) {
        self.dshot.store(v, REL);
    }

    pub fn servo_pwm(&self) -> bool {
        self.servo_pwm.load(ACQ)
    }
    pub fn set_servo_pwm(&self, v: bool) {
        self.servo_pwm.store(v, REL);
    }

    pub fn dshot_telemetry(&self) -> bool {
        self.dshot_telemetry.load(ACQ)
    }
    pub fn set_dshot_telemetry(&self, v: bool) {
        self.dshot_telemetry.store(v, REL);
    }

    pub fn save_settings_flag(&self) -> bool {
        self.save_settings_flag.load(ACQ)
    }
    pub fn set_save_settings_flag(&self, v: bool) {
        self.save_settings_flag.store(v, REL);
    }

    pub fn publish_servo_calibration(&self, low_threshold: u8, high_threshold: u8) {
        self.pending_servo_low_threshold.store(low_threshold, REL);
        self.pending_servo_high_threshold.store(high_threshold, REL);
        self.pending_servo_calibration.store(true, REL);
    }

    pub fn take_servo_calibration(&self) -> Option<(u8, u8)> {
        if self.pending_servo_calibration.swap(false, ACQ) {
            Some((
                self.pending_servo_low_threshold.load(ACQ),
                self.pending_servo_high_threshold.load(ACQ),
            ))
        } else {
            None
        }
    }

    pub fn push_config_write(&self, offset: u8, value: u8) -> bool {
        let head = self.config_write_head.load(ACQ);
        let next = (head + 1) % CONFIG_WRITE_QUEUE_LEN;
        if next != self.config_write_tail.load(ACQ) {
            self.config_write_queue[head as usize]
                .store(((offset as u16) << 8) | value as u16, REL);
            self.config_write_head.store(next, REL);
            true
        } else {
            false
        }
    }

    pub fn pop_config_write(&self) -> Option<(u8, u8)> {
        let tail = self.config_write_tail.load(ACQ);
        if tail == self.config_write_head.load(ACQ) {
            return None;
        }
        let packed = self.config_write_queue[tail as usize].load(ACQ);
        self.config_write_tail
            .store((tail + 1) % CONFIG_WRITE_QUEUE_LEN, REL);
        Some(((packed >> 8) as u8, packed as u8))
    }

    pub fn send_esc_info_flag(&self) -> bool {
        self.send_esc_info_flag.load(ACQ)
    }
    pub fn set_send_esc_info_flag(&self, v: bool) {
        self.send_esc_info_flag.store(v, REL);
    }

    // --- U32 accessors ---

    pub fn zero_crosses(&self) -> u32 {
        self.zero_crosses.load(ACQ)
    }
    pub fn set_zero_crosses(&self, v: u32) {
        self.zero_crosses.store(v, REL);
    }
    /// Increment zero_crosses, capped at 10000 (matches C behavior).
    pub fn increment_zero_crosses(&self) {
        self.zero_crosses
            .fetch_update(REL, ACQ, |v| if v < 10000 { Some(v + 1) } else { None })
            .ok();
    }

    pub fn commutation_interval(&self) -> u32 {
        self.commutation_interval.load(ACQ)
    }
    pub fn set_commutation_interval(&self, v: u32) {
        self.commutation_interval.store(v, REL);
    }

    pub fn e_com_time(&self) -> i32 {
        self.e_com_time.load(ACQ) as i32
    }
    pub fn set_e_com_time(&self, v: i32) {
        self.e_com_time.store(v as u32, REL);
    }

    /// See `IsrTiming::gate_avg`. Relaxed: one word, no ordering with
    /// other fields.
    pub fn gate_avg(&self) -> u32 {
        self.gate_avg.load(core::sync::atomic::Ordering::Relaxed)
    }
    pub fn set_gate_avg(&self, v: u32) {
        self.gate_avg
            .store(v, core::sync::atomic::Ordering::Relaxed);
    }

    // --- U16 accessors ---

    pub fn newinput(&self) -> u16 {
        self.newinput.load(ACQ)
    }
    pub fn set_newinput(&self, v: u16) {
        self.newinput.store(v, REL);
    }

    pub fn adjusted_input(&self) -> u16 {
        self.adjusted_input.load(ACQ)
    }
    pub fn set_adjusted_input(&self, v: u16) {
        self.adjusted_input.store(v, REL);
    }

    pub fn duty_cycle_setpoint(&self) -> u16 {
        self.duty_cycle_setpoint.load(ACQ)
    }
    pub fn set_duty_cycle_setpoint(&self, v: u16) {
        self.duty_cycle_setpoint.store(v, REL);
    }

    pub fn duty_cycle(&self) -> u16 {
        self.duty_cycle.load(ACQ)
    }
    pub fn set_duty_cycle(&self, v: u16) {
        self.duty_cycle.store(v, REL);
    }

    pub fn forward(&self) -> bool {
        self.forward.load(ACQ)
    }
    pub fn set_forward(&self, v: bool) {
        self.forward.store(v, REL);
    }

    pub fn signal_timeout(&self) -> u32 {
        self.signal_timeout.load(ACQ)
    }
    pub fn set_signal_timeout(&self, v: u32) {
        self.signal_timeout.store(v, REL);
    }
    pub fn increment_signal_timeout(&self) {
        self.signal_timeout
            .fetch_update(REL, ACQ, |v| if v < u32::MAX { Some(v + 1) } else { None })
            .ok();
    }

    pub fn zero_input_count(&self) -> u16 {
        self.zero_input_count.load(ACQ)
    }
    pub fn set_zero_input_count(&self, v: u16) {
        self.zero_input_count.store(v, REL);
    }

    // --- Measurement accessors (main writes, ISR reads for EDT) ---

    pub fn stall_protection_adjust(&self) -> u16 {
        self.stall_protection_adjust.load(ACQ)
    }
    pub fn set_stall_protection_adjust(&self, v: u16) {
        self.stall_protection_adjust.store(v, REL);
    }

    pub fn current_limit_adjust(&self) -> u16 {
        self.current_limit_adjust.load(ACQ)
    }
    pub fn set_current_limit_adjust(&self, v: u16) {
        self.current_limit_adjust.store(v, REL);
    }

    pub fn actual_current(&self) -> i16 {
        self.actual_current.load(ACQ) as i16
    }
    pub fn set_actual_current(&self, v: i16) {
        self.actual_current.store(v as u16, REL);
    }

    pub fn battery_voltage(&self) -> u16 {
        self.battery_voltage.load(ACQ)
    }
    pub fn set_battery_voltage(&self, v: u16) {
        self.battery_voltage.store(v, REL);
    }

    pub fn degrees_celsius(&self) -> i16 {
        self.degrees_celsius.load(ACQ) as i16
    }
    pub fn set_degrees_celsius(&self, v: i16) {
        self.degrees_celsius.store(v as u16, REL);
    }

    pub fn interval_timer_count(&self) -> u32 {
        self.interval_timer_count.load(ACQ)
    }
    pub fn set_interval_timer_count(&self, v: u32) {
        self.interval_timer_count.store(v, REL);
    }

    // --- Main→ISR published control ---

    pub fn tim1_arr(&self) -> u16 {
        self.tim1_arr.load(ACQ)
    }
    pub fn set_tim1_arr(&self, v: u16) {
        self.tim1_arr.store(v, REL);
    }

    pub fn duty_maximum(&self) -> u16 {
        self.duty_maximum.load(ACQ)
    }
    pub fn set_duty_maximum(&self, v: u16) {
        self.duty_maximum.store(v, REL);
    }

    pub fn filter_level(&self) -> u8 {
        self.filter_level.load(ACQ)
    }
    pub fn set_filter_level(&self, v: u8) {
        self.filter_level.store(v, REL);
    }

    pub fn min_bemf_counts(&self) -> u8 {
        self.min_bemf_counts.load(ACQ)
    }
    pub fn set_min_bemf_counts(&self, v: u8) {
        self.min_bemf_counts.store(v, REL);
    }

    pub fn auto_advance(&self) -> u8 {
        self.auto_advance.load(ACQ)
    }
    pub fn set_auto_advance(&self, v: u8) {
        self.auto_advance.store(v, REL);
    }

    pub fn prop_brake_active(&self) -> bool {
        self.prop_brake_active.load(ACQ)
    }
    pub fn set_prop_brake_active(&self, v: bool) {
        self.prop_brake_active.store(v, REL);
    }
    pub fn isr_action(&self) -> crate::shared_comm::IsrAction {
        crate::shared_comm::IsrAction::from_u8(self.isr_action.load(ACQ))
    }
    pub fn request_isr_action(&self, action: crate::shared_comm::IsrAction) {
        // Only upgrade priority — don't downgrade AllOff to ResetIntervalTimer
        let new = action as u8;
        let _ = self.isr_action.fetch_max(new, REL);
    }
    /// Request a tone (rm32::tone ids); last writer wins.
    pub fn set_tone_request(&self, id: u8) {
        self.tone_request.store(id, REL);
    }
    /// Tone stepper side: consume the pending request (0 = none).
    pub fn take_tone_request(&self) -> u8 {
        let v = self.tone_request.load(ACQ);
        if v != 0 {
            self.tone_request.store(0, REL);
        }
        v
    }

    pub fn clear_isr_action(&self, action: crate::shared_comm::IsrAction) {
        self.isr_action
            .compare_exchange(action as u8, 0, REL, ACQ)
            .ok();
    }
    pub fn changeover_step(&self) -> u8 {
        self.changeover_step.load(ACQ)
    }
    pub fn set_changeover_step(&self, step: u8) {
        self.changeover_step.store(step, REL);
    }
    pub fn desync_check_pending(&self) -> bool {
        self.desync_check_pending.load(ACQ)
    }
    pub fn set_desync_check_pending(&self, v: bool) {
        self.desync_check_pending.store(v, REL);
    }
    pub fn take_desync_check_pending(&self) -> bool {
        self.desync_check_pending.swap(false, Ordering::AcqRel)
    }
    pub fn control_ticks(&self) -> u32 {
        self.control_ticks.load(ACQ)
    }
    pub fn increment_control_ticks(&self) {
        self.control_ticks.fetch_add(1, REL);
    }
}

impl crate::shared_comm::MotorStatus for SharedState {
    fn motor_mode(&self) -> MotorMode {
        self.motor_mode()
    }
    fn set_motor_mode(&self, mode: MotorMode) {
        self.set_motor_mode(mode);
    }
    fn transition(&self, event: crate::motor_mode::MotorEvent) {
        SharedState::transition(self, event);
    }
    // Override convenience setters to use atomic CAS inherent methods
    // instead of the non-atomic trait defaults (load + store).
    fn set_armed(&self, v: bool) {
        SharedState::set_armed(self, v);
    }
    fn set_running(&self, v: bool) {
        SharedState::set_running(self, v);
    }
    fn set_old_routine(&self, v: bool) {
        SharedState::set_old_routine(self, v);
    }
    fn set_stepper_sine(&self, v: bool) {
        SharedState::set_stepper_sine(self, v);
    }
    // Plain load + store; the caller holds the trait's precondition
    // (reached only through `SingleWriter`).
    unsafe fn transition_isr0(&self, event: crate::motor_mode::MotorEvent) {
        let mode = MotorMode::from_u8(self.motor_mode.load(Ordering::Relaxed));
        let new = mode.transition(event);
        if new != mode {
            self.motor_mode.store(new as u8, Ordering::Release);
        }
    }
    unsafe fn set_old_routine_isr0(&self, v: bool) {
        let mode = MotorMode::from_u8(self.motor_mode.load(Ordering::Relaxed));
        if v && mode.is_running() {
            self.motor_mode
                .store(MotorMode::OldRoutine as u8, Ordering::Release);
        } else if !v && mode.is_old_routine() {
            self.motor_mode
                .store(MotorMode::Running as u8, Ordering::Release);
        }
    }
    fn zero_crosses(&self) -> u32 {
        self.zero_crosses()
    }
    fn set_zero_crosses(&self, v: u32) {
        self.set_zero_crosses(v);
    }
    fn increment_zero_crosses(&self) {
        self.increment_zero_crosses();
    }
    unsafe fn increment_zero_crosses_isr0(&self) {
        let v = self.zero_crosses.load(Ordering::Relaxed);
        if v < 10000 {
            self.zero_crosses.store(v + 1, Ordering::Release);
        }
    }
    fn commutation_interval(&self) -> u32 {
        self.commutation_interval()
    }
    fn set_commutation_interval(&self, v: u32) {
        self.set_commutation_interval(v);
    }
    fn e_com_time(&self) -> i32 {
        self.e_com_time()
    }
    fn set_e_com_time(&self, v: i32) {
        SharedState::set_e_com_time(self, v);
    }
    fn gate_avg(&self) -> u32 {
        SharedState::gate_avg(self)
    }
    fn set_gate_avg(&self, v: u32) {
        SharedState::set_gate_avg(self, v);
    }
    fn interval_timer_count(&self) -> u32 {
        SharedState::interval_timer_count(self)
    }
    fn set_interval_timer_count(&self, v: u32) {
        SharedState::set_interval_timer_count(self, v);
    }
    fn duty_cycle(&self) -> u16 {
        SharedState::duty_cycle(self)
    }
    fn set_duty_cycle(&self, v: u16) {
        SharedState::set_duty_cycle(self, v);
    }
    fn forward(&self) -> bool {
        SharedState::forward(self)
    }
    fn set_forward(&self, v: bool) {
        SharedState::set_forward(self, v);
    }
    fn desync_check_pending(&self) -> bool {
        SharedState::desync_check_pending(self)
    }
    fn set_desync_check_pending(&self, v: bool) {
        SharedState::set_desync_check_pending(self, v);
    }
    fn changeover_step(&self) -> u8 {
        SharedState::changeover_step(self)
    }
    fn set_changeover_step(&self, step: u8) {
        SharedState::set_changeover_step(self, step);
    }
}

impl crate::shared_comm::Throttle for SharedState {
    fn input_set(&self) -> bool {
        SharedState::input_set(self)
    }
    fn set_input_set(&self, v: bool) {
        SharedState::set_input_set(self, v);
    }
    fn dshot_telemetry(&self) -> bool {
        SharedState::dshot_telemetry(self)
    }
    fn is_dshot(&self) -> bool {
        SharedState::dshot(self)
    }
    fn set_is_dshot(&self, v: bool) {
        SharedState::set_dshot(self, v);
    }
    fn newinput(&self) -> u16 {
        SharedState::newinput(self)
    }
    fn set_newinput(&self, v: u16) {
        SharedState::set_newinput(self, v);
    }
    fn adjusted_input(&self) -> u16 {
        SharedState::adjusted_input(self)
    }
    fn set_adjusted_input(&self, v: u16) {
        SharedState::set_adjusted_input(self, v);
    }
    fn duty_cycle_setpoint(&self) -> u16 {
        SharedState::duty_cycle_setpoint(self)
    }
    fn set_duty_cycle_setpoint(&self, v: u16) {
        SharedState::set_duty_cycle_setpoint(self, v);
    }
    fn signal_timeout(&self) -> u32 {
        self.signal_timeout()
    }
    fn increment_signal_timeout(&self) {
        self.increment_signal_timeout();
    }
    fn prop_brake_active(&self) -> bool {
        SharedState::prop_brake_active(self)
    }
    fn set_prop_brake_active(&self, v: bool) {
        SharedState::set_prop_brake_active(self, v);
    }
}

impl crate::shared_comm::Drive for SharedState {
    fn tim1_arr(&self) -> u16 {
        SharedState::tim1_arr(self)
    }
    fn set_tim1_arr(&self, v: u16) {
        SharedState::set_tim1_arr(self, v);
    }
    fn filter_level(&self) -> u8 {
        SharedState::filter_level(self)
    }
    fn set_filter_level(&self, v: u8) {
        SharedState::set_filter_level(self, v);
    }
    fn min_bemf_counts(&self) -> u8 {
        SharedState::min_bemf_counts(self)
    }
    fn set_min_bemf_counts(&self, v: u8) {
        SharedState::set_min_bemf_counts(self, v);
    }
    fn auto_advance(&self) -> u8 {
        SharedState::auto_advance(self)
    }
    fn set_auto_advance(&self, v: u8) {
        SharedState::set_auto_advance(self, v);
    }
}

impl crate::shared_comm::Protections for SharedState {
    fn duty_maximum(&self) -> u16 {
        SharedState::duty_maximum(self)
    }
    fn set_duty_maximum(&self, v: u16) {
        SharedState::set_duty_maximum(self, v);
    }
    fn stall_protection_adjust(&self) -> u16 {
        SharedState::stall_protection_adjust(self)
    }
    fn set_stall_protection_adjust(&self, v: u16) {
        SharedState::set_stall_protection_adjust(self, v);
    }
    fn current_limit_adjust(&self) -> u16 {
        SharedState::current_limit_adjust(self)
    }
    fn set_current_limit_adjust(&self, v: u16) {
        SharedState::set_current_limit_adjust(self, v);
    }
}

impl crate::shared_comm::Telemetry for SharedState {
    fn send_telemetry(&self) -> bool {
        SharedState::send_telemetry(self)
    }
    fn set_send_telemetry(&self, v: bool) {
        SharedState::set_send_telemetry(self, v);
    }
    fn send_esc_info_flag(&self) -> bool {
        SharedState::send_esc_info_flag(self)
    }
    fn set_send_esc_info_flag(&self, v: bool) {
        SharedState::set_send_esc_info_flag(self, v);
    }
    fn telem_counter_check_and_inc(&self, limit: u16) -> bool {
        SharedState::telem_counter_check_and_inc(self, limit)
    }
    fn battery_voltage(&self) -> u16 {
        SharedState::battery_voltage(self)
    }
    fn set_battery_voltage(&self, v: u16) {
        SharedState::set_battery_voltage(self, v);
    }
    fn set_actual_current(&self, v: i16) {
        SharedState::set_actual_current(self, v);
    }
    fn set_degrees_celsius(&self, v: i16) {
        SharedState::set_degrees_celsius(self, v);
    }
}

impl crate::shared_comm::Commands for SharedState {
    fn isr_action(&self) -> crate::shared_comm::IsrAction {
        SharedState::isr_action(self)
    }
    fn request_isr_action(&self, action: crate::shared_comm::IsrAction) {
        SharedState::request_isr_action(self, action);
    }
    fn clear_isr_action(&self, action: crate::shared_comm::IsrAction) {
        SharedState::clear_isr_action(self, action);
    }
    fn save_settings_flag(&self) -> bool {
        SharedState::save_settings_flag(self)
    }
    fn set_save_settings_flag(&self, v: bool) {
        SharedState::set_save_settings_flag(self, v);
    }
    fn increment_control_ticks(&self) {
        SharedState::increment_control_ticks(self);
    }
    fn one_khz_counter_inc(&self) {
        SharedState::one_khz_counter_inc(self);
    }
    fn one_khz_counter_check_and_reset(&self, divider: u8) -> bool {
        SharedState::one_khz_counter_check_and_reset(self, divider)
    }
}

#[cfg(test)]
mod tests {
    use crate::shared_comm::{Commands, IsrAction, MotorStatus};

    use super::*;

    #[test]
    fn isr_action_handoff_is_clearable() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        assert_eq!(Commands::isr_action(&shared), IsrAction::None);
        Commands::request_isr_action(&shared, IsrAction::ResetIntervalTimer);
        Commands::request_isr_action(&shared, IsrAction::AllOff);
        assert_eq!(Commands::isr_action(&shared), IsrAction::AllOff);
        Commands::clear_isr_action(&shared, IsrAction::AllOff);
        assert_eq!(Commands::isr_action(&shared), IsrAction::None);
    }

    #[test]
    fn clear_isr_action_keeps_newer_higher_priority_request() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        Commands::request_isr_action(&shared, IsrAction::ResetIntervalTimer);
        Commands::request_isr_action(&shared, IsrAction::AllOff);
        Commands::clear_isr_action(&shared, IsrAction::ResetIntervalTimer);

        assert_eq!(Commands::isr_action(&shared), IsrAction::AllOff);
    }

    #[test]
    fn changeover_step_handoff_roundtrips() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        assert_eq!(MotorStatus::changeover_step(&shared), 0);
        MotorStatus::set_changeover_step(&shared, 5);
        assert_eq!(MotorStatus::changeover_step(&shared), 5);
    }

    #[test]
    fn config_write_queue_retains_eight_writes() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        for offset in 0..CONFIG_WRITE_QUEUE_CAPACITY {
            assert!(shared.push_config_write(offset, offset + 10));
        }
        assert!(!shared.push_config_write(99, 100));

        for offset in 0..CONFIG_WRITE_QUEUE_CAPACITY {
            assert_eq!(shared.pop_config_write(), Some((offset, offset + 10)));
        }
        assert_eq!(shared.pop_config_write(), None);
    }

    #[test]
    fn desync_check_pending_roundtrips() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        assert!(!MotorStatus::desync_check_pending(&shared));
        MotorStatus::set_desync_check_pending(&shared, true);
        assert!(MotorStatus::desync_check_pending(&shared));
        MotorStatus::set_desync_check_pending(&shared, false);
        assert!(!MotorStatus::desync_check_pending(&shared));
    }

    #[test]
    fn take_desync_check_pending_clears_atomically() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        shared.set_desync_check_pending(true);

        assert!(shared.take_desync_check_pending());
        assert!(!shared.take_desync_check_pending());
    }

    #[test]
    fn one_khz_counter_saturates_until_consumed() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        for _ in 0..300 {
            shared.one_khz_counter_inc();
        }

        assert!(shared.one_khz_counter_check_and_reset(20));
        assert!(!shared.one_khz_counter_check_and_reset(20));
    }

    #[test]
    fn one_khz_counter_dispatches_on_divider_tick() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        for _ in 0..19 {
            shared.one_khz_counter_inc();
        }
        assert!(!shared.one_khz_counter_check_and_reset(20));

        shared.one_khz_counter_inc();
        assert!(shared.one_khz_counter_check_and_reset(20));
        assert!(!shared.one_khz_counter_check_and_reset(20));
    }

    #[test]
    fn telem_counter_dispatches_on_limit_tick() {
        let shared = SharedState::new::<crate::platform::TestPlatform>();

        for _ in 0..4 {
            assert!(!shared.telem_counter_check_and_inc(5));
        }

        assert!(shared.telem_counter_check_and_inc(5));
        assert!(!shared.telem_counter_check_and_inc(5));
    }
}
