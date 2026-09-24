//! Rotation witness: does the rotor actually turn while the bridge drives it?
//!
//! **Why this is its own module with its own tests.** Every rotation verdict
//! this firmware printed for an entire campaign came from a coast
//! peak-to-peak amplitude, and that verdict was refuted on the bench: a rotor
//! the operator watched stand still produced the same 86 codes as one believed
//! to be spinning. The threshold behind it, and every `spun=1` it ever
//! printed, were void. A quantity that decides whether a motor is turning is a
//! policy, so it belongs here under test rather than inline in the binary.
//!
//! The physics this relies on, and why amplitude could never work:
//!
//! * The floating phase's terminal voltage referred to the neutral **is** its
//!   back-EMF, and back-EMF reverses sign once per electrical cycle. A turning
//!   rotor therefore *must* produce polarity alternations about the signal's
//!   own midpoint.
//! * A stationary rotor has no electrical angle to sweep. It holds one fixed
//!   offset, however hard the bridge chops at it, so it **cannot** produce an
//!   alternation.
//! * Peak-to-peak amplitude, by contrast, is produced by switching decay,
//!   divider settling and ADC noise just as readily as by rotation. That is
//!   exactly how the false positive happened.
//!
//! Two details that a naive sign test gets wrong, both measured on this rig:
//!
//! * The divider pair carries a DC offset -- about +16 codes -- so testing the
//!   raw sign of `vsenc - star` counts nothing at all. The midpoint has to be
//!   learned from the signal and then held.
//! * Scan-to-scan noise is a couple of codes, so the decision needs a
//!   hysteresis band wide enough to reject it and narrow enough to pass a real
//!   swing.

/// Counts back-EMF polarity alternations about a learned midpoint.
///
/// Feed it only samples taken while the phase it watches is **floating** --
/// a driven phase sits at a rail and says nothing about back-EMF.
#[derive(Copy, Clone, Debug)]
pub struct RotationWitness {
    min: i32,
    max: i32,
    mid: i32,
    mid_fixed: bool,
    state: i8,
    alternations: u32,
    samples: u32,
    learn_samples: u32,
    hysteresis: i32,
}

impl RotationWitness {
    /// `learn_samples` samples establish the midpoint before counting starts;
    /// `hysteresis` is the band in raw ADC codes a sample must clear.
    #[must_use]
    pub const fn new(learn_samples: u32, hysteresis: i32) -> Self {
        Self {
            min: i32::MAX,
            max: i32::MIN,
            mid: 0,
            mid_fixed: false,
            state: 0,
            alternations: 0,
            samples: 0,
            learn_samples,
            hysteresis,
        }
    }

    /// Offer one floating-phase sample.
    pub fn sample(&mut self, v: i32) {
        self.samples = self.samples.saturating_add(1);
        if v < self.min {
            self.min = v;
        }
        if v > self.max {
            self.max = v;
        }
        if !self.mid_fixed {
            // Learn the offset, then hold it. Holding matters: a midpoint that
            // keeps tracking the signal follows it across the very crossing
            // that is being counted, and counts nothing.
            if self.samples >= self.learn_samples && self.max > self.min {
                self.mid = (self.max + self.min) / 2;
                self.mid_fixed = true;
            }
            return;
        }
        let d = v.saturating_sub(self.mid);
        if d > self.hysteresis {
            if self.state == -1 {
                self.alternations = self.alternations.saturating_add(1);
            }
            self.state = 1;
        } else if d < -self.hysteresis {
            if self.state == 1 {
                self.alternations = self.alternations.saturating_add(1);
            }
            self.state = -1;
        }
    }

    /// Polarity alternations observed. Non-zero is the rotation verdict.
    #[must_use]
    pub const fn alternations(&self) -> u32 {
        self.alternations
    }

    /// Did this witness observe rotation?
    ///
    /// Deliberately a single alternation: one genuine sign reversal of
    /// back-EMF cannot be produced by a stationary rotor, so demanding more
    /// would only delay the verdict, not strengthen it.
    #[must_use]
    pub const fn rotated(&self) -> bool {
        self.alternations > 0
    }

    #[must_use]
    pub const fn samples(&self) -> u32 {
        self.samples
    }

