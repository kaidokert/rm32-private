//! Startup seed: qualify real comparator acceptances observed under driven
//! six-step, and produce the closed loop's first interval estimate.
//!
//! Transcribed from `binz/examples/support/driven_seed.rs::accept_startup`,
//! the path the frozen oracle compiles (`bench-startup-adc` selects
//! `Qualification::with_startup_estimator`). The reference works in half-µs
//! TIM2 ticks; this crate works in µs, and every constant is converted and
//! asserted in `constants_are_the_references_halved`.
//!
//! What it accepts, in the reference's own terms:
//!
//! * one acceptance per driven sector, tagged with that sector's command
//!   **epoch** (the index of the driven commutation that armed it);
//! * epoch 0 is never an anchor: it starts part-way through a sector inherited
//!   from the sine;
//! * consecutive epochs with consecutive steps extend the run; a forward gap
//!   of 2..=6 epochs whose step is consistent with the gap re-anchors (starts
//!   the count again, never bridging the missing edge); anything else fails;
//! * each spacing is at most `DELTA_MAX_US`, and the ISR's own measured
//!   interval must agree with the timestamps (corroboration of the timeline);
//! * after **twelve** consecutive intervals the mean must lie in
//!   `[SEED_MIN_US, SEED_MAX_US]`; the seed is the step, edge time and mean;
//! * the whole acquisition must finish within `WINDOW_US` of its start.
//!
//! A completed seed is frozen: later acceptances never refresh it.

use crate::commutation::Step;

/// Longest admissible spacing between consecutive acceptances (12000 half-µs).
pub const DELTA_MAX_US: u32 = 6_000;
/// Acquisition window from the driven stage's start (80000 half-µs).
pub const WINDOW_US: u32 = 40_000;
/// Mean interval bounds (12·952 and 24000 half-µs over twelve intervals).
pub const SEED_MIN_US: u32 = 476;
pub const SEED_MAX_US: u32 = 1_000;
/// Intervals averaged into the seed.
pub const INTERVALS: u8 = 12;
/// Timestamp slack for the corroboration check (reference: 2 half-µs ticks,
/// rounded up to one whole µs).
const SLACK_US: u32 = 1;

/// One persistence-qualified acceptance.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Command epoch of the driven sector the edge was accepted in.
    pub epoch: u16,
    /// Logical step of that sector.
    pub step: Step,
    /// Extended µs timestamp of the edge.
    pub at_us: u32,
    /// The ISR's own measurement of the time since the previous acceptance,
    /// from the raw 16-bit timer, µs.
    pub interval_us: u32,
}

/// The closed loop's starting point.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub step: Step,
    pub edge_us: u32,
    pub interval_us: u32,
}

/// Why qualification stopped for good.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The window elapsed without a seed (reference fault 6).
    Window,
    /// An epoch/step sequence that is neither a successor nor a clean gap (2).
    Sequence,
    /// A zero or over-long spacing (3).
    Spacing,
    /// The ISR's interval disagrees with the timestamps (4).
    Corroboration,
}

pub struct Qualification {
    start_us: u32,
    previous: Option<Edge>,
    ready: Option<Seed>,
    fault: Option<Fault>,
    sum: u32,
    count: u8,
    reanchors: u16,
}

/// `x / 12` for the bounded domain, as the reference computes it
/// (`* 21_846 >> 18`). Exact for every sum that can reach it.
#[must_use]
pub const fn div_by_12(x: u32) -> u32 {
    (x * 21_846) >> 18
}

impl Qualification {
    #[must_use]
    pub const fn new(start_us: u32) -> Self {
        Self {
            start_us,
            previous: None,
            ready: None,
            fault: None,
            sum: 0,
            count: 0,
            reanchors: 0,
        }
    }

    #[must_use]
    pub const fn ready(&self) -> Option<Seed> {
        self.ready
    }

    #[must_use]
    pub const fn fault(&self) -> Option<Fault> {
        self.fault
    }

    #[must_use]
    pub const fn intervals(&self) -> u8 {
        self.count
    }

    #[must_use]
    pub const fn reanchors(&self) -> u16 {
        self.reanchors
    }

