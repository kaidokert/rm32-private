//! Foreground-only average-current foldback policy.
//!
//! The DMA producer publishes only a warning after one complete over-limit
//! block. This policy converts that warning into a lower live-duty ceiling;
//! it has no timer, bridge, guard, or current-threshold authority.
/// Severity-scaled foreground actuator. A marginal over-window loses only1%;
/// a severe excursion loses at most5%. This remains much slower than the
/// electrical backstops and never changes their thresholds or second-window
/// terminal stop.
pub const MIN_STEP_TENTHS: u32 = 10;
pub const MAX_STEP_TENTHS: u32 = 50;
pub const MIN_DUTY_TENTHS: u32 = super::duty_envelope::MIN;
pub const MAX_DUTY_TENTHS: u32 = super::duty_envelope::MAX;

pub struct Governor {
    ceiling: u32,
    reductions: u32,
    #[cfg(feature = "bench-bus-recovery")]
    requested: u32,
    #[cfg(feature = "bench-bus-recovery")]
    release_at: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reduction {
    pub duty: u32,
    pub step: u32,
}

impl Governor {
    pub const fn new() -> Self {
        Self {
            ceiling: MAX_DUTY_TENTHS,
            reductions: 0,
            #[cfg(feature = "bench-bus-recovery")]
            requested: MIN_DUTY_TENTHS,
            #[cfg(feature = "bench-bus-recovery")]
            release_at: 0,
        }
    }

    /// Convert a first-over warning into a monotonically lower ceiling.
    /// At the minimum there is no weaker command to apply; the existing
    /// second-over hard stop remains the authority.
    pub fn warning(&mut self, current: u32, residual: u32, allowance: u32) -> Option<Reduction> {
        if !(MIN_DUTY_TENTHS..=MAX_DUTY_TENTHS).contains(&current)
            || allowance == 0
            || residual <= allowance
        {
            return None;
        }
        // Cross-products keep the M0 path division-free. All policy operands
        // are bounded ADC block sums, so these products fit u32 comfortably.
        let step = if residual * 10 <= allowance * 11 {
            10 // <=110%
        } else if residual * 4 <= allowance * 5 {
            20 // <=125%
        } else if residual * 2 <= allowance * 3 {
            30 // <=150%
        } else if residual * 4 <= allowance * 7 {
            40 // <=175%
        } else {
            50
        };
        let next = current.saturating_sub(step).max(MIN_DUTY_TENTHS);
        if next >= current {
            return None;
        }
        self.ceiling = self.ceiling.min(next);
        self.reductions = self.reductions.saturating_add(1);
        Some(Reduction {
            duty: self.ceiling,
            step,
        })
    }

    #[cfg(feature = "bench-bus-recovery")]
    pub fn bus_warning(&mut self, current: u32, now: u32) -> Option<Reduction> {
        if !(MIN_DUTY_TENTHS..=MAX_DUTY_TENTHS).contains(&current) {
            return None;
        }
        let next = current.saturating_sub(MIN_STEP_TENTHS).max(MIN_DUTY_TENTHS);
        if next >= current {
            return None;
        }
        self.ceiling = self.ceiling.min(next);
        self.reductions = self.reductions.saturating_add(1);
        self.release_at = now.wrapping_add(2_000_000);
        Some(Reduction {
            duty: self.ceiling,
            step: MIN_STEP_TENTHS,
        })
    }

    /// A host/restart request stays authoritative below the learned ceiling;
    /// an upward request cannot silently defeat current foldback in-session.
    pub fn request(&mut self, requested: u32) -> Option<u32> {
        if !(MIN_DUTY_TENTHS..=MAX_DUTY_TENTHS).contains(&requested) {
            return None;
        }
        #[cfg(feature = "bench-bus-recovery")]
        {
            self.requested = requested;
        }
        Some(requested.min(self.ceiling))
    }

    #[cfg(feature = "bench-bus-recovery")]
    pub fn arm_recovery(&mut self, now: u32) {
        self.release_at = now.wrapping_add(2_000_000);
    }

