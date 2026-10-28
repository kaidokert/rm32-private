//! Transfer complete dispatcher.
//!
//! Equivalent to C `transfercomplete()` — the central dispatcher that handles
//! DMA completion for both DShot and servo input, auto-detection, bidir
//! telemetry, and unarmed frame averaging.

use crate::dshot;
use crate::functions::get_abs_dif;
use crate::platform::Platform;
use crate::servo::{ServoResult, ServoState};
use crate::signal;

/// Transfer complete processing state.
#[derive(Default)]
pub struct TransferState {
    pub servo: ServoState,
    // Unarmed DShot frame averaging
    average_count: u8,
    average_packet_length: u32,
    // Calibration entry
    enter_calibration_count: u8,
    last_input: u16,
    high_pin_count: u8,
    bidir_confirms: u8,
    pending_protocol: Option<DetectedProtocol>,
}

/// Detected input protocol during auto-detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectedProtocol {
    Dshot,
    Servo,
}

/// Primary action from transfer complete processing.
#[derive(Debug)]
pub enum TransferAction {
    /// No valid frame decoded (bad CRC, timing, or rising edge)
    None,
    /// Input protocol detected (first frame after boot)
    InputDetected(DetectedProtocol),
    /// Valid DShot throttle frame
    DshotThrottle { value: u16, telemetry: bool },
    /// Valid DShot command frame
    DshotCommand { cmd: u16, telemetry: bool },
    /// Valid servo throttle
    ServoThrottle(u16),
    /// Servo calibration in progress (signal alive, no throttle value)
    ServoCalibrating,
    /// Servo calibration complete; persist thresholds to EEPROM.
    ServoCalibrationDone {
        low_threshold: u8,
        high_threshold: u8,
    },
}

/// Capture setup requested for the next input frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    /// Edges (timestamps) to capture before the next frame callback (a
    /// DMA port's transfer count; AM32's `buffersize`).
    pub edges_per_burst: u32,
    /// Timer prescaler update, if protocol detection changed the capture rate.
    pub prescaler: Option<u16>,
}

impl CaptureConfig {
    pub const DSHOT: Self = Self {
        edges_per_burst: 32,
        prescaler: None,
    };
    /// DShot detection uses one full frame. Using 33 edges consumes the next
    /// frame's first edge and can leave steady-state capture misaligned.
    pub const fn dshot_detection<P: Platform>() -> Self {
        Self {
            edges_per_burst: 32,
            prescaler: Some((P::CAPTURE_HZ / 6_000_000) as u16),
        }
    }
    pub const SERVO: Self = Self {
        edges_per_burst: 2,
        prescaler: None,
    };
    pub const SERVO_REALIGN: Self = Self {
        edges_per_burst: 3,
        prescaler: None,
    };

    pub const DSHOT600_DETECTED: Self = Self {
        edges_per_burst: 32,
        prescaler: Some(0),
    };
    pub const DSHOT300_DETECTED: Self = Self {
        edges_per_burst: 32,
        prescaler: Some(1),
    };
    pub const DSHOT150_DETECTED: Self = Self {
        edges_per_burst: 32,
        prescaler: Some(3),
    };

    pub const fn servo_detected<P: Platform>() -> Self {
        Self {
            edges_per_burst: 2,
            prescaler: Some((P::CAPTURE_HZ / 1_000_000).saturating_sub(1) as u16),
        }
    }
}

/// Actions the caller (ISR) should take after transfer complete.
pub struct TransferActions {
    /// Primary action
    pub action: TransferAction,
    /// Capture setup for the next DMA cycle.
    pub next_capture: CaptureConfig,
    /// DShot frame timing update (from unarmed averaging)
    pub frametime: Option<(u16, u16)>,
    /// Bidirectional DShot auto-detected.
    pub bidir_detected: bool,
    /// Snapshot of `high_pin_count` for the bidir auto-detect path
    /// (published to a SharedState counter for diagnostics).
    #[cfg(feature = "bench-diag")]
    pub high_pin_count: u8,
}

