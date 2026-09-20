//! Observe-only crossing evidence around minz's existing polarity/filter.
//! No peripheral access, scheduler, duty output, or commutation authority.
//! Two opposite samples arm; two expected samples produce one candidate.
//! These bench thresholds are not a qualified production control tuning.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Reason {
    Invalid,
    NeedBaseline,
    Opposite,
    Accumulating,
    Candidate,
    Latched,
}

#[derive(Clone, Copy, Debug)]
pub struct Decision {
    pub expected: bool,
    pub armed: bool,
    pub event: bool,
    pub latched: bool,
    pub reason: Reason,
}

#[derive(Default)]
pub struct Detector {
    step: u8,
    before: u16,
    count: u16,
    bad: u16,
    armed: bool,
    latched: bool,
}

impl Detector {
    pub fn update(&mut self, step: u8, level: bool, valid: bool) -> Decision {
        if step != self.step {
            *self = Self {
                step,
                ..Self::default()
            };
        }
        let valid = valid && (1..=6).contains(&step);
        let expected = if valid {
            minz_core::drive::edges_for(3, step - 1).0
        } else {
            false
        };
        let reason = if !valid {
            self.before = 0;
            self.count = 0;
            self.bad = 0;
            self.armed = false;
            Reason::Invalid
        } else if self.latched {
            Reason::Latched
        } else if !self.armed {
            if level != expected {
                self.before = self.before.saturating_add(1);
                self.armed = self.before >= 2;
                Reason::Opposite
            } else {
                self.before = 0;
                Reason::NeedBaseline
            }
        } else {
            // threshold0 means a mismatch resets accumulated expected reads.
            // Bound bad input because upstream increments it without saturation.
            (self.count, self.bad) = minz_core::am32::bemf_count_step(
                self.count,
                self.bad.min(u16::MAX - 1),
                level == expected,
                0,
            );
            if self.count >= 2 {
                self.latched = true;
                Reason::Candidate
            } else if level != expected {
                Reason::Opposite
            } else {
                Reason::Accumulating
            }
        };
        Decision {
            expected,
            armed: self.armed,
            event: reason == Reason::Candidate,
            latched: self.latched,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn already_expected_is_not_a_crossing() {
        let mut d = Detector::default();
        for _ in 0..100 {
            assert_eq!(d.update(1, true, true).reason, Reason::NeedBaseline);
        }
    }
    #[test]
    fn polarity_and_single_latch() {
        for step in 1..=6 {
            let expected = minz_core::drive::edges_for(3, step - 1).0;
            let mut d = Detector::default();
            assert!(!d.update(step, !expected, true).armed);
            assert!(d.update(step, !expected, true).armed);
            assert_eq!(d.update(step, expected, true).reason, Reason::Accumulating);
            assert!(d.update(step, expected, true).event);
            assert_eq!(d.update(step, expected, true).reason, Reason::Latched);
        }
    }
    #[test]
    fn gap_and_step_reset_support() {
        let mut d = Detector::default();
        d.update(1, false, true);
        d.update(1, false, true);
        assert_eq!(d.update(1, true, false).reason, Reason::Invalid);
        assert_eq!(d.update(1, true, true).reason, Reason::NeedBaseline);
        d.update(1, false, true);
        d.update(1, false, true);
        assert_eq!(d.update(2, false, true).reason, Reason::NeedBaseline);
        assert_eq!(d.update(0, false, true).reason, Reason::Invalid);
    }
    #[test]
    fn spike_does_not_complete_candidate() {
        let mut d = Detector::default();
        for level in [false, false, true, false, true] {
            assert!(!d.update(1, level, true).event);
        }
        assert!(d.update(1, true, true).event);
    }
}