    fn fail(&mut self, f: Fault) -> Option<Seed> {
        self.fault = Some(f);
        self.ready = None;
        None
    }

    fn restart(&mut self) {
        self.previous = None;
        self.sum = 0;
        self.count = 0;
    }

    /// Has the acquisition window run out at `now_us` without a seed?
    #[must_use]
    pub fn expired(&self, now_us: u32) -> bool {
        self.ready.is_none() && now_us.wrapping_sub(self.start_us) > WINDOW_US
    }

    /// Offer one acceptance. Returns the seed on the acceptance that completes
    /// it, and the same frozen seed thereafter.
    pub fn accept(&mut self, e: Edge) -> Option<Seed> {
        if self.fault.is_some() {
            return None;
        }
        if self.ready.is_some() {
            return self.ready;
        }
        if e.at_us.wrapping_sub(self.start_us) > WINDOW_US {
            return self.fail(Fault::Window);
        }
        if self.previous.is_none() && e.epoch == 0 {
            return None;
        }
        if let Some(p) = self.previous {
            let gap = e.epoch.wrapping_sub(p.epoch);
            let delta = e.at_us.wrapping_sub(p.at_us);
            if delta == 0 || delta > DELTA_MAX_US {
                return self.fail(Fault::Spacing);
            }
            let lower = delta.saturating_sub(SLACK_US);
            let upper = delta.saturating_add(SLACK_US);
            if e.interval_us < lower || e.interval_us > upper {
                return self.fail(Fault::Corroboration);
            }
            if gap != 1 || e.step != p.step.next() {
                if !(2..=6).contains(&gap) || e.step != step_after(p.step, gap) {
                    return self.fail(Fault::Sequence);
                }
                self.restart();
                self.reanchors = self.reanchors.saturating_add(1);
            } else {
                self.sum += delta;
                self.count += 1;
                if self.count == INTERVALS {
                    if self.sum >= INTERVALS as u32 * SEED_MIN_US && self.sum <= INTERVALS as u32 * SEED_MAX_US {
                        let seed = Seed {
                            step: e.step,
                            edge_us: e.at_us,
                            interval_us: div_by_12(self.sum),
                        };
                        self.ready = Some(seed);
                        self.previous = Some(e);
                        return Some(seed);
                    }
                    self.restart();
                    self.reanchors = self.reanchors.saturating_add(1);
                }
            }
        }
        if self.previous.is_none() {
            self.sum = 0;
            self.count = 0;
        }
        self.previous = Some(e);
        None
    }
}