impl TransferState {
    /// Process a DMA transfer complete event.
    ///
    /// `edges`: the captured DMA data (32 entries for DShot, 2-3 for servo)
    /// `input_set`: whether input type has been detected
    /// `dshot_mode`: whether DShot is the active input
    /// `servo_mode`: whether servo PWM is the active input
    /// `armed`: motor armed state
    /// `dshot_telemetry`: bidirectional DShot mode
    /// `input_pin_high`: current state of input pin (for servo edge detection)
    /// `adjusted_input`: current throttle for calibration entry check
    /// `current_newinput`: current newinput for rate limiting
    /// `bidirectional`: config bi_direction flag
    /// `disable_stick_cal`: config disable_stick_calibration flag
    /// `zero_input_count`: current zero input counter
    /// `frametime_low/high`: current DShot frame timing bounds
    /// `P`: the port's `Platform` (`CAPTURE_HZ` sets the detection prescalers)
    #[allow(clippy::too_many_arguments)]
    pub fn process<P: Platform>(
        &mut self,
        edges: &[u32],
        input_set: bool,
        dshot_mode: bool,
        servo_mode: bool,
        dshot_telemetry: bool,
        armed: bool,
        input_pin_high: bool,
        adjusted_input: u16,
        current_newinput: u16,
        bidirectional: bool,
        disable_stick_cal: bool,
        zero_input_count: &mut u16,
        frametime_low: u16,
        frametime_high: u16,
    ) -> TransferActions {
        let mut action = TransferAction::None;
        let mut frametime = None;

        // --- Input detection ---
        if !input_set {
            let sig = signal::detect_input(edges);
            let protocol = match sig {
                signal::SignalType::Dshot600
                | signal::SignalType::Dshot300
                | signal::SignalType::Dshot150 => Some(DetectedProtocol::Dshot),
                signal::SignalType::ServoPwm => Some(DetectedProtocol::Servo),
                signal::SignalType::None => None,
            };
            let confirmed = match (protocol, self.pending_protocol) {
                (Some(protocol), Some(pending)) if protocol == pending => {
                    self.pending_protocol = None;
                    Some(protocol)
                }
                (Some(protocol), _) => {
                    self.pending_protocol = Some(protocol);
                    None
                }
                (None, _) => {
                    self.pending_protocol = None;
                    None
                }
            };
            let (action, next_capture) = if let Some(protocol) = confirmed {
                let capture = match sig {
                    signal::SignalType::Dshot600 => CaptureConfig::DSHOT600_DETECTED,
                    signal::SignalType::Dshot300 => CaptureConfig::DSHOT300_DETECTED,
                    signal::SignalType::Dshot150 => CaptureConfig::DSHOT150_DETECTED,
                    signal::SignalType::ServoPwm => CaptureConfig::servo_detected::<P>(),
                    signal::SignalType::None => {
                        unreachable!("confirmed input detection cannot have SignalType::None")
                    }
                };
                (TransferAction::InputDetected(protocol), capture)
            } else {
                (TransferAction::None, CaptureConfig::dshot_detection::<P>())
            };
            return TransferActions {
                action,
                next_capture,
                frametime,
                bidir_detected: false,
                #[cfg(feature = "bench-diag")]
                high_pin_count: self.high_pin_count,
            };
        }

        let mut bidir_detected = false;

        // --- DShot processing ---
        if dshot_mode && edges.len() >= 32 {
            let buf: [u32; 32] = {
                let mut b = [0u32; 32];
                b.copy_from_slice(&edges[..32]);
                b
            };
            let frame = dshot::decode_frame(&buf, frametime_low, frametime_high, dshot_telemetry);
            if !armed
                && !dshot_telemetry
                && input_pin_high
                && self.high_pin_count >= crate::constants::BIDIR_IDLE_HIGH_FRAMES
            {
                let inverted = dshot::decode_frame(&buf, frametime_low, frametime_high, true);
                let inverted_ok = matches!(
                    inverted,
                    dshot::DshotFrame::Throttle { .. } | dshot::DshotFrame::Command { .. }
                );
                if inverted_ok {
                    self.bidir_confirms = self.bidir_confirms.saturating_add(1);
                    if self.bidir_confirms >= crate::constants::BIDIR_CONFIRM_FRAMES {
                        bidir_detected = true;
                    }
                } else {
                    self.bidir_confirms = 0;
                    if matches!(
                        frame,
                        dshot::DshotFrame::Throttle { .. } | dshot::DshotFrame::Command { .. }
                    ) {
                        self.high_pin_count = 0;
                    }
                }
            }
            action = match frame {
                dshot::DshotFrame::Throttle { value, telemetry } => {
                    TransferAction::DshotThrottle { value, telemetry }
                }
                dshot::DshotFrame::Command { cmd, telemetry } => {
                    TransferAction::DshotCommand { cmd, telemetry }
                }
                _ => TransferAction::None,
            };
        }
        // --- Servo processing (mutually exclusive with DShot) ---
        else if servo_mode {
            if input_pin_high {
                // Rising edge — wait for falling to get pulse width
            } else if edges.len() >= 2 {
                let pulse = edges[1].wrapping_sub(edges[0]) as u16;
                action = match self.servo.compute(pulse, current_newinput, bidirectional) {
                    ServoResult::Throttle(v) => TransferAction::ServoThrottle(v),
                    ServoResult::OutOfRange => {
                        *zero_input_count = 0;
                        TransferAction::None
                    }
                    ServoResult::Calibrating | ServoResult::CalibrationHighDone => {
                        TransferAction::ServoCalibrating
                    }
                    ServoResult::CalibrationDone {
                        low_threshold_eeprom,
                        high_threshold_eeprom,
                    } => TransferAction::ServoCalibrationDone {
                        low_threshold: low_threshold_eeprom,
                        high_threshold: high_threshold_eeprom,
                    },
                };
            }
        }

        // --- Unarmed housekeeping ---
        if !armed {
            if dshot_mode && !dshot_telemetry && input_pin_high {
                self.high_pin_count = self.high_pin_count.saturating_add(1);
            } else {
                self.high_pin_count = 0;
                self.bidir_confirms = 0;
            }
        } else {
            self.high_pin_count = 0;
            self.bidir_confirms = 0;
        }

        if !armed {
            // DShot frame averaging (for dshot_frametime calibration)
            if dshot_mode && self.average_count < 8 && *zero_input_count > 5 {
                self.average_count += 1;
                if edges.len() >= 32 {
                    self.average_packet_length += (edges[31].wrapping_sub(edges[0])) as u16 as u32;
                }
                if self.average_count == 8 {
                    let avg = self.average_packet_length >> 3;
                    let high = (avg + (self.average_packet_length >> 7)) as u16;
                    let low = (avg - (self.average_packet_length >> 7)) as u16;
                    frametime = Some((low, high));
                }
            }

            // Calibration entry detection
            if adjusted_input == 0 && !self.servo.calibration_required() {
                *zero_input_count += 1;
            } else if !disable_stick_cal {
                *zero_input_count = 0;
                if adjusted_input > crate::constants::CALIBRATION_MIN_THROTTLE {
                    if get_abs_dif(adjusted_input as i32, self.last_input as i32)
                        > crate::constants::CALIBRATION_MAX_JITTER
                    {
                        self.enter_calibration_count = 0;
                    } else {
                        self.enter_calibration_count += 1;
                    }
                    if self.enter_calibration_count > crate::constants::CALIBRATION_ENTRY_COUNT
                        && !self.servo.high_calibration_set()
                    {
                        self.servo.set_calibration_required(true);
                        self.enter_calibration_count = 0;
                    }
                    self.last_input = adjusted_input;
                }
            }
        }

        let next_capture = if servo_mode && input_pin_high {
            CaptureConfig::SERVO_REALIGN
        } else if servo_mode {
            CaptureConfig::SERVO
        } else {
            CaptureConfig::DSHOT
        };

        TransferActions {
            action,
            next_capture,
            frametime,
            bidir_detected,
            #[cfg(feature = "bench-diag")]
            high_pin_count: self.high_pin_count,
        }
    }
}

