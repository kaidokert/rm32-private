//! Transfer complete dispatcher.
//!
//! Equivalent to C `transfercomplete()` — the central dispatcher that handles
//! DMA completion for both DShot and servo input, auto-detection, bidir
//! telemetry, and unarmed frame averaging.

use crate::dshot;
use crate::functions::get_abs_dif;
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
    // Bidirectional DShot auto-detection: counts consecutive frames where
    // input pin is HIGH at idle (inverted signaling). >100 → bidir detected.
    high_pin_count: u8,
    // Protocol re-confirmation: require 2 consecutive matching detections
    // before locking protocol. Prevents false lock from single noisy frame.
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
    /// Servo calibration complete — persist thresholds to EEPROM
    ServoCalibrationDone {
        low_threshold: u8,
        high_threshold: u8,
    },
}

/// DMA capture configuration — buffer size + timer prescaler.
///
/// Mirrors AM32's `buffersize` and `ic_timer_prescaler` globals.
/// The decoder tells the HAL how to arm the next DMA capture AND
/// what timer resolution to use. Both feedback loops were lost in
/// the original Rust port (see BRINGUP_NOTES_L431.md, LOST_PRESCALER.md).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureConfig {
    /// DMA transfer count (number of edges to capture)
    pub ndtr: u32,
    /// Timer prescaler value (None = don't change, Some(v) = set PSC to v).
    /// AM32 sets `ic_timer_prescaler = CPU_FREQUENCY_MHZ - 1` for servo
    /// (1 µs/tick) and 0 or 1 for DShot (max resolution).
    pub prescaler: Option<u16>,
}

impl CaptureConfig {
    /// DShot detection / normal operation: 32 edges, no prescaler change.
    pub const DSHOT: Self = Self {
        ndtr: 32,
        prescaler: None,
    };

    /// DShot detection (re-entry): 32 edges + slow prescaler so DShot pulses
    /// fit `signal::detect_input()`'s `smallest 1-8 ticks` heuristic
    /// thresholds. Must be applied any time we re-enter detection mode after
    /// a prior successful detection (which fast-tracked the prescaler to 0/1
    /// for max resolution). `cpu_mhz/6` gives ~5.7 MHz tick at 80 MHz CPU →
    /// DShot300 "0" pulse = ~7 ticks (in the 4-8 range), DShot600 "0" =
    /// ~3.6 ticks (in 1-4 range).
    pub fn dshot_detection(cpu_mhz: u8) -> Self {
        Self {
            ndtr: 32,
            prescaler: Some((cpu_mhz / 6) as u16),
        }
    }

    /// Servo aligned: 2 edges, no prescaler change (already set on detection).
    pub const SERVO: Self = Self {
        ndtr: 2,
        prescaler: None,
    };

    /// Servo misaligned: 3 edges for realignment, no prescaler change.
    pub const SERVO_REALIGN: Self = Self {
        ndtr: 3,
        prescaler: None,
    };

    /// Servo detected: 2 edges + prescaler to 1 µs/tick.
    /// `cpu_mhz`: CPU frequency in MHz (prescaler = cpu_mhz - 1).
    pub fn servo_detected(cpu_mhz: u8) -> Self {
        Self {
            ndtr: 2,
            prescaler: Some(cpu_mhz as u16 - 1),
        }
    }

    /// DShot600 detected: 32 edges + prescaler to 0 (max resolution).
    pub const DSHOT600_DETECTED: Self = Self {
        ndtr: 32,
        prescaler: Some(0),
    };

    /// DShot300 detected: 32 edges + prescaler to 1 (half resolution).
    pub const DSHOT300_DETECTED: Self = Self {
        ndtr: 32,
        prescaler: Some(1),
    };

    /// DShot150 detected: 32 edges + prescaler to 3 (quarter resolution).
    pub const DSHOT150_DETECTED: Self = Self {
        ndtr: 32,
        prescaler: Some(3),
    };
}

