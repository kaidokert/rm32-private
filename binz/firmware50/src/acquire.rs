//! Handover catch: measure the rotor's real interval before closing the loop.
//!
//! **Why this module exists.** Every closed-loop attempt in this crate handed
//! over *blind*: the open-loop ramp ran to a scheduled frequency, the loop was
//! closed at a scheduled time, and the interval estimator was seeded from the
//! *scheduled* interval. If the rotor had fallen out of step with the open-loop
//! drive -- and above ~110 eHz on this rig it does -- the estimator started
//! from a speed the rotor was not doing, no real crossing ever matched it, and
//! the bridge whined at a stationary rotor (notebook E031, E035). Open-loop
//! spin works; the transfer was never a catch.
//!
//! The qualified image does the opposite, and its run shows it:
//! `DRIVESTOP acquisition_reason=22 acquisition_us=10706` and
//! `DRIVENENTRY ... fly_seeded=1`. It keeps driving open-loop while it
//! *watches real back-EMF edges*, validates a whole electrical cycle of them,
//! and seeds the closed loop with the **measured** interval -- a flying seed.
//!
//! The validity bounds are the reference's, from its source-derived constants
//! table (`binz/LOW_DUTY_REPLICATION.md`), converted from its 0.5 µs acquisition
//! ticks to microseconds:
//!
//! | reference constant | half-µs ticks | µs | meaning |
//! |---|---|---|---|
//! | `INDIVIDUAL_MIN_TICKS` | 476 | 238 | shortest admissible single interval |
//! | `CYCLE_MIN_TICKS` | 5716 | 2858 | shortest admissible six-event cycle |
//! | `SEED_MIN_TICKS` | 952 | 476 | shortest admissible seed interval |
//!
//! One rule here is **mine and labelled so**: the consistency check (every
//! interval within a quarter of the cycle mean). The reference validates a
//! flying seed, but its exact consistency test is not in the documents I have,
//! and accepting six wildly different intervals as a "cycle" would seed the
//! loop with noise. It is a stated divergence, bounded, and host-tested.

/// Shortest admissible single acquisition interval, µs (`INDIVIDUAL_MIN_TICKS`).
pub const INDIVIDUAL_MIN_US: u32 = 238;
/// Shortest admissible six-event cycle, µs (`CYCLE_MIN_TICKS`).
pub const CYCLE_MIN_US: u32 = 2_858;
/// Shortest admissible seed interval, µs (`SEED_MIN_TICKS`).
pub const SEED_MIN_US: u32 = 476;
/// Longest interval treated as part of a cycle rather than a lost edge, µs.
///
/// Not a reference constant -- a guard against a stalled rotor feeding one
/// enormous interval into the cycle. 20 ms is one sector at 8 eHz, far slower
/// than any catch speed, so a real rotor never reaches it.
pub const INDIVIDUAL_MAX_US: u32 = 20_000;

/// Events per electrical cycle in six-step.
const EVENTS: usize = 6;

/// Why an acquisition attempt did not produce a seed.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Rejection {
    /// An interval shorter than `INDIVIDUAL_MIN_US`: comparator noise.
    IntervalTooShort,
    /// An interval longer than `INDIVIDUAL_MAX_US`: a lost edge or a stall.
    IntervalTooLong,
    /// The six-event cycle was shorter than `CYCLE_MIN_US`.
    CycleTooShort,
    /// The derived seed was shorter than `SEED_MIN_US`.
    SeedTooShort,
    /// Intervals disagreed by more than the consistency band.
    Inconsistent,
}

/// Collects back-EMF edge timestamps and yields a validated seed interval once
/// a whole, consistent electrical cycle has been observed.
#[derive(Copy, Clone, Debug)]
pub struct Acquisition {
    intervals: [u32; EVENTS],
    count: usize,
    last_edge: Option<u32>,
    rejections: u32,
    last_rejection: Option<Rejection>,
}

impl Default for Acquisition {
    fn default() -> Self {
        Self::new()
    }
}

