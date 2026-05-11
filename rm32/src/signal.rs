//! Input signal detection and processing (DShot/Servo/auto-detect).

/// Signal detection result
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalType {
    None,
    Dshot600,
    Dshot300,
    ServoPwm,
}

/// Detect input type from DMA buffer pulse pattern.
pub fn detect_input(dma_buffer: &[u32], _cpu_mhz: u8) -> SignalType {
    let mut smallest = 20000u16;
    let mut average_pulse = 0u32;
    let mut last = dma_buffer[0] as u16;

    for sample in &dma_buffer[1..31] {
        // 16-bit wraparound: timer is 16-bit, stored as u32
        let diff = (*sample as u16).wrapping_sub(last);
        if diff > 0 {
            if diff < smallest {
                smallest = diff;
            }
            average_pulse += diff as u32;
        }
        last = *sample as u16;
    }
    average_pulse /= 32;

    // Check DShot600: smallest 1-4, average < 60
    if (1..4).contains(&smallest) && average_pulse < 60 {
        return SignalType::Dshot600;
    }
    // Check DShot300: smallest 4-8, average < 100
    if (4..=8).contains(&smallest) && average_pulse < 100 {
        return SignalType::Dshot300;
    }
    // Check Servo: smallest > 200
    if smallest > 200 && smallest < 20000 {
        return SignalType::ServoPwm;
    }

    SignalType::None
}

/// Compute servo input from pulse width.
/// Returns mapped throttle value (0-2047 for unidirectional).
/// Compute MultiShot input from DMA buffer.
/// MultiShot uses a single pulse width (243-1200µs → 0-2000 throttle).
pub fn compute_multishot(dma_buffer: &[u32]) -> Option<u16> {
    use crate::functions::map;
    if dma_buffer.len() < 2 {
        return None;
    }
    let pulse = dma_buffer[1].wrapping_sub(dma_buffer[0]);
    if pulse > 0 && pulse < 1500 {
        Some(map(pulse as i32, 243, 1200, 0, 2000) as u16)
    } else {
        None
    }
}

/// Compute servo input from pulse width.
/// Returns mapped throttle value (0-2047 for unidirectional).
pub fn compute_servo_unidirectional(
    pulse_width: u16,
    low_threshold: u16,
    high_threshold: u16,
) -> u16 {
    use crate::functions::map;
    let raw = map(
        pulse_width as i32,
        low_threshold as i32,
        high_threshold as i32,
        47,
        2047,
    );
    if raw <= 48 { 0 } else { raw as u16 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_dshot600() {
        let mut buf = [0u32; 32];
        for i in 0..32 {
            buf[i] = 100 + i as u32 * 3;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot600);
    }

    #[test]
    fn detect_servo() {
        let mut buf = [0u32; 32];
        for i in 0..32 {
            buf[i] = 100 + i as u32 * 1000;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::ServoPwm);
    }

    #[test]
    fn detect_out_of_range() {
        let buf = [0u32; 32]; // all zeros, no valid pulses
        assert_eq!(detect_input(&buf, 48), SignalType::None);
    }

    #[test]
    fn servo_unidirectional_mid() {
        let val = compute_servo_unidirectional(1500, 1100, 1900);
        assert!(val > 900 && val < 1200); // roughly mid-range
    }

    #[test]
    fn servo_unidirectional_below_threshold() {
        let val = compute_servo_unidirectional(1050, 1100, 1900);
        assert_eq!(val, 0);
    }

    #[test]
    fn servo_unidirectional_max() {
        let val = compute_servo_unidirectional(1900, 1100, 1900);
        assert_eq!(val, 2047);
    }

    #[test]
    fn detect_dshot300() {
        let mut buf = [0u32; 32];
        for i in 0..32 {
            buf[i] = 100 + i as u32 * 5;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot300);
    }

    #[test]
    fn detect_rejects_ambiguous() {
        let mut buf = [0u32; 32];
        for i in 0..32 {
            buf[i] = 100 + i as u32 * 12;
        } // smallest=12, >8
        // average = 12*31/32 ~ 11, not matching servo (>200) either
        assert_eq!(detect_input(&buf, 48), SignalType::None);
    }

    #[test]
    fn servo_mid_range_bidir_below_neutral() {
        // Below neutral: maps 0-1000
        let val = compute_servo_unidirectional(1300, 1100, 1900);
        assert!(val > 0 && val < 2047);
    }

    #[test]
    fn servo_at_threshold_is_zero() {
        // Exactly at low threshold: map returns out_min=47, then 47 <= 48 -> 0
        let val = compute_servo_unidirectional(1100, 1100, 1900);
        assert_eq!(val, 0);
    }

    // --- Timer wraparound tests ---
    // DMA captures 16-bit timer values stored as u32. The timer wraps at 65535.
    // detect_input must handle a wrap within the 32-edge capture window.

    #[test]
    fn detect_dshot600_wraparound() {
        // DShot600 with timer wrapping mid-capture
        let mut buf = [0u32; 32];
        let start = 65520u32; // near u16 max
        for i in 0..32 {
            buf[i] = (start + i as u32 * 3) & 0xFFFF; // wraps at 65536
        }
        // e.g. 65520, 65523, 65526, 65529, 65532, 65535, 2, 5, 8, ...
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot600);
    }

    #[test]
    fn detect_dshot300_wraparound() {
        let mut buf = [0u32; 32];
        let start = 65500u32;
        for i in 0..32 {
            buf[i] = (start + i as u32 * 5) & 0xFFFF;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot300);
    }

    #[test]
    fn detect_servo_wraparound() {
        // Servo with timer wrapping: large intervals that cross the 16-bit boundary
        let mut buf = [0u32; 32];
        let start = 60000u32;
        for i in 0..32 {
            buf[i] = (start + i as u32 * 1000) & 0xFFFF;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::ServoPwm);
    }
}
