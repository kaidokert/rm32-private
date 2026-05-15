//! Input signal detection and processing (DShot/Servo/auto-detect).

use crate::functions::map;

/// Signal detection result
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SignalType {
    None,
    Dshot600,
    Dshot300,
    Dshot150,
    ServoPwm,
}

/// Detect input type from DMA buffer pulse pattern.
pub fn detect_input(dma_buffer: &[u32], _cpu_mhz: u8) -> SignalType {
    let mut smallest = 20000u16;
    let mut average_pulse = 0u32;

    // Skip the very first slot — in TIM/DMA continuous-capture mode buf[0]
    // is racily stale from the previous frame's tail (typical observed value
    // ~11385 even on a freshly-armed DMA). The `buf[0]→buf[1]` delta is an
    // inter-frame gap, not a bit pulse. Start from buf[1].
    //
    // Stop at buf[31] (don't include buf[32]) — buf[32] is the FIRST edge of
    // the NEXT frame in the 33-edge layout (or stale tail in the alternate
    // alignment), so the delta into buf[32] is the inter-frame gap and would
    // pollute the average.
    //
    // Subtract at the timer's natural width (u16) so a TIM wrap from
    // 65535→0 yields the real elapsed ticks rather than a billion-sized u32.
    let mut last = dma_buffer[1] as u16;
    let mut count: u32 = 0;
    for sample in &dma_buffer[2..32] {
        let s = *sample as u16;
        let diff = s.wrapping_sub(last);
        if diff > 0 {
            if diff < smallest {
                smallest = diff;
            }
            average_pulse += diff as u32;
            count += 1;
        }
        last = s;
    }
    if count > 0 {
        average_pulse /= count;
    }

    // Check DShot600: smallest 1-4, average < 60
    if (1..4).contains(&smallest) && average_pulse < 60 {
        return SignalType::Dshot600;
    }
    // Check DShot300: smallest 4-8, average < 100
    if (4..=8).contains(&smallest) && average_pulse < 100 {
        return SignalType::Dshot300;
    }
    // Check DShot150: smallest 9-16, average < 200
    if (9..=16).contains(&smallest) && average_pulse < 200 {
        return SignalType::Dshot150;
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
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 3;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot600);
    }

    #[test]
    fn detect_servo() {
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 1000;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::ServoPwm);
    }

    #[test]
    fn detect_out_of_range() {
        let buf = [0u32; 33]; // all zeros, no valid pulses
        assert_eq!(detect_input(&buf, 48), SignalType::None);
    }

    /// TIM15 is a 16-bit counter. After ~11 ms at 5.7 MHz it wraps from
    /// 65535 back to 0, so consecutive captured edges can straddle the wrap.
    /// Regression: before the u16-width fix, `wrapping_sub` at u32 width
    /// produced a billion-sized delta on the wrap point, blowing up the
    /// average and intermittently making valid DShot frames fail detection
    /// (and occasionally pass by accident via u32 overflow wrap).
    #[test]
    fn detect_dshot300_across_timer_wrap() {
        let mut buf = [0u32; 33];
        // Start near the top of u16 so the first few deltas wrap to zero.
        let mut t: u32 = 65000;
        for slot in buf.iter_mut() {
            *slot = t & 0xFFFF;
            // Mix of "0" pulses (5 ticks) and "1" pulses (15 ticks) — fits
            // DShot300's "smallest 4-8, avg < 100" bucket.
            t += if (t & 1) == 0 { 5 } else { 15 };
        }
        assert_eq!(detect_input(&buf, 80), SignalType::Dshot300);
    }

    /// Same regression check for DShot600 — narrower bit pulses (1-4 ticks).
    #[test]
    fn detect_dshot600_across_timer_wrap() {
        let mut buf = [0u32; 33];
        let mut t: u32 = 65500;
        for slot in buf.iter_mut() {
            *slot = t & 0xFFFF;
            t += if (t & 1) == 0 { 2 } else { 6 };
        }
        assert_eq!(detect_input(&buf, 80), SignalType::Dshot600);
    }

    /// Regression: hardware buffer captured on L431 with TIM15 in DMA
    /// circular mode shows `buf[0]` lingering at the previous burst's last
    /// edge (~11385) while `buf[1..]` is filled with this frame's bit edges.
    /// The `buf[0]→buf[1]` delta is therefore an inter-frame gap, not a bit
    /// pulse, and including it in the average blew detection up to a value
    /// that always failed `< 100`. The fix is to start the loop from
    /// `buf[1]` and only count valid bit deltas.
    ///
    /// Buffer values copied from a real failing exti log on bench:
    /// `buf[0..4]=11385 11092 11099 11111` with subsequent bit deltas of
    /// 6-7 ticks ("0") and 12-13 ticks ("1") at PSC=13 / 5.71 MHz tick,
    /// which is DShot300 timing.
    #[test]
    fn detect_dshot300_with_stale_buf0() {
        let mut buf = [0u32; 33];
        // Position 0: stale value from previous burst.
        buf[0] = 11385;
        // Position 1: this frame's first edge.
        let mut t: u32 = 11092;
        buf[1] = t;
        // Fill remaining 31 edges with DShot300 bit timings.
        for slot in &mut buf[2..] {
            t += if (t & 1) == 0 { 7 } else { 13 };
            *slot = t & 0xFFFF;
        }
        assert_eq!(detect_input(&buf, 80), SignalType::Dshot300);
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
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 5;
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot300);
    }

    #[test]
    fn detect_dshot150() {
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 10; // smallest=10, average~10
        }
        assert_eq!(detect_input(&buf, 48), SignalType::Dshot150);
    }

    #[test]
    fn detect_rejects_ambiguous() {
        let mut buf = [0u32; 33];
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = 100 + i as u32 * 20;
        } // smallest=20, >16 (past DShot150), <200 (not servo)
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
}