impl Acquisition {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            intervals: [0; EVENTS],
            count: 0,
            last_edge: None,
            rejections: 0,
            last_rejection: None,
        }
    }

    /// Discard everything and start a fresh cycle.
    fn restart(&mut self, why: Rejection) {
        self.count = 0;
        self.rejections = self.rejections.saturating_add(1);
        self.last_rejection = Some(why);
    }

    /// Offer one back-EMF edge at `t_us`. Returns `Some(seed_us)`, the measured
    /// sector interval, once six consistent intervals have been observed.
    ///
    /// A rejection restarts the cycle from this edge rather than aborting the
    /// acquisition: one noisy edge should cost one cycle, not the catch.
    #[must_use = "a validated seed that is dropped is a catch that never happens"]
    pub fn edge(&mut self, t_us: u32) -> Option<u32> {
        let Some(prev) = self.last_edge else {
            self.last_edge = Some(t_us);
            return None;
        };
        self.last_edge = Some(t_us);
        let dt = t_us.wrapping_sub(prev);

        if dt < INDIVIDUAL_MIN_US {
            self.restart(Rejection::IntervalTooShort);
            return None;
        }
        if dt > INDIVIDUAL_MAX_US {
            self.restart(Rejection::IntervalTooLong);
            return None;
        }

        // Bounded index: `count` is always < EVENTS here.
        if self.count < EVENTS {
            self.intervals[self.count] = dt;
            self.count += 1;
        }
        if self.count < EVENTS {
            return None;
        }

        // A whole cycle is in hand; validate it, then slide the window so the
        // next edge re-tests the most recent six rather than starting over.
        let verdict = self.validate();
        self.intervals.copy_within(1.., 0);
        self.count = EVENTS - 1;
        match verdict {
            Ok(seed) => Some(seed),
            Err(why) => {
                self.rejections = self.rejections.saturating_add(1);
                self.last_rejection = Some(why);
                None
            }
        }
    }

    fn validate(&self) -> Result<u32, Rejection> {
        let mut cycle: u32 = 0;
        for dt in self.intervals {
            cycle = cycle.saturating_add(dt);
        }
        if cycle < CYCLE_MIN_US {
            return Err(Rejection::CycleTooShort);
        }
        let seed = div_by_6(cycle);
        if seed < SEED_MIN_US {
            return Err(Rejection::SeedTooShort);
        }
        // Consistency (mine): every interval within a quarter of the mean.
        let band = seed >> 2;
        for dt in self.intervals {
            let dev = dt.abs_diff(seed);
            if dev > band {
                return Err(Rejection::Inconsistent);
            }
        }
        Ok(seed)
    }

    /// Cycles rejected so far. Reported, because a catch that never happens
    /// needs to say *why* -- the same discipline as the refusal counters.
    #[must_use]
    pub const fn rejections(&self) -> u32 {
        self.rejections
    }

    #[must_use]
    pub const fn last_rejection(&self) -> Option<Rejection> {
        self.last_rejection
    }
}

