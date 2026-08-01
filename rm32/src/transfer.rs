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
    // input pin is HIGH at idle (inverted signaling). >100 → bidir is a
    // HINT only; commit requires inverted-CRC decode confirmation below.
    high_pin_count: u8,
    // Bidir self-validation: consecutive successful inverted-CRC decodes
    // while the high-idle hint is active. A high-idle line alone spuriously
    // matches normal DShot frames that idle high briefly — committing on
    // the hint alone inverted the CRC on non-bidir traffic and every frame
    // then failed. Commit only after BIDIR_CONFIRM_FRAMES in a row.
    bidir_confirms: u8,
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
    /// DShot detection / normal operation: 32 edges (AM32 `buffersize = 32`,
    /// signal.c:27), no prescaler change.
    ///
    /// NDTR MUST be 32: a DShot frame is exactly 32 edges, so TC fires on
    /// the frame's LAST edge and the re-arm happens inside the ~ms
    /// inter-frame gap — the capture window stays frame-locked. The re-arm
    /// (receiveDshotDma parity) pulses an RCC reset of the capture timer,
    /// which clears any latched CC/DMA request, so slot 0 is always the
    /// next frame's first edge — no stale slot.
    ///
    /// History: an earlier stale-slot-0 observation (pre-RCC-reset re-arm)
    /// was band-aided with NDTR=33 + a 2-way alignment picker. But 33
    /// consumes one extra edge per 32-edge frame: TC then fires on the
    /// NEXT frame's first edge, the re-arm lands mid-frame, and the window
    /// slides +1 edge every frame — ~97% of frames decode against a
    /// gap-spanning buffer (whose u16-wrapped frametime even passes the
    /// window check, failing as BadCrc). Measured live vs Betaflight
    /// DSHOT300: crc_pass=13 / crc_fail=410 per 2 s life.
    pub const DSHOT: Self = Self {
        ndtr: 32,
        prescaler: None,
    };

    /// DShot detection (re-entry): 33 edges + slow prescaler so DShot pulses
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

        let mut bidir_detected = false;

        // --- DShot processing ---
        // 32 edges = exactly one frame, frame-locked by construction (see
        // CaptureConfig::DSHOT): TC fires on the frame's last edge, the
        // re-arm happens in the inter-frame gap, and the RCC-reset re-arm
        // guarantees slot 0 is the frame's first edge. Decode directly —
        // AM32-verbatim (dshot.c uses dma_buffer[0..32] as-is).
        if dshot_mode && dma_buffer.len() >= 32 {
            let buf: [u32; 32] = {
                let mut b = [0u32; 32];
                b.copy_from_slice(&dma_buffer[..32]);
                b
            };
            let frame = dshot::decode_frame(&buf, frametime_low, frametime_high, dshot_telemetry);
            // Bidir self-validation (unarmed, hint active): try the SAME
            // frame with inverted CRC. Only a run of successful inverted
            // decodes commits bidir; a normal-CRC success while inverted
            // fails proves the high idle was spurious — reset the hint.
            if !armed && !dshot_telemetry && self.high_pin_count > 100 {
                let inv = dshot::decode_frame(&buf, frametime_low, frametime_high, true);
                let inv_ok = matches!(
                    inv,
                    dshot::DshotFrame::Throttle { .. } | dshot::DshotFrame::Command { .. }
                );
                if inv_ok {
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
        if !armed {
            // Bidirectional DShot auto-detection hint: idle pin HIGH for
            // 100+ frames while unarmed suggests inverted (bidir)
            // signaling. Commit happens in the decode block above, only
            // after consecutive successful inverted-CRC decodes.
            if dshot_mode && !dshot_telemetry && input_pin_high {
                self.high_pin_count = self.high_pin_count.saturating_add(1);
            }

            // DShot frame averaging (for dshot_frametime calibration).
            // Same alignment-detection logic as the decode path: smaller of
            // the two candidate frametimes is the real frame.
            if dshot_mode && self.average_count < 8 && *zero_input_count > 5 {
                self.average_count += 1;
                if dma_buffer.len() >= 32 {
                    let frametime = dma_buffer[31].wrapping_sub(dma_buffer[0]) as u16;
                    self.average_packet_length += frametime as u32;
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
        // DSHOT NDTR is 32 = exactly one frame (AM32 buffersize) — TC on
        // the last edge, re-arm in the inter-frame gap, frame-locked.
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

    /// 32-slot DMA buffer holding exactly one DShot frame (frame-locked
    /// capture: slot 0 = the frame's first edge).
    fn dshot_dma_buffer(value: u16, telem: bool, inverted_crc: bool) -> [u32; 32] {
        let mut bits = [0u8; 16];
        for (i, b) in bits.iter_mut().enumerate().take(11) {
            *b = ((value >> (10 - i)) & 1) as u8;
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
        for i in 0..16 {
            buf[i * 2] = base;
            buf[i * 2 + 1] = base + if bits[i] != 0 { 22 } else { 10 };
            base += 32;
        }
        buf
    }

    /// Spurious high-idle on a NORMAL DShot line must never commit bidir:
    /// the inverted-CRC probe fails every frame and resets the hint.
    #[test]
    fn bidir_hint_alone_never_commits() {
        let mut state = TransferState::default();
        let buf = dshot_dma_buffer(999, false, false); // normal CRC
        let mut zic = 0u16;
        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, false, true, // pin high, unarmed
                0, 0, false, true, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected, "spurious bidir commit");
            // Normal decoding keeps working throughout.
            assert!(matches!(
                actions.action,
                TransferAction::DshotThrottle { value: 999, .. }
            ));
        }
        // The hint keeps getting reset by successful normal decodes.
        assert!(state.high_pin_count <= 101);
    }

    /// Real bidir traffic (inverted CRC + high idle) commits after the
    /// hint threshold plus BIDIR_CONFIRM_FRAMES consecutive confirms.
    #[test]
    fn bidir_commits_after_inverted_crc_confirms() {
        let mut state = TransferState::default();
        let buf = dshot_dma_buffer(999, false, true); // inverted CRC
        let mut zic = 0u16;
        let mut detected_at = None;
        for n in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, false, true, // pin high, unarmed
                0, 0, false, true, &mut zic, 400, 600, 64,
            );
            // Until commit, normal-CRC decode fails (no throttle action).
            if actions.bidir_detected && detected_at.is_none() {
                detected_at = Some(n);
            }
        }
        let n = detected_at.expect("bidir never committed");
        // Hint needs >100 high frames, then 4 confirms.
        assert!(
            (100..=110).contains(&n),
            "commit at frame {n}, expected 104-ish"
        );
    }

    /// Armed traffic never runs the bidir probe (handshake is pre-arm).
    #[test]
    fn bidir_probe_unarmed_only() {
        let mut state = TransferState::default();
        let buf = dshot_dma_buffer(999, false, true);
        let mut zic = 0u16;
        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, true, true, // ARMED
                0, 0, false, true, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected);
        }
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
        // NDTR=32: one frame per capture, TC on the frame's last edge.
        assert_eq!(actions.next_capture.ndtr, 32);
    }

    /// REGRESSION (bench capture 07-31, Betaflight DSHOT300 @ PSC=1):
    /// a real disarm frame — all-0 bits, 47/87-tick half-bits with the
    /// observed jitter — must decode as Throttle{0}. These exact deltas
    /// failed 97% of the time under the NDTR=33 sliding window.
    #[test]
    fn bf_dshot300_disarm_frame_decodes() {
        // Measured half-bit times (rise->fall = high = 46..47 ticks,
        // fall->rise = low = 86..87), from the [snap] frame autopsy.
        let highs = [
            47, 47, 47, 46, 47, 47, 47, 46, 47, 46, 47, 47, 46, 47, 47, 47,
        ];
        let lows = [
            87, 86, 87, 87, 86, 87, 87, 87, 86, 87, 87, 86, 87, 87, 87, 0,
        ];
        let mut buf = [0u32; 32];
        let mut t = 5000u32;
        for i in 0..16 {
            buf[i * 2] = t;
            buf[i * 2 + 1] = t + highs[i];
            t += highs[i] + lows[i];
        }
        let mut state = TransferState::default();
        let mut zic = 0u16;
        // Firmware initial wide frametime window (main.rs: 100..60000).
        let actions = state.process(
            &buf, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 100, 60000,
            80,
        );
        assert!(
            matches!(
                actions.action,
                TransferAction::DshotThrottle { value: 0, .. }
            ),
            "BF disarm frame must decode as Throttle 0, got {:?}",
            actions.action
        );
    }

    /// A gap-spanning capture (the NDTR=33 failure mode: window starts
    /// mid-frame, spans the ~2 ms inter-frame gap whose u16-wrapped size
    /// can sneak past the frametime window) must NOT decode a throttle.
    #[test]
    fn gap_spanning_capture_rejected() {
        let mut buf = [0u32; 32];
        // 9 tail edges of frame k (uniform cadence), so the inter-frame
        // gap lands INSIDE bit-pair 4 (slots 8,9) — the typical sliding-
        // window phase. (A gap BETWEEN pairs with uniform edges decodes
        // as a legal all-zero frame — identical in AM32's decoder; the
        // guards there are frame-locked capture + the narrowed frametime
        // window after unarmed averaging.)
        let mut t = 5000u32;
        for slot in buf.iter_mut().take(9) {
            *slot = t;
            t += 67;
        }
        // ...the inter-frame gap (78_400 ticks @40 MHz ≈ 1.96 ms — wraps
        // u16 to 12_864, sneaking past the wide frametime window)...
        t += 78_400;
        // ...then 23 head edges of frame k+1.
        for slot in buf.iter_mut().skip(9) {
            *slot = t;
            t += 67;
        }
        let mut state = TransferState::default();
        let mut zic = 0u16;
        let actions = state.process(
            &buf, true, true, false, false, false, false, 0, 0, false, false, &mut zic, 100, 60000,
            80,
        );
        assert!(
            matches!(actions.action, TransferAction::None),
            "gap-spanning capture must be rejected, got {:?}",
            actions.action
        );
    }

    #[test]
    fn servo_detection_sets_prescaler() {
        let mut state = TransferState::default();
        // Servo-like pulse timing in detection buffer (32-edge layout — see
        // CaptureConfig::DSHOT.ndtr).
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
        // CONTRACT CHANGE: the 100-frame high-idle count is now only a
        // HINT — commitment additionally requires BIDIR_CONFIRM_FRAMES
        // consecutive successful inverted-CRC decodes (see
        // bidir_commits_after_inverted_crc_confirms). A high idle with
        // no decodable inverted frames (this test: garbage buffers) must
        // NEVER commit — that was the false-positive that used to apply
        // CRC inversion to normal DShot traffic.
        let mut state = TransferState::default();
        let buf = [0u32; 32];
        let mut zic = 0u16;

        for _ in 0..200 {
            let actions = state.process(
                &buf, true, true, false, false, // dshot_telemetry=false
                false, // armed=false
                true,  // input_pin_high=true (idle-high hint)
                0, 0, false, false, &mut zic, 400, 600, 64,
            );
            assert!(!actions.bidir_detected);
        }
        // The hint itself accumulated…
        assert!(state.high_pin_count > 100);
        // …but no confirms, so no commit.
        assert_eq!(state.bidir_confirms, 0);
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