    /// Foreground-only1%/s release after the quiet hold. Host intent is
    /// remembered, but every step still uses the guarded live-duty writer.
    #[cfg(feature = "bench-bus-recovery")]
    pub fn poll_recovery(&mut self, now: u32, current: u32) -> Option<u32> {
        if self.release_at == 0
            || now.wrapping_sub(self.release_at) >= 0x8000_0000
            || current >= self.requested
        {
            return None;
        }
        let next = current.saturating_add(10).min(self.requested);
        self.ceiling = next;
        if next == self.requested {
            self.release_at = 0;
            self.ceiling = MAX_DUTY_TENTHS;
        } else {
            self.release_at = now.wrapping_add(1_000_000);
        }
        Some(next)
    }

    pub const fn ceiling(&self) -> u32 {
        self.ceiling
    }

    pub const fn reductions(&self) -> u32 {
        self.reductions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_scale_with_severity_and_never_release_upward() {
        let mut governor = Governor::new();
        assert_eq!(
            governor.warning(MAX_DUTY_TENTHS, 101, 100),
            Some(Reduction {
                duty: MAX_DUTY_TENTHS - 10,
                step: 10
            })
        );
        assert_eq!(
            governor.request(MAX_DUTY_TENTHS),
            Some(MAX_DUTY_TENTHS - 10)
        );
        assert_eq!(governor.request(200), Some(200));
        assert_eq!(
            governor.warning(200, 185, 100),
            Some(Reduction {
                duty: 150,
                step: 50
            })
        );
        assert_eq!(governor.request(250), Some(150));
        assert_eq!((governor.ceiling(), governor.reductions()), (150, 2));
    }

    #[test]
    fn severity_boundaries_are_exact_and_division_free() {
        let expected = [
            (101, 10),
            (110, 10),
            (111, 20),
            (125, 20),
            (126, 30),
            (150, 30),
            (151, 40),
            (175, 40),
            (176, 50),
        ];
        for (residual, step) in expected {
            let mut governor = Governor::new();
            assert_eq!(
                governor.warning(MAX_DUTY_TENTHS, residual, 100),
                Some(Reduction {
                    duty: MAX_DUTY_TENTHS - step,
                    step
                })
            );
        }
    }

    #[test]
    fn retained_e803_excursion_demands_bounded_maximum_step() {
        if MAX_DUTY_TENTHS < 380 {
            return; // E803 belongs to the explicit50% envelope build.
        }
        let mut governor = Governor::new();
        assert_eq!(
            governor.warning(380, 28_065, 15_145),
            Some(Reduction {
                duty: 330,
                step: 50
            })
        );
    }

    #[test]
    fn minimum_leaves_terminal_second_window_unchanged() {
        let mut governor = Governor::new();
        assert_eq!(
            governor.warning(50, 101, 100),
            Some(Reduction { duty: 40, step: 10 })
        );
        assert_eq!(governor.warning(40, 101, 100), None);
        assert_eq!(governor.request(MAX_DUTY_TENTHS), Some(40));
        assert_eq!(governor.reductions(), 1);
    }

    #[test]
    fn invalid_values_are_refused_not_clamped_into_authority() {
        let mut governor = Governor::new();
        for value in [0, MIN_DUTY_TENTHS - 1, MAX_DUTY_TENTHS + 1, u32::MAX] {
            assert_eq!(governor.warning(value, 101, 100), None);
            assert_eq!(governor.request(value), None);
        }
        assert_eq!(governor.warning(100, 100, 100), None);
        assert_eq!(governor.warning(100, 101, 0), None);
        assert_eq!(
            (governor.ceiling(), governor.reductions()),
            (MAX_DUTY_TENTHS, 0)
        );
    }

    #[cfg(feature = "bench-bus-recovery")]
    #[test]
    fn corroborated_backoff_recovers_only_after_hold_at_one_percent_per_second() {
        let mut governor = Governor::new();
        assert_eq!(governor.request(450), Some(450));
        assert_eq!(governor.bus_warning(400, 100).unwrap().duty, 390);
        assert_eq!(governor.request(450), Some(390));
        assert_eq!(governor.poll_recovery(2_000_099, 390), None);
        assert_eq!(governor.poll_recovery(2_000_100, 390), Some(400));
        assert_eq!(governor.poll_recovery(3_000_100, 400), Some(410));
        assert_eq!(governor.request(300), Some(300));
        assert_eq!(governor.poll_recovery(4_000_100, 300), None);
    }
}
