//! The driven stage: six-step sectors that continue the sine's phase.
//!
//! **Why this exists (LAB_NOTEBOOK E056-E058).** The qualified image never
//! closes its loop from a six-step open loop. It drags the rotor up to speed
//! with a *sine*, then switches to a *driven* six-step whose sector boundaries
//! are computed from the sine's own phase accumulator, so the vector keeps
//! turning at exactly the sine's rate and angle. While that forced drive runs,
//! the ordinary comparator ISR observes real crossings, and the seed for the
//! closed loop comes from them (`binz/examples/support/driven_run.rs`,
//! `driven_irq_live.rs`). firmware50 handed over from a six-step open loop
//! that loses the rotor near ~110 eHz, and its raw-edge acquisition validated
//! the drive's own transients (seed = schedule exactly, E057).
//!
//! Transcribed from `binz/examples/support/phase_schedule.rs` (boundaries),
//! `sixstep.rs::step_for_values` (sector for a vector) and `campaign.rs`
//! (`phase_shift`, `phase_rate`). The sine phases here are **logical** A/B/C,
//! the same order `sine::compares` returns, and the resulting step is a logical
//! step: the caller applies the wiring map, exactly as for the closed loop.
//!
//! The boundary computation divides (`(delta - 1) / rate`), as the reference's
//! does. It runs in the foreground at each driven sector, never in an ISR root,
//! so the fail-closed audit's no-soft-division rule is not engaged.

use crate::commutation::Step;
use crate::sine::{PHASE_STEP, SINE_LUT};

/// The six-step sector whose source is the highest sine phase and whose sink
/// is the lowest, as the reference picks it. `0` for a degenerate vector.
///
/// Ties resolve as the reference's do: `>=` for the source, `<` for the sink,
/// scanning A, B, C.
#[must_use]
pub const fn step_for_values(values: [u8; 3]) -> u8 {
    let mut source = 0;
    let mut sink = 0;
    let mut i = 1;
    while i < 3 {
        if values[i] >= values[source] {
            source = i;
        }
        if values[i] < values[sink] {
            sink = i;
        }
        i += 1;
    }
    match (source, sink) {
        (0, 1) => 1,
        (2, 1) => 2,
        (2, 0) => 3,
        (1, 0) => 4,
        (1, 2) => 5,
        (0, 2) => 6,
        _ => 0,
    }
}

/// Sector for each of the 256 phase-accumulator positions.
pub const STEPS: [u8; 256] = {
    let mut out = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let p = PHASE_STEP as usize;
        out[i] = step_for_values([SINE_LUT[i], SINE_LUT[(i + p) & 255], SINE_LUT[(i + 2 * p) & 255]]);
        i += 1;
    }
    out
};

/// Distance, in accumulator positions, to the next change of sector.
const NEXT: [u8; 256] = {
    let mut out = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        let mut d = 1;
        while d < 256 && STEPS[(i + d) & 255] == STEPS[i] {
            d += 1;
        }
        out[i] = d as u8;
        i += 1;
    }
    out
};

/// The sector in force now, the one after it, and how long until it changes.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Boundary {
    pub step: Step,
    pub next_step: Step,
    pub delay_us: u16,
}

impl Boundary {
    /// A partial first sector shorter than 100 µs is skipped with the bridge
    /// off rather than driven for a sliver (reference `initial_wait`).
    #[must_use]
    pub const fn initial_wait(self) -> u16 {
        if self.delay_us < 100 {
            self.delay_us
        } else {
            0
        }
    }
}

/// Q0.32 electrical revolutions per microsecond for a frequency in
/// centihertz. Const: evaluated at compile time for the one rate used.
#[must_use]
pub const fn phase_rate(freq_chz: u32) -> u32 {
    ((freq_chz as u64 * (1u64 << 32)) / 100_000_000) as u32
}

/// Advance a phase by whole degrees (reference `campaign::phase_shift`).
#[must_use]
pub const fn phase_shift(theta: u32, degrees: i32) -> u32 {
    theta.wrapping_add(((degrees as i64 * (1i64 << 32)) / 360) as u32)
}

/// Admissible rates: the reference tested 50..300 eHz.
pub const RATE_MIN: u32 = 214_748;
pub const RATE_MAX: u32 = 1_288_490;

