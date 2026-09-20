//! Shared shell target validation and bidirectional ramp arithmetic.
pub const MAX_EHZ: u32 = 250;
/// Replay the successful host trajectory in the 1kHz foreground envelope.
/// The caller's staircase catch is50Hz; target stays50Hz until tick3200.
/// Fifteen ascending steps span1200ticks, with no runtime division.
pub const fn staircase_target(tick: u32) -> u32 {
    let mut index = 0u32;
    while index < 15 {
        // floor(index*1200/14), matching host spacing to within1tick.
        let deadline = 3200
            + match index {
                0 => 0,
                1 => 85,
                2 => 171,
                3 => 257,
                4 => 342,
                5 => 428,
                6 => 514,
                7 => 600,
                8 => 685,
                9 => 771,
                10 => 857,
                11 => 942,
                12 => 1028,
                13 => 1114,
                _ => 1200,
            };
        if tick < deadline {
            return 50 + 10 * index;
        }
        index += 1;
    }
    200
}
const _: () = {
    let mut index = 0;
    while index < 15 {
        let tick = 3200 + index * 1200 / 14;
        assert!(staircase_target(tick) == 60 + 10 * index);
        assert!(staircase_target(tick - 1) == 50 + 10 * index);
        index += 1;
    }
};
/// Storage policy only; ADC/guards must run before this decision every tick.
pub fn retain_sample(tick: u32, stride: u32, adc_fault: bool) -> bool {
    assert!((1..=100).contains(&stride));
    adc_fault || tick % stride == 0
}
pub fn phase_shift(theta: u32, degrees: i32) -> u32 {
    theta.wrapping_add(((degrees as i64) * (1i64 << 32) / 360) as u32)
}
/// Q0.32 electrical revolutions per microsecond, calculated at envelope rate.
pub fn phase_rate(freq_chz: u32) -> u32 {
    ((freq_chz as u64) * (1u64 << 32) / 100_000_000) as u32
}
pub fn target(bytes: &[u8]) -> Option<u32> {
    let mut value = 0u32;
    let mut any = false;
    for &b in bytes {
        if b == b' ' {
            continue;
        }
        if !b.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((b - b'0') as u32)?;
        any = true;
    }
    if any && (1..=MAX_EHZ).contains(&value) {
        Some(value)
    } else {
        None
    }
}
pub fn ramp(start: u32, end: u32, elapsed: u32, duration: u32) -> u32 {
    let t = elapsed.min(duration);
    if end >= start {
        start + (end - start) * t / duration
    } else {
        start - (start - end) * t / duration
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_storage_preserves_faults_and_startup_span() {
        let mut retained = 0;
        for tick in 1..=4700 {
            if retain_sample(tick, 19, false) {
                retained += 1;
            }
            assert!(retain_sample(tick, 19, true));
            assert!(retain_sample(tick, 1, false));
        }
        assert_eq!(retained, 247);
        assert!(retained < 256);
        assert!(!retain_sample(2000, 19, false));
        assert!(retain_sample(2000, 19, true));
    }
    #[test]
    fn phase_offsets_wrap_and_preserve_zero() {
        assert_eq!(phase_shift(123, 0), 123);
        assert_eq!(phase_shift(0, 90), 1 << 30);
        assert_eq!(phase_shift(0, -90), 3 << 30);
        assert_eq!(phase_shift(phase_shift(u32::MAX, 30), -30), u32::MAX);
    }
    #[test]
    fn targets() {
        for (s, n) in [("50", 50), (" 100", 100), ("200", 200), ("250", 250)] {
            assert_eq!(target(s.as_bytes()), Some(n));
        }
        for s in ["", "0", "251", "-20", "200x", "99999999999999999999"] {
            assert_eq!(target(s.as_bytes()), None);
        }
    }
    #[test]
    fn up_and_down() {
        assert_eq!(ramp(10000, 20000, 1000, 2000), 15000);
        assert_eq!(ramp(10000, 5000, 1000, 2000), 7500);
        assert_eq!(ramp(10000, 25000, 2000, 2000), 25000);
    }
    #[test]
    fn elapsed_phase_is_cadence_independent() {
        let rate = phase_rate(20000);
        let a = rate.wrapping_mul(100).wrapping_mul(50);
        let b = rate.wrapping_mul(250).wrapping_mul(20);
        assert_eq!(a, b);
        // Truncated Q32 rate loses <5000 counts per 5ms revolution.
        assert!(a > u32::MAX - 5000);
    }
}