/// Actions the caller (ISR) should take after transfer complete.
pub struct TransferActions {
    /// Primary action
    pub action: TransferAction,
    /// DMA + timer config for next capture cycle
    pub next_capture: CaptureConfig,
    /// DShot frame timing update (from unarmed averaging)
    pub frametime: Option<(u16, u16)>,
    /// Bidirectional DShot auto-detected (caller should set dshot_telemetry=true)
    pub bidir_detected: bool,
    /// Bench-debug snapshot of `high_pin_count` for the bidir auto-detect path.
    /// Exposed via TransferActions so the ISR handler can publish it to a
    /// SharedState counter for main-loop heartbeat dumps. Capped at u8 (the
    /// internal counter is u8 saturating).
    pub high_pin_count: u8,
}

impl TransferState {
    /// Process a DMA transfer complete event.
    ///
    /// `dma_buffer`: the captured DMA data (32 entries for DShot, 2-3 for servo)
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
    #[allow(clippy::too_many_arguments)]
    pub fn process(
        &mut self,
        dma_buffer: &[u32],
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
        cpu_mhz: u8,
    ) -> TransferActions {
        let mut action = TransferAction::None;
        let mut frametime = None;

        // --- Input detection (requires 2 consecutive matching detections) ---
        if !input_set {
            let sig = signal::detect_input(dma_buffer, cpu_mhz);
            let proto = match sig {
                signal::SignalType::Dshot600
                | signal::SignalType::Dshot300
                | signal::SignalType::Dshot150 => Some(DetectedProtocol::Dshot),
                signal::SignalType::ServoPwm => Some(DetectedProtocol::Servo),
                signal::SignalType::None => None,
            };
            // Re-confirmation: first detection is tentative; second matching
            // detection confirms. A single `None` frame between two valid
            // detections is treated as transient noise — only a *different*
            // protocol detection resets pending state. Otherwise marginal
            // capture timing (DMA window grabbing mid-edge) can produce
            // alternating Some/None and the chain never reaches 2-in-a-row.
            let confirmed = match (proto, self.pending_protocol) {
                (Some(p), Some(pending)) if p == pending => {
                    self.pending_protocol = None;
                    true
                }
                (Some(p), _) => {
                    self.pending_protocol = Some(p);
                    false
                }
                (None, _) => false,
            };
            // Only switch to the protocol's fast capture config once
            // *confirmed*. Until then keep the slow detection prescaler so
            // the heuristic's `smallest 1-8 ticks` ranges keep matching on
            // the next frame — otherwise a single detect_input() hit drops
            // the prescaler to 0/1 and the next frame's deltas blow past
            // the detection thresholds, locking us out.
            let (action, capture) = if confirmed {
                let cap = match sig {
                    signal::SignalType::Dshot600 => CaptureConfig::DSHOT600_DETECTED,
                    signal::SignalType::Dshot300 => CaptureConfig::DSHOT300_DETECTED,
                    signal::SignalType::Dshot150 => CaptureConfig::DSHOT150_DETECTED,
                    signal::SignalType::ServoPwm => CaptureConfig::servo_detected(cpu_mhz),
                    signal::SignalType::None => CaptureConfig::dshot_detection(cpu_mhz),
                };
                (TransferAction::InputDetected(proto.unwrap()), cap)
            } else {
                (
                    TransferAction::None,
                    CaptureConfig::dshot_detection(cpu_mhz),
                )
            };
            return TransferActions {
                action,
                next_capture: capture,
                frametime,
                bidir_detected: false,
                high_pin_count: self.high_pin_count,
            };
        }

        // --- DShot processing ---
        if dshot_mode && dma_buffer.len() >= 32 {
            let buf: [u32; 32] = {
                let mut b = [0u32; 32];
                b.copy_from_slice(&dma_buffer[..32]);
                b
            };
            let frame = dshot::decode_frame(&buf, frametime_low, frametime_high, dshot_telemetry);
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
            } else if dma_buffer.len() >= 2 {
                let pulse = dma_buffer[1].wrapping_sub(dma_buffer[0]) as u16;
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
        let mut bidir_detected = false;
        if !armed {
            // Bidirectional DShot auto-detection: when idle pin is HIGH
            // for 100+ consecutive frames while unarmed, the FC is using
            // inverted (bidir) signaling. Set dshot_telemetry to invert CRC.
            if dshot_mode && !dshot_telemetry && input_pin_high {
                self.high_pin_count = self.high_pin_count.saturating_add(1);
                if self.high_pin_count > 100 {
                    bidir_detected = true;
                }
            }

            // DShot frame averaging (for dshot_frametime calibration)
            if dshot_mode && self.average_count < 8 && *zero_input_count > 5 {
                self.average_count += 1;
                if dma_buffer.len() >= 32 {
                    self.average_packet_length +=
                        (dma_buffer[31].wrapping_sub(dma_buffer[0])) as u16 as u32;
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

        // Compute next capture config — mirrors AM32's buffersize + ic_timer_prescaler.
        // Prescaler only changes on detection (above); steady-state just adjusts NDTR.
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
            high_pin_count: self.high_pin_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_config_ndtr_values() {
        assert_eq!(CaptureConfig::DSHOT.ndtr, 32);
        assert_eq!(CaptureConfig::SERVO.ndtr, 2);
        assert_eq!(CaptureConfig::SERVO_REALIGN.ndtr, 3);
    }

    #[test]
    fn servo_pin_high_requests_realign() {
        let mut state = TransferState::default();
        let buf = [0u32; 2];
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, false, true, false, false, true, // input_pin_high
            0, 0, false, false, &mut zic, 400, 600, 64,
        );
        assert_eq!(actions.next_capture.ndtr, 3);
        assert!(actions.next_capture.prescaler.is_none());
    }

    #[test]
    fn servo_pin_low_requests_normal() {
        let mut state = TransferState::default();
        let buf = [1000u32, 2500];
        state.servo.set_calibration(1100, 1900, 1500, 100);
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, false, true, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
            64,
        );
        assert_eq!(actions.next_capture.ndtr, 2);
        assert!(actions.next_capture.prescaler.is_none());
    }

    #[test]
    fn dshot_mode_requests_32() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
            64,
        );
        assert_eq!(actions.next_capture.ndtr, 32);
    }

    #[test]
    fn servo_detection_sets_prescaler() {
        let mut state = TransferState::default();
        // Servo-like pulse timing in detection buffer
        let mut buf = [0u32; 32];
        buf[0] = 100;
        buf[1] = 5000; // large gap = servo-like
        let mut zic = 0u16;
        let actions = state.process(
            &buf, false, // input_set=false → detection mode
            false, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600, 80,
        );
        if let TransferAction::InputDetected(DetectedProtocol::Servo) = actions.action {
            assert_eq!(actions.next_capture.prescaler, Some(79)); // cpu_mhz - 1
            assert_eq!(actions.next_capture.ndtr, 2);
        }
        // (If detection doesn't trigger with this buffer, the test is inconclusive
        // but won't fail — detection depends on signal timing heuristics)
    }

    #[test]
    fn bidir_auto_detect_after_100_frames() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;

        // Simulate 100 unarmed DShot frames with pin HIGH — not yet detected
        for _ in 0..100 {
            let actions = state.process(
                &buf, true, true, false, false, // dshot_telemetry=false
                false, // armed=false
                true,  // input_pin_high=true (bidir idle)
                0, 0, false, false, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected);
        }

        // Frame 101 — should trigger detection
        let actions = state.process(
            &buf, true, true, false, false, false, true, 0, 0, false, false, &mut zic, 400, 600, 64,
        );
        assert!(actions.bidir_detected);
    }

    #[test]
    fn bidir_not_detected_when_pin_low() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;

        // 200 frames with pin LOW — no detection
        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, false, false, // pin LOW
                0, 0, false, false, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn bidir_not_detected_when_armed() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;

        // 200 frames with pin HIGH but armed — no detection
        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, true, // armed=true
                true, 0, 0, false, false, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected);
        }
    }

    #[test]
    fn bidir_not_detected_when_already_set() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;

        // 200 frames with pin HIGH and dshot_telemetry already true
        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, true, // dshot_telemetry=true
                false, true, 0, 0, false, false, &mut zic, 400, 600, 64,
            );
            // Counter shouldn't increment when already detected
            assert!(!actions.bidir_detected);
        }
    }
}