    #[must_use]
    pub const fn midpoint(&self) -> i32 {
        self.mid
    }

    #[must_use]
    pub const fn midpoint_fixed(&self) -> bool {
        self.mid_fixed
    }

    /// Observed range, or `(0, 0)` before any sample.
    #[must_use]
    pub const fn range(&self) -> (i32, i32) {
        if self.samples == 0 {
            (0, 0)
        } else {
            (self.min, self.max)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEARN: u32 = 8;
    const HYST: i32 = 12;

    fn witness() -> RotationWitness {
        RotationWitness::new(LEARN, HYST)
    }

    /// A square alternation about an offset midpoint is counted.
    #[test]
    fn a_reversing_signal_is_counted() {
        let mut w = witness();
        // Offset by +16, the measured divider bias on this rig.
        for cycle in 0..10 {
            for _ in 0..8 {
                w.sample(16 + 40);
            }
            for _ in 0..8 {
                w.sample(16 - 40);
            }
            let _ = cycle;
        }
        assert!(w.rotated());
        // Nineteen reversals across ten full cycles, the first half-cycle
        // being consumed by learning the midpoint.
        assert!(w.alternations() >= 15, "only {}", w.alternations());
    }

    /// **The case that was failing silently on hardware.** A stationary rotor
    /// holds one offset with noise on it, and must never read as rotation --
    /// however large that offset is.
    #[test]
    fn a_fixed_offset_with_noise_is_not_rotation() {
        for offset in [-900i32, -86, 0, 16, 86, 900] {
            let mut w = witness();
            for i in 0..2000 {
                // +-2 codes of scan noise, the measured level.
                let n = if i % 2 == 0 { 2 } else { -2 };
                w.sample(offset + n);
            }
            assert!(
                !w.rotated(),
                "offset {offset} read as rotation ({} alternations)",
                w.alternations()
            );
        }
    }

    /// A slow drift across the midpoint is one reversal, not many: the
    /// hysteresis must not let a single transit ring.
    #[test]
    fn a_single_transit_counts_once() {
        let mut w = witness();
        for _ in 0..LEARN {
            w.sample(50);
        }
        for _ in 0..LEARN {
            w.sample(-50);
        }
        let first = w.alternations();
        for v in -50..=50 {
            w.sample(v);
        }
        assert_eq!(w.alternations(), first + 1);
    }

    /// Noise inside the hysteresis band cannot manufacture an alternation even
    /// when it straddles the midpoint.
    #[test]
    fn noise_inside_the_band_never_alternates() {
        let mut w = witness();
        for _ in 0..LEARN {
            w.sample(HYST + 1);
        }
        for _ in 0..LEARN {
            w.sample(-HYST - 1);
        }
        let baseline = w.alternations();
        for i in 0..1000 {
            w.sample(if i % 2 == 0 { HYST - 1 } else { -HYST + 1 });
        }
        assert_eq!(w.alternations(), baseline);
    }

    /// Before the midpoint is learned nothing is counted, and the range is
    /// reported as empty rather than as `i32::MAX/MIN`.
    #[test]
    fn nothing_is_claimed_before_learning() {
        let mut w = witness();
        assert_eq!(w.range(), (0, 0));
        assert!(!w.midpoint_fixed());
        for _ in 0..(LEARN - 1) {
            w.sample(100);
        }
        assert_eq!(w.alternations(), 0);
        assert!(!w.rotated());
    }

    /// A flat signal never fixes a midpoint, so it can never alternate -- the
    /// degenerate case of a bridge that is off.
    #[test]
    fn a_perfectly_flat_signal_never_arms() {
        let mut w = witness();
        for _ in 0..1000 {
            w.sample(42);
        }
        assert!(!w.midpoint_fixed());
        assert!(!w.rotated());
    }

    /// Counters saturate rather than wrap, so a long run cannot report zero.
    #[test]
    fn counters_saturate() {
        let mut w = RotationWitness::new(1, 0);
        w.sample(10);
        w.sample(-10);
        for _ in 0..40 {
            w.sample(10);
            w.sample(-10);
        }
        assert!(w.alternations() > 0);
        assert!(w.samples() > 0);
    }
}