/// The step `gap` sectors after `step` (gap already validated to 2..=6).
const fn step_after(step: Step, gap: u16) -> Step {
    let mut s = step;
    let mut i = 0;
    while i < gap {
        s = s.next();
        i += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const I: u32 = 833;

    fn edge(epoch: u16, at: u32, interval: u32) -> Edge {
        Edge {
            epoch,
            step: step_of(epoch),
            at_us: at,
            interval_us: interval,
        }
    }

    /// Epoch n drives step (n mod 6) + 1 in these tests.
    fn step_of(epoch: u16) -> Step {
        Step::new((epoch % 6) as u8 + 1).unwrap()
    }

    #[test]
    fn constants_are_the_references_halved() {
        assert_eq!(DELTA_MAX_US, 12_000 / 2);
        assert_eq!(WINDOW_US, 80_000 / 2);
        assert_eq!(SEED_MIN_US, 952 / 2);
        assert_eq!(SEED_MAX_US, 24_000 / 12 / 2);
    }

    #[test]
    fn div_by_12_is_exact_over_the_reachable_domain() {
        for x in 0..=(INTERVALS as u32 * SEED_MAX_US) {
            assert_eq!(div_by_12(x), x / 12, "x={x}");
        }
    }

    #[test]
    fn thirteen_consecutive_acceptances_seed_on_the_thirteenth() {
        let mut q = Qualification::new(0);
        let mut got = None;
        for n in 1..=13u16 {
            let r = q.accept(edge(n, 1_000 + n as u32 * I, I));
            if n < 13 {
                assert_eq!(r, None, "early seed at {n}");
            }
            got = r;
        }
        let s = got.expect("12 intervals seed");
        assert_eq!(s.interval_us, I);
        assert_eq!(s.step, step_of(13));
        assert_eq!(s.edge_us, 1_000 + 13 * I);
    }

    #[test]
    fn epoch_zero_never_anchors() {
        let mut q = Qualification::new(0);
        assert_eq!(q.accept(edge(0, 500, 500)), None);
        assert_eq!(q.intervals(), 0);
        // The first real anchor is epoch 1; twelve more intervals are needed.
        for n in 1..=12u16 {
            assert_eq!(q.accept(edge(n, 1_000 + n as u32 * I, I)), None);
        }
        assert!(q.accept(edge(13, 1_000 + 13 * I, I)).is_some());
    }

    #[test]
    fn a_missing_sector_reanchors_without_bridging() {
        let mut q = Qualification::new(0);
        for n in 1..=6u16 {
            let _ = q.accept(edge(n, n as u32 * I, I));
        }
        // Epoch 7 missing; 8 arrives two sectors later with a consistent step.
        let _ = q.accept(edge(8, 8 * I, 2 * I));
        assert_eq!(q.fault(), None);
        assert_eq!(q.reanchors(), 1);
        assert_eq!(q.intervals(), 0);
    }

    #[test]
    fn an_inconsistent_step_fails() {
        let mut q = Qualification::new(0);
        let _ = q.accept(edge(1, I, I));
        let mut bad = edge(2, 2 * I, I);
        bad.step = step_of(4);
        let _ = q.accept(bad);
        assert_eq!(q.fault(), Some(Fault::Sequence));
    }

    #[test]
    fn an_over_long_spacing_fails() {
        let mut q = Qualification::new(0);
        let _ = q.accept(edge(1, 1_000, I));
        let _ = q.accept(edge(2, 1_000 + DELTA_MAX_US + 1, DELTA_MAX_US + 1));
        assert_eq!(q.fault(), Some(Fault::Spacing));
    }

    #[test]
    fn a_disagreeing_isr_interval_fails() {
        let mut q = Qualification::new(0);
        let _ = q.accept(edge(1, 1_000, I));
        let _ = q.accept(edge(2, 1_000 + I, I + 50));
        assert_eq!(q.fault(), Some(Fault::Corroboration));
    }

    #[test]
    fn a_mean_outside_the_band_reanchors_rather_than_seeding() {
        // 1100 us spacing: 12 * 1100 exceeds 12 * SEED_MAX_US.
        let mut q = Qualification::new(0);
        for n in 1..=13u16 {
            assert_eq!(q.accept(edge(n, n as u32 * 1_100, 1_100)), None);
        }
        assert_eq!(q.fault(), None);
        assert_eq!(q.reanchors(), 1);
    }

    #[test]
    fn the_window_bounds_the_whole_acquisition() {
        let mut q = Qualification::new(0);
        assert!(!q.expired(WINDOW_US));
        assert!(q.expired(WINDOW_US + 1));
        let _ = q.accept(edge(1, WINDOW_US + 1, I));
        assert_eq!(q.fault(), Some(Fault::Window));
    }

    #[test]
    fn a_seed_is_frozen() {
        let mut q = Qualification::new(0);
        let mut first = None;
        for n in 1..=13u16 {
            first = q.accept(edge(n, n as u32 * I, I));
        }
        let later = q.accept(edge(14, 14 * I + 100, I + 100));
        assert_eq!(first, later);
    }

    #[test]
    fn jitter_is_averaged_not_refused() {
        let mut q = Qualification::new(0);
        let spacing = [800u32, 870, 810, 860, 820, 850, 830, 840, 800, 866, 826, 834];
        let mut t = 1_000;
        let _ = q.accept(edge(1, t, I));
        let mut got = None;
        for (k, d) in spacing.iter().enumerate() {
            t += d;
            got = q.accept(edge(k as u16 + 2, t, *d));
        }
        let s = got.expect("seeded");
        assert_eq!(s.interval_us, spacing.iter().sum::<u32>() / 12);
    }
}