/// ISR-side input state the transfer dispatch reads and updates (AM32
/// `transfercomplete`'s globals). Firmware and harness both build it from
/// their own fields.
pub struct InputContext<'a> {
    pub config: &'a mut crate::config::EepromConfig,
    pub cmd: &'a mut crate::dshot_commands::CommandProcessor,
    pub edt: &'a mut crate::edt::EdtScheduler,
    pub forward: &'a mut bool,
    pub edt_armed: &'a mut bool,
    pub edt_arm_enable: bool,
    pub frametime_low: &'a mut u16,
    pub frametime_high: &'a mut u16,
}

/// Apply one frame's `TransferActions` to the shared and ISR-side state:
/// the one dispatch firmware and harness share (S7.3; the harness copy had
/// drifted: no config-diff writes, `ProgrammingCommit`, EDT init/deinit or
/// calibration writes). `set_prescaler` receives the bidirectional DShot
/// output prescaler when detection locks DShot (the port's register work).
///
/// `on_command` runs first on a decoded command (a port's bench hooks).
/// Both hooks run where the firmware's inline code ran them: hooked outside
/// the match, LLVM tail-duplicated a shared-state store into a second path.
#[inline(always)]
pub fn dispatch<C: FnOnce(u16), P: FnOnce(crate::hal::DshotOutputPrescaler)>(
    actions: &TransferActions,
    shared: &crate::shared_state::SharedState,
    ctx: InputContext<'_>,
    on_command: C,
    set_prescaler: P,
) {
    use crate::dshot_commands::CommandResult;
    match actions.action {
        TransferAction::InputDetected(proto) => {
            shared.set_input_set(true);
            match proto {
                DetectedProtocol::Dshot => {
                    shared.set_dshot(true);
                    if let Some(prescaler) = actions.next_capture.prescaler {
                        set_prescaler(
                            crate::hal::DshotOutputPrescaler::new(prescaler)
                                .expect("invalid DShot output prescaler"),
                        );
                    }
                }
                DetectedProtocol::Servo => shared.set_servo_pwm(true),
            }
        }
        TransferAction::DshotThrottle { value, telemetry } => {
            // EDT arm gate: throttle passes once armed (or EDT not required);
            // zero always passes and disarms. The gate reads `edt_armed`
            // before the disarm; the store comes last so LLVM keeps one
            // `newinput` store (store-then-disarm got tail-duplicated).
            let pass = !ctx.edt_arm_enable || *ctx.edt_armed || value == 0;
            if value == 0 && ctx.edt_arm_enable {
                *ctx.edt_armed = false;
            }
            if pass {
                shared.set_newinput(value);
            }
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
        }
        TransferAction::DshotCommand { cmd, telemetry } => {
            on_command(cmd);
            shared.set_newinput(0);
            if telemetry {
                shared.set_send_telemetry(true);
            }
            shared.set_signal_timeout(0);
            let old_config = *ctx.config;
            let result = ctx.cmd.process(
                cmd,
                shared.armed(),
                shared.running(),
                ctx.config,
                ctx.forward,
                ctx.edt_armed,
                ctx.edt_arm_enable,
            );
            for (offset, (old, new)) in old_config
                .as_bytes()
                .iter()
                .zip(ctx.config.as_bytes().iter())
                .enumerate()
            {
                if old != new {
                    shared.push_config_write(offset as u8, *new);
                }
            }
            match result {
                CommandResult::SaveSettings => shared.set_save_settings_flag(true),
                CommandResult::PlayTone(tone) => shared.set_tone_request(tone),
                CommandResult::SendEscInfo => shared.set_send_esc_info_flag(true),
                CommandResult::ProgrammingCommit { position, value } => {
                    shared.push_config_write(position as u8, value);
                }
                _ => {}
            }
            shared.set_forward(*ctx.forward);
            if ctx.cmd.take_edt_init() {
                ctx.edt.request_init();
            }
            if ctx.cmd.take_edt_deinit() {
                ctx.edt.request_deinit();
            }
        }
        TransferAction::ServoThrottle(value) => {
            shared.set_newinput(value);
            shared.set_signal_timeout(0);
        }
        TransferAction::ServoCalibrating => {
            shared.set_signal_timeout(0);
        }
        TransferAction::ServoCalibrationDone {
            low_threshold,
            high_threshold,
        } => {
            // Persist to the config; publish the bytes so main's copy is
            // current when it acts on the save flag.
            ctx.config.servo_low_threshold = low_threshold;
            ctx.config.servo_high_threshold = high_threshold;
            shared.push_config_write(
                core::mem::offset_of!(crate::config::EepromConfig, servo_low_threshold) as u8,
                low_threshold,
            );
            shared.push_config_write(
                core::mem::offset_of!(crate::config::EepromConfig, servo_high_threshold) as u8,
                high_threshold,
            );
            shared.set_save_settings_flag(true);
            shared.set_signal_timeout(0);
        }
        TransferAction::None => {}
    }
    if let Some((low, high)) = actions.frametime {
        *ctx.frametime_low = low;
        *ctx.frametime_high = high;
    }
    if actions.bidir_detected {
        shared.set_dshot_telemetry(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test platforms at the capture clocks the tests were written for.
    macro_rules! mhz {
        ($($name:ident = $mhz:expr),*) => {$(
            struct $name;
            impl Platform for $name {
                const TICK_HZ: u32 = 20_000;
                const TIMER_HZ: u32 = 2_000_000;
                const CAPTURE_HZ: u32 = $mhz * 1_000_000;
                const GCR_SHIFT: u8 = 7;
                const VOLTAGE_DIVIDER_X100: u16 = 1100;
                const MILLIVOLT_PER_AMP: u16 = 20;
                const CURRENT_OFFSET: i16 = 0;
                const USE_NTC: bool = false;
                const PWM_BASE_PERIOD: u16 = 1999;
                type Tuning = crate::tuning::QualifiedRigTuning;
            }
        )*};
    }
    mhz!(Mhz60 = 60, Mhz64 = 64, Mhz80 = 80);

    const DSHOT300_START_TICKS: u32 = 5000;
    const DSHOT300_ZERO_HIGHS: [u32; 16] = [
        47, 47, 47, 46, 47, 47, 47, 46, 47, 46, 47, 47, 46, 47, 47, 47,
    ];
    const DSHOT300_ZERO_LOWS: [u32; 16] = [
        87, 86, 87, 87, 86, 87, 87, 87, 86, 87, 87, 86, 87, 87, 87, 0,
    ];
    const INTER_FRAME_GAP_TICKS: u32 = 78_400;

    #[test]
    fn capture_config_edges_per_burst() {
        assert_eq!(CaptureConfig::DSHOT.edges_per_burst, 32);
        assert_eq!(CaptureConfig::SERVO.edges_per_burst, 2);
        assert_eq!(CaptureConfig::SERVO_REALIGN.edges_per_burst, 3);
    }

    fn dshot_dma_buffer(value: u16, telem: bool, inverted_crc: bool) -> [u32; 32] {
        let mut bits = [0u8; 16];
        for (i, bit) in bits[..11].iter_mut().enumerate() {
            *bit = ((value >> (10 - i)) & 1) as u8;
        }
        bits[11] = u8::from(telem);

        let mut crc = (bits[0] ^ bits[4] ^ bits[8]) << 3
            | (bits[1] ^ bits[5] ^ bits[9]) << 2
            | (bits[2] ^ bits[6] ^ bits[10]) << 1
            | (bits[3] ^ bits[7] ^ bits[11]);
        if inverted_crc {
            crc = (!crc) & 0xF;
        }
        bits[12] = (crc >> 3) & 1;
        bits[13] = (crc >> 2) & 1;
        bits[14] = (crc >> 1) & 1;
        bits[15] = crc & 1;

        let mut buf = [0u32; 32];
        let mut base = 1000u32;
        for (i, bit) in bits.iter().enumerate() {
            buf[i * 2] = base;
            buf[i * 2 + 1] = base + if *bit != 0 { 22 } else { 10 };
            base += 32;
        }
        buf
    }

    fn dshot300_disarm_capture() -> [u32; 32] {
        let mut buf = [0u32; 32];
        let mut t = DSHOT300_START_TICKS;
        for i in 0..16 {
            buf[i * 2] = t;
            buf[i * 2 + 1] = t + DSHOT300_ZERO_HIGHS[i];
            t += DSHOT300_ZERO_HIGHS[i] + DSHOT300_ZERO_LOWS[i];
        }
        buf
    }

    fn gap_spanning_capture() -> [u32; 32] {
        let mut buf = [0u32; 32];
        let mut t = DSHOT300_START_TICKS;
        for slot in buf.iter_mut().take(9) {
            *slot = t;
            t += 67;
        }
        t += INTER_FRAME_GAP_TICKS;
        for slot in buf.iter_mut().skip(9) {
            *slot = t;
            t += 67;
        }
        buf
    }

    #[test]
    fn servo_pin_high_requests_realign_capture() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let actions = state.process::<Mhz64>(
            &[0, 0, 0],
            true,
            false,
            true,
            false,
            false,
            true,
            0,
            0,
            false,
            false,
            &mut zic,
            400,
            600,
        );

        assert_eq!(actions.next_capture, CaptureConfig::SERVO_REALIGN);
    }

    #[test]
    fn servo_pin_low_requests_normal_capture() {
        let mut state = TransferState::default();
        state.servo.set_calibration(1100, 1900, 1500, 100);
        let mut zic = 0;
        let actions = state.process::<Mhz64>(
            &[1000, 2500],
            true,
            false,
            true,
            false,
            false,
            false,
            0,
            0,
            false,
            false,
            &mut zic,
            400,
            600,
        );

        assert_eq!(actions.next_capture, CaptureConfig::SERVO);
        assert!(matches!(actions.action, TransferAction::ServoThrottle(_)));
    }

    #[test]
    fn dshot_mode_requests_dshot_capture() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let actions = state.process::<Mhz64>(
            &[0; 32], true, true, false, false, false, false, 0, 0, false, false, &mut zic, 400,
            600,
        );

        assert_eq!(actions.next_capture, CaptureConfig::DSHOT);
    }

    #[test]
    fn dshot_detection_uses_one_frame_capture_window() {
        assert_eq!(
            CaptureConfig::dshot_detection::<Mhz60>().edges_per_burst,
            32
        );
        assert_eq!(
            CaptureConfig::dshot_detection::<Mhz60>().prescaler,
            Some(10)
        );
    }

    #[test]
    fn normal_dshot_with_high_pin_does_not_commit_bidir() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let frame = dshot_dma_buffer(0, false, false);

        for _ in 0..200 {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400,
                600,
            );

            assert!(!actions.bidir_detected);
            assert!(matches!(
                actions.action,
                TransferAction::DshotThrottle { value: 0, .. }
            ));
        }
    }

    #[test]
    fn inverted_crc_dshot_with_high_pin_commits_bidir_after_confirms() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let frame = dshot_dma_buffer(0, false, true);
        let mut detected_at = None;
        let expected_detect_at = crate::constants::BIDIR_IDLE_HIGH_FRAMES as usize
            + crate::constants::BIDIR_CONFIRM_FRAMES as usize
            - 1;

        for tick in 0..150 {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400,
                600,
            );

            if actions.bidir_detected {
                detected_at = Some(tick);
                break;
            }
        }

        assert_eq!(detected_at, Some(expected_detect_at));
    }

    #[test]
    fn bidir_autodetect_is_unarmed_only() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let frame = dshot_dma_buffer(0, false, true);

        for _ in 0..150 {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, true, true, 0, 0, false, false, &mut zic, 400,
                600,
            );

            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn bidir_autodetect_requires_high_pin() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let frame = dshot_dma_buffer(0, false, true);

        for _ in 0..150 {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 400,
                600,
            );

            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn bidir_autodetect_resets_when_pin_goes_low() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let normal = dshot_dma_buffer(0, false, false);
        let inverted = dshot_dma_buffer(0, false, true);

        for _ in 0..150 {
            let actions = state.process::<Mhz64>(
                &normal, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400,
                600,
            );
            assert!(!actions.bidir_detected);
        }

        for _ in 0..10 {
            let actions = state.process::<Mhz64>(
                &inverted, true, true, false, false, false, false, 0, 0, false, false, &mut zic,
                400, 600,
            );
            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn bidir_autodetect_resets_when_armed() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let frame = dshot_dma_buffer(0, false, true);
        let partial_confirm_frames = crate::constants::BIDIR_IDLE_HIGH_FRAMES as usize
            + crate::constants::BIDIR_CONFIRM_FRAMES as usize
            - 1;

        for _ in 0..partial_confirm_frames {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400,
                600,
            );
            assert!(!actions.bidir_detected);
        }

        let armed = state.process::<Mhz64>(
            &frame, true, true, false, false, true, true, 0, 0, false, false, &mut zic, 400, 600,
        );
        assert!(!armed.bidir_detected);

        for _ in 0..crate::constants::BIDIR_CONFIRM_FRAMES {
            let actions = state.process::<Mhz64>(
                &frame, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400,
                600,
            );
            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn first_dshot_detection_stays_tentative() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 5;
        }

        let actions = state.process::<Mhz60>(
            &buf, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
        );

        assert!(matches!(actions.action, TransferAction::None));
        assert_eq!(
            actions.next_capture,
            CaptureConfig::dshot_detection::<Mhz60>()
        );
        assert_eq!(
            actions.next_capture.edges_per_burst,
            CaptureConfig::DSHOT.edges_per_burst
        );
    }

    #[test]
    fn invalid_detection_clears_pending_protocol() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let mut dshot = [0u32; 33];
        for (i, slot) in dshot.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 5;
        }
        let invalid = [0u32; 33];

        let _ = state.process::<Mhz60>(
            &dshot, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400,
            600,
        );
        let _ = state.process::<Mhz60>(
            &invalid, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400,
            600,
        );
        let actions = state.process::<Mhz60>(
            &dshot, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400,
            600,
        );

        assert!(matches!(actions.action, TransferAction::None));
        assert_eq!(
            actions.next_capture,
            CaptureConfig::dshot_detection::<Mhz60>()
        );
    }

    #[test]
    fn bf_dshot300_disarm_frame_decodes() {
        let buf = dshot300_disarm_capture();
        let mut state = TransferState::default();
        let mut zic = 0u16;
        let actions = state.process::<Mhz80>(
            &buf, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 100, 60000,
        );

        assert!(
            matches!(
                actions.action,
                TransferAction::DshotThrottle { value: 0, .. }
            ),
            "expected DShot throttle 0, got {:?}",
            actions.action
        );
    }

    #[test]
    fn gap_spanning_capture_rejected() {
        let buf = gap_spanning_capture();
        let frame = dshot::decode_frame(&buf, 400, 600, false);

        assert!(
            matches!(frame, dshot::DshotFrame::InvalidTiming),
            "gap-spanning capture decoded as {:?}",
            frame
        );
    }

    #[test]
    fn matching_dshot_detection_locks_protocol_with_rate_prescaler() {
        for (step, expected_capture) in [
            (3, CaptureConfig::DSHOT600_DETECTED),
            (5, CaptureConfig::DSHOT300_DETECTED),
            (10, CaptureConfig::DSHOT150_DETECTED),
        ] {
            let mut state = TransferState::default();
            let mut zic = 0;
            let mut buf = [0u32; 33];
            for (i, slot) in buf.iter_mut().enumerate() {
                *slot = 100 + i as u32 * step;
            }

            let _ = state.process::<Mhz60>(
                &buf, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400,
                600,
            );
            let actions = state.process::<Mhz60>(
                &buf, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400,
                600,
            );

            assert!(matches!(
                actions.action,
                TransferAction::InputDetected(DetectedProtocol::Dshot)
            ));
            assert_eq!(actions.next_capture, expected_capture);
        }
    }

    #[test]
    fn matching_servo_detection_locks_protocol() {
        let mut state = TransferState::default();
        let mut zic = 0;
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 1000;
        }

        let _ = state.process::<Mhz60>(
            &buf, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
        );
        let actions = state.process::<Mhz60>(
            &buf, false, false, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
        );

        assert!(matches!(
            actions.action,
            TransferAction::InputDetected(DetectedProtocol::Servo)
        ));
        assert_eq!(
            actions.next_capture,
            CaptureConfig::servo_detected::<Mhz60>()
        );
    }

    #[test]
    fn servo_calibration_done_returns_thresholds() {
        let mut state = TransferState::default();
        state.servo.set_calibration_required(true);
        let mut zic = 0;

        for _ in 0..51 {
            let high = state.process::<Mhz64>(
                &[1000, 2900],
                true,
                false,
                true,
                false,
                false,
                false,
                0,
                0,
                false,
                false,
                &mut zic,
                400,
                600,
            );
            assert!(matches!(
                high.action,
                TransferAction::ServoCalibrating | TransferAction::ServoCalibrationDone { .. }
            ));
        }

        let mut done = TransferAction::None;
        for _ in 0..76 {
            done = state
                .process::<Mhz64>(
                    &[1000, 2100],
                    true,
                    false,
                    true,
                    false,
                    false,
                    false,
                    0,
                    0,
                    false,
                    false,
                    &mut zic,
                    400,
                    600,
                )
                .action;
        }

        assert!(matches!(
            done,
            TransferAction::ServoCalibrationDone {
                low_threshold: _,
                high_threshold: _
            }
        ));
    }

    // ---- S7.3: the shared dispatch -------------------------------------

    struct Isr {
        config: crate::config::EepromConfig,
        cmd: crate::dshot_commands::CommandProcessor,
        edt: crate::edt::EdtScheduler,
        forward: bool,
        edt_armed: bool,
        edt_arm_enable: bool,
        ft: (u16, u16),
    }

    impl Isr {
        fn new(edt_arm_enable: bool) -> Self {
            Self {
                config: crate::config::EepromConfig::default(),
                cmd: Default::default(),
                edt: Default::default(),
                forward: true,
                edt_armed: false,
                edt_arm_enable,
                ft: (400, 600),
            }
        }
        /// Dispatch one action; returns (commands hooked, prescaler set).
        fn run(
            &mut self,
            shared: &crate::shared_state::SharedState,
            action: TransferAction,
        ) -> (u32, Option<u16>) {
            let actions = TransferActions {
                action,
                next_capture: CaptureConfig::DSHOT300_DETECTED,
                frametime: None,
                bidir_detected: false,
                #[cfg(feature = "bench-diag")]
                high_pin_count: 0,
            };
            let (mut hooked, mut psc) = (0, None);
            dispatch(
                &actions,
                shared,
                InputContext {
                    config: &mut self.config,
                    cmd: &mut self.cmd,
                    edt: &mut self.edt,
                    forward: &mut self.forward,
                    edt_armed: &mut self.edt_armed,
                    edt_arm_enable: self.edt_arm_enable,
                    frametime_low: &mut self.ft.0,
                    frametime_high: &mut self.ft.1,
                },
                |_| hooked += 1,
                |p| psc = Some(p.raw()),
            );
            (hooked, psc)
        }
    }

    fn armed_shared() -> crate::shared_state::SharedState {
        let shared = crate::shared_state::SharedState::new::<Mhz64>();
        shared.set_armed(true);
        assert!(shared.armed());
        shared
    }

    fn command(cmd: u16) -> TransferAction {
        TransferAction::DshotCommand {
            cmd,
            telemetry: false,
        }
    }

    /// A config-changing command publishes exactly the changed byte to main
    /// (the write the harness copy never made) and the direction.
    #[test]
    fn dispatch_command_publishes_config_diff() {
        let shared = armed_shared();
        let mut isr = Isr::new(false);
        for _ in 0..6 {
            assert_eq!(
                isr.run(&shared, command(crate::dshot::commands::DIRECTION_REVERSED))
                    .0,
                1
            );
        }
        assert_eq!(isr.config.dir_reversed, 1);
        assert!(!isr.forward);
        assert!(!shared.forward());
        let off = core::mem::offset_of!(crate::config::EepromConfig, dir_reversed) as u8;
        assert_eq!(shared.pop_config_write(), Some((off, 1)));
        assert_eq!(shared.pop_config_write(), None);
    }

    /// Beacon commands become the tick tone request.
    #[test]
    fn dispatch_beacon_requests_tone() {
        let shared = armed_shared();
        let mut isr = Isr::new(false);
        isr.run(&shared, command(3));
        assert_eq!(shared.take_tone_request(), 3);
    }

    /// EDT arm gate: throttle is held until EDT_ENABLE arms it, zero always
    /// passes and disarms, and EDT_ENABLE queues the EDT init frame.
    #[test]
    fn dispatch_edt_arm_gate() {
        let shared = armed_shared();
        let mut isr = Isr::new(true);
        let throttle = |value| TransferAction::DshotThrottle {
            value,
            telemetry: false,
        };
        shared.set_newinput(7);
        isr.run(&shared, throttle(500));
        assert_eq!(shared.newinput(), 7, "held before EDT arms");
        for _ in 0..6 {
            isr.run(&shared, command(crate::dshot::commands::EDT_ENABLE));
        }
        assert!(isr.edt_armed);
        assert!(matches!(
            isr.edt.next_frame(0, 0, 0),
            crate::edt::EdtFrame::Extended(crate::edt::EDT_INIT_FRAME)
        ));
        isr.run(&shared, throttle(500));
        assert_eq!(shared.newinput(), 500, "passes once armed");
        isr.run(&shared, throttle(0));
        assert_eq!(shared.newinput(), 0);
        assert!(!isr.edt_armed, "zero disarms");
        isr.run(&shared, throttle(500));
        assert_eq!(shared.newinput(), 0, "held again");
    }

    /// Servo calibration persists both thresholds, publishes both bytes and
    /// asks main to save (the harness copy did not publish).
    #[test]
    fn dispatch_servo_calibration_publishes_thresholds() {
        let shared = armed_shared();
        let mut isr = Isr::new(false);
        isr.run(
            &shared,
            TransferAction::ServoCalibrationDone {
                low_threshold: 51,
                high_threshold: 99,
            },
        );
        use crate::config::EepromConfig;
        assert_eq!(
            (
                isr.config.servo_low_threshold,
                isr.config.servo_high_threshold
            ),
            (51, 99)
        );
        let lo = core::mem::offset_of!(EepromConfig, servo_low_threshold) as u8;
        let hi = core::mem::offset_of!(EepromConfig, servo_high_threshold) as u8;
        assert_eq!(shared.pop_config_write(), Some((lo, 51)));
        assert_eq!(shared.pop_config_write(), Some((hi, 99)));
        assert!(shared.save_settings_flag());
    }

    /// DShot detection hands the port its output prescaler; no command hook.
    #[test]
    fn dispatch_detection_sets_output_prescaler() {
        let shared = crate::shared_state::SharedState::new::<Mhz64>();
        let mut isr = Isr::new(false);
        let (hooked, psc) = isr.run(
            &shared,
            TransferAction::InputDetected(DetectedProtocol::Dshot),
        );
        assert_eq!((hooked, psc), (0, Some(1)));
        assert!(shared.input_set() && shared.dshot());
    }
}