/// The current sector and the delay to the next boundary, for accumulator
/// `theta` turning at `rate`. `None` outside the tested rate band.
#[must_use]
pub fn next(theta: u32, rate: u32) -> Option<Boundary> {
    if !(RATE_MIN..=RATE_MAX).contains(&rate) {
        return None;
    }
    let index = (theta >> 24) as usize & 255;
    let distance = NEXT[index] as usize;
    if distance == 0 {
        return None;
    }
    let next_index = (index + distance) & 255;
    let delta = ((next_index as u32) << 24).wrapping_sub(theta);
    // Ceiling division without overflow (reference).
    let delay = (delta - 1) / rate + 1;
    if delay == 0 || delay > u16::MAX as u32 {
        return None;
    }
    Some(Boundary {
        step: Step::new(STEPS[index])?,
        next_step: Step::new(STEPS[next_index])?,
        delay_us: delay as u16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commutation::{sector, Phase};

    /// Every accumulator position maps to a real sector.
    #[test]
    fn every_position_has_a_sector() {
        for (i, s) in STEPS.iter().enumerate() {
            assert!((1..=6).contains(s), "position {i} -> {s}");
        }
    }

    /// The sector a vector selects is the one whose table source/sink are the
    /// highest/lowest phases -- firmware50's `sector()` and the reference's
    /// `step_for_values` must agree, or the driven stage drives a different
    /// sector from the one the detector expects.
    #[test]
    fn step_for_values_agrees_with_the_sector_table() {
        for s in 1..=6u8 {
            let sec = sector(Step::new(s).unwrap());
            let mut v = [128u8; 3];
            v[sec.source.index()] = 250;
            v[sec.sink.index()] = 5;
            v[sec.floating.index()] = 128;
            assert_eq!(step_for_values(v), s, "sector {s}");
        }
        let _ = Phase::A;
    }

    /// As the accumulator advances the sector sequence steps 1,2,3,4,5,6 --
    /// the same logical order the closed loop commutates in -- so a positive
    /// rate turns the rotor the way the loop will.
    #[test]
    fn increasing_phase_walks_the_logical_sequence_forward() {
        let mut changes = 0;
        for i in 0..256usize {
            let a = STEPS[i];
            let b = STEPS[(i + 1) & 255];
            if a != b {
                assert_eq!(b, a % 6 + 1, "at {i}: {a} -> {b}");
                changes += 1;
            }
        }
        assert_eq!(changes, 6, "one full cycle has six sectors");
    }

    #[test]
    fn every_lookup_reaches_exactly_the_next_sector() {
        for hz in [50u32, 100, 167, 200, 250, 300] {
            let rate = phase_rate(hz * 100);
            for index in 0..256u32 {
                for fraction in [0u32, 1, 0x7f_ffff, 0xff_ffff] {
                    let theta = (index << 24) | fraction;
                    let b = next(theta, rate).unwrap();
                    assert_eq!(b.step.get(), STEPS[index as usize]);
                    assert_eq!(b.next_step, b.step.next());
                    let before = theta.wrapping_add(rate * (b.delay_us as u32 - 1));
                    let at = theta.wrapping_add(rate * b.delay_us as u32);
                    assert_eq!(STEPS[(before >> 24) as usize], b.step.get());
                    assert_eq!(STEPS[(at >> 24) as usize], b.next_step.get());
                }
            }
        }
    }

    /// Recomputing at each boundary from the original phase never drifts: 20 ms
    /// at 200 eHz is four cycles, 24 sectors.
    #[test]
    fn a_long_schedule_accumulates_no_rounding() {
        let rate = phase_rate(20_000);
        for initial in [0u32, 0x1234_5678, u32::MAX] {
            let mut elapsed = 0u32;
            for _ in 0..24 {
                let theta = initial.wrapping_add(rate.wrapping_mul(elapsed));
                let b = next(theta, rate).unwrap();
                elapsed += b.delay_us as u32;
                let after = initial.wrapping_add(rate.wrapping_mul(elapsed));
                assert_eq!(STEPS[(after >> 24) as usize], b.next_step.get());
            }
            assert!((19_000..=20_001).contains(&elapsed), "{elapsed}");
        }
    }

    #[test]
    fn out_of_band_rates_are_refused() {
        assert!(next(0, 0).is_none());
        assert!(next(0, u32::MAX).is_none());
        assert!(next(0, RATE_MIN - 1).is_none());
        assert!(next(0, RATE_MAX + 1).is_none());
    }

    #[test]
    fn a_sector_at_200_ehz_is_833_microseconds() {
        let rate = phase_rate(20_000);
        // Start exactly on a boundary so the whole sector is measured.
        let mut i = 0usize;
        while STEPS[i] == STEPS[(i + 1) & 255] {
            i += 1;
        }
        let theta = (((i + 1) & 255) as u32) << 24;
        let b = next(theta, rate).unwrap();
        assert!((820..=846).contains(&b.delay_us), "{}", b.delay_us);
    }

    #[test]
    fn sixty_degrees_is_one_sector_of_phase() {
        let shifted = phase_shift(0, 60);
        assert_eq!(shifted, 715_827_882);
        // One full turn returns to the start.
        assert_eq!(phase_shift(0, 360), 0);
    }

    #[test]
    fn short_initial_sectors_are_waited_out_not_driven() {
        let rate = phase_rate(20_000);
        let mut skipped = 0;
        for index in 0..256u32 {
            let b = next(index << 24, rate).unwrap();
            let wait = b.initial_wait();
            assert!(wait < 100);
            if wait != 0 {
                skipped += 1;
                let actual = (index << 24).wrapping_add(rate * (wait as u32 + 1));
                let admitted = next(actual, rate).unwrap();
                assert_eq!(admitted.step, b.next_step);
            }
        }
        assert!(skipped > 0);
    }
}
