//! Commanded-waveform phase continuity only; no rotor sensing or gate authority.
use crate::{sine_table::SINE_LUT, sixstep};
pub const STEPS: [u8; 256] = {
    let mut result = [0; 256];
    let mut i = 0;
    while i < 256 {
        result[i] = sixstep::step_for_values([
            SINE_LUT[i],
            SINE_LUT[(i + 85) & 255],
            SINE_LUT[(i + 170) & 255],
        ]);
        i += 1;
    }
    result
};
const NEXT: [u8; 256] = {
    let mut result = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut distance = 1;
        while distance < 256 && STEPS[(i + distance) & 255] == STEPS[i] {
            distance += 1;
        }
        result[i] = distance as u8;
        i += 1;
    }
    result
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Boundary {
    pub step: u8,
    pub next_step: u8,
    pub delay_us: u16,
}
impl Boundary {
    /// Too-short initial partial sectors are skipped with gates disabled.
    /// Recompute from the real clock after waiting; never extend this sector.
    pub fn initial_wait(self) -> u16 {
        if self.delay_us < 100 {
            self.delay_us
        } else {
            0
        }
    }
}
/// `theta` must be extrapolated from the captured startup phase to the actual
/// scheduling instant. Caller accounts for calculation/register-write latency.
/// Pass the same Q0.32 rate as the sine generator,not a rounded sector period.
/// Recompute at every boundary to avoid accumulated833us-vs833.333us drift.
pub fn next(theta: u32, rate: u32) -> Option<Boundary> {
    if !(214_748..=1_288_490).contains(&rate) {
        return None;
    } // tested50..300eHz
    let index = (theta >> 24) as usize;
    let distance = NEXT[index] as usize;
    if distance == 0 {
        return None;
    }
    let next_index = (index + distance) & 255;
    let delta = ((next_index as u32) << 24).wrapping_sub(theta);
    let delay = (delta - 1) / rate + 1; // ceil,without overflow or64-bit arithmetic
    if delay == 0 || delay > u16::MAX as u32 {
        return None;
    }
    Some(Boundary {
        step: STEPS[index],
        next_step: STEPS[next_index],
        delay_us: delay as u16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::phase_rate;
    #[test]
    fn short_initial_sector_wait_preserves_phase_and_full_next_window() {
        for hz in [50, 100, 167, 200, 250, 300] {
            let rate = phase_rate(hz * 100);
            let mut skipped = 0;
            for index in 0..256u32 {
                for fraction in [0, 1, 0x7fffff, 0xffffff] {
                    let theta = (index << 24) | fraction;
                    let b = next(theta, rate).unwrap();
                    let wait = b.initial_wait();
                    assert!(wait < 100);
                    if wait == 0 {
                        assert!(b.delay_us >= 100);
                        continue;
                    }
                    skipped += 1;
                    // Include bounded dispatch/calculation overshoot,not an exact
                    // synthetic counter boundary that the MCU could never observe.
                    for overshoot in [0, 1, 10, 20] {
                        let actual = theta.wrapping_add(rate * (wait as u32 + overshoot));
                        let admitted = next(actual, rate).unwrap();
                        assert_eq!(admitted.step, b.next_step);
                        assert!(admitted.delay_us >= 100);
                    }
                }
            }
            assert!(skipped > 0);
        }
    }
    #[test]
    fn every_lookup_position_and_fraction_reaches_exact_next_sector() {
        for hz in [50, 100, 167, 200, 250, 300] {
            let rate = phase_rate(hz * 100);
            for index in 0..256u32 {
                for fraction in [0, 1, 0x7fffff, 0xffffff] {
                    let theta = (index << 24) | fraction;
                    let b = next(theta, rate).unwrap();
                    assert_eq!(b.step, STEPS[index as usize]);
                    assert_eq!(b.next_step, b.step % 6 + 1);
                    assert_eq!(
                        STEPS[(theta.wrapping_add(rate * (b.delay_us as u32 - 1)) >> 24) as usize],
                        b.step
                    );
                    assert_eq!(
                        STEPS[(theta.wrapping_add(rate * b.delay_us as u32) >> 24) as usize],
                        b.next_step
                    );
                }
            }
        }
    }
    #[test]
    fn twenty_ms_schedule_never_restarts_mid_sector_or_accumulates_rounding() {
        let rate = phase_rate(20000);
        for initial in [0, 0x12345678, u32::MAX] {
            let mut elapsed = 0u32;
            for _ in 0..24 {
                let theta = initial.wrapping_add(rate.wrapping_mul(elapsed));
                let b = next(theta, rate).unwrap();
                elapsed += b.delay_us as u32;
                assert_eq!(
                    STEPS[(initial.wrapping_add(rate.wrapping_mul(elapsed)) >> 24) as usize],
                    b.next_step
                );
            }
            assert!((19_000..=20_001).contains(&elapsed));
        }
        assert!(next(0, 0).is_none());
        assert!(next(0, u32::MAX).is_none());
    }
}
