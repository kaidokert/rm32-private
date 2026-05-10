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
}

/// How many DMA edges to capture on the next cycle.
///
/// Mirrors AM32's `buffersize` global — the decoder tells the HAL how
/// to arm the next DMA capture. This is the feedback loop that was
/// lost in the original Rust port (see BRINGUP_NOTES_L431.md).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureSize {
    /// DShot: 32 edges (16 bit-pairs)
    Dshot,
    /// Servo aligned: 2 edges [rise, fall] — ready to decode
    Servo,
    /// Servo misaligned: 3 edges [fall, rise, fall] — re-align next cycle
    ServoRealign,
}

impl CaptureSize {
    /// DMA NDTR value for this capture size.
    pub fn ndtr(self) -> u32 {
        match self {
            CaptureSize::Dshot => 32,
            CaptureSize::Servo => 2,
            CaptureSize::ServoRealign => 3,
        }
    }
}

/// Actions the caller (ISR) should take after transfer complete.
pub struct TransferActions {
    /// Primary action
    pub action: TransferAction,
    /// How many edges to capture next cycle (HAL re-arm directive)
    pub next_capture: CaptureSize,
    /// DShot frame timing update (from unarmed averaging)
    pub frametime: Option<(u16, u16)>,
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
    ) -> TransferActions {
        let mut action = TransferAction::None;
        let mut frametime = None;

        // --- Input detection ---
        if !input_set {
            let sig = signal::detect_input(dma_buffer, 48);
            action = match sig {
                signal::SignalType::Dshot600 | signal::SignalType::Dshot300 => {
                    TransferAction::InputDetected(DetectedProtocol::Dshot)
                }
                signal::SignalType::ServoPwm => {
                    TransferAction::InputDetected(DetectedProtocol::Servo)
                }
                _ => TransferAction::None,
            };
            return TransferActions {
                action,
                next_capture: CaptureSize::Dshot, // detection uses 32-edge captures
                frametime,
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
                    ServoResult::Calibrating
                    | ServoResult::CalibrationHighDone
                    | ServoResult::CalibrationDone { .. } => TransferAction::ServoCalibrating,
                };
            }
        }

        // --- Unarmed housekeeping ---
        if !armed {
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

        // Compute next capture size — mirrors AM32's buffersize global.
        // Servo + pin_high means [fall,rise] alignment: request 3 edges to realign.
        let next_capture = if servo_mode && input_pin_high {
            CaptureSize::ServoRealign
        } else if servo_mode {
            CaptureSize::Servo
        } else {
            CaptureSize::Dshot
        };

        TransferActions {
            action,
            next_capture,
            frametime,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_size_ndtr_values() {
        assert_eq!(CaptureSize::Dshot.ndtr(), 32);
        assert_eq!(CaptureSize::Servo.ndtr(), 2);
        assert_eq!(CaptureSize::ServoRealign.ndtr(), 3);
    }

    #[test]
    fn servo_pin_high_requests_realign() {
        let mut state = TransferState::default();
        let buf = [0u32; 2];
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, false, true, false, false,
            true, // input_pin_high — the key condition
            0, 0, false, false, &mut zic, 400, 600,
        );
        assert_eq!(
            actions.next_capture,
            CaptureSize::ServoRealign,
            "servo + pin_high should request 3-edge realignment"
        );
    }

    #[test]
    fn servo_pin_low_requests_normal() {
        let mut state = TransferState::default();
        let buf = [1000u32, 2500];
        state.servo.set_calibration(1100, 1900, 1500, 100);
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, false, true, false, false, false, // input_pin_low — aligned
            0, 0, false, false, &mut zic, 400, 600,
        );
        assert_eq!(
            actions.next_capture,
            CaptureSize::Servo,
            "servo + pin_low should request 2-edge capture"
        );
    }

    #[test]
    fn dshot_mode_requests_32() {
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 400, 600,
        );
        assert_eq!(actions.next_capture, CaptureSize::Dshot);
    }
}