/// `x / 6` without a division instruction, exact for every `x` this module can
/// produce.
///
/// The M0 has no divider, so `/6` would compile to `__aeabi_uidiv`. Acquisition
/// runs in the foreground, where that would be tolerable, but a reciprocal
/// costs nothing and keeps the module usable from interrupt context later.
/// `43691 / 2^18` over-approximates `1/6` by 1 part in 262144; the host test
/// `div_by_6_is_exact_over_the_whole_range` proves the truncated result equals
/// `x / 6` for every `x` up to `6 * INDIVIDUAL_MAX_US`, the largest cycle that
/// can be offered.
#[inline]
#[must_use]
pub const fn div_by_6(x: u32) -> u32 {
    ((x as u64 * 43_691) >> 18) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feed a steady edge train at `interval` µs and return the first seed.
    fn steady(interval: u32, edges: usize) -> Option<u32> {
        let mut a = Acquisition::new();
        let mut t = 1_000u32;
        let mut seed = None;
        for _ in 0..edges {
            if let Some(s) = a.edge(t) {
                seed = Some(s);
                break;
            }
            t = t.wrapping_add(interval);
        }
        seed
    }

    #[test]
    fn a_steady_rotor_at_100_ehz_yields_its_own_interval() {
        // 100 eHz: one sector is 1666 us.
        assert_eq!(steady(1_666, 20), Some(1_666));
    }

    #[test]
    fn a_seed_needs_a_whole_cycle() {
        // Six intervals need seven edges; six edges are not enough.
        assert_eq!(steady(1_666, 6), None);
        assert_eq!(steady(1_666, 7), Some(1_666));
    }

    #[test]
    fn comparator_noise_restarts_the_cycle() {
        let mut a = Acquisition::new();
        let mut t = 0u32;
        for _ in 0..4 {
            let _ = a.edge(t);
            t += 1_666;
        }
        // A spurious edge 50 us later: below INDIVIDUAL_MIN_US.
        let _ = a.edge(t - 1_666 + 50);
        assert_eq!(a.last_rejection(), Some(Rejection::IntervalTooShort));
        assert_eq!(a.rejections(), 1);
    }

    #[test]
    fn a_stall_is_rejected_not_seeded() {
        let mut a = Acquisition::new();
        let _ = a.edge(0);
        let _ = a.edge(INDIVIDUAL_MAX_US + 1);
        assert_eq!(a.last_rejection(), Some(Rejection::IntervalTooLong));
    }

    #[test]
    fn a_rotor_faster_than_the_seed_floor_is_refused() {
        // 300 us sectors: individual floor passes (>= 238), but the six-event
        // cycle of 1800 us is below CYCLE_MIN_US.
        let mut a = Acquisition::new();
        let mut t = 0u32;
        let mut got = None;
        for _ in 0..12 {
            if let Some(s) = a.edge(t) {
                got = Some(s);
            }
            t += 300;
        }
        assert_eq!(got, None);
        assert_eq!(a.last_rejection(), Some(Rejection::CycleTooShort));
    }

    #[test]
    fn inconsistent_intervals_are_not_a_cycle() {
        let mut a = Acquisition::new();
        let mut t = 0u32;
        // Alternating 1000 / 2400 us: mean 1700, each off by far more than a
        // quarter.
        let mut got = None;
        for i in 0..14 {
            if let Some(s) = a.edge(t) {
                got = Some(s);
            }
            t += if i % 2 == 0 { 1_000 } else { 2_400 };
        }
        assert_eq!(got, None);
        assert_eq!(a.last_rejection(), Some(Rejection::Inconsistent));
    }

    #[test]
    fn mild_jitter_within_the_band_still_seeds() {
        let mut a = Acquisition::new();
        let mut t = 0u32;
        let pattern = [1_600u32, 1_700, 1_650, 1_720, 1_620, 1_680, 1_660];
        let mut got = None;
        for dt in pattern.iter().chain(pattern.iter()) {
            if let Some(s) = a.edge(t) {
                got = Some(s);
                break;
            }
            t += dt;
        }
        let seed = got.expect("jitter within a quarter should seed");
        assert!((1_550..=1_750).contains(&seed), "seed {seed}");
    }

    #[test]
    fn timestamps_may_wrap() {
        let mut a = Acquisition::new();
        let mut t = u32::MAX - 3_000;
        let mut got = None;
        for _ in 0..20 {
            if let Some(s) = a.edge(t) {
                got = Some(s);
                break;
            }
            t = t.wrapping_add(1_666);
        }
        assert_eq!(got, Some(1_666));
    }

    #[test]
    fn div_by_6_is_exact_over_the_whole_range() {
        let top = 6 * INDIVIDUAL_MAX_US + 6;
        let mut x = 0u32;
        while x <= top {
            assert_eq!(div_by_6(x), x / 6, "x={x}");
            x += 1;
        }
    }

    #[test]
    fn reference_bounds_are_the_half_microsecond_ticks_halved() {
        // The table in the module docs, asserted, so a mistyped conversion
        // cannot survive.
        assert_eq!(INDIVIDUAL_MIN_US, 476 / 2);
        assert_eq!(CYCLE_MIN_US, 5_716 / 2);
        assert_eq!(SEED_MIN_US, 952 / 2);
    }
}
