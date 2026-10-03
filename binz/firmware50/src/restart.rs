//! Normal-start recovery: one ordinary restart after a tracking loss.
//!
//! Transcribed from the qualified image's policy
//! (`binz/NORMAL_RESTART_E759.md`, `examples/support/normal_restart.rs`):
//!
//! - only a **tracking** stop (reason 8) may request a restart; every other
//!   fault (current, bus, driver, deadline, feedback, execution) stays latched;
//! - the attempt is **one-shot** and is consumed even when refused;
//! - the whole ordinary startup plus a dispatch reserve must fit in what is
//!   left of the window;
//! - the drive time handed back is the **remainder of the original window**,
//!   never a fresh one;
//! - the arithmetic is checked, so a wrapped or over-run session is refused.
//!
//! The recovery itself is the reference's too (`NORMAL_RESTART_E770.md`): the
//! bridge off for a full second, a fresh same-wake zero, the average-current
//! protection reinstalled, the ordinary sine staircase with the handover
//! settings replayed, and a fresh handover. No flying reacquisition. That
//! sequencing lives in the binary; this module only decides admission.

use crate::protection::Reason;

/// Dispatch reserve on top of the caller's stated need, µs (the reference's
/// "plus 200 us dispatch reserve").
pub const RESERVE_US: u32 = 200;

/// Why a restart was refused.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The stop was not a tracking loss; it stays latched.
    NotTracking,
    /// The one attempt was already used.
    AlreadyUsed,
    /// What is left of the window cannot hold the startup and the reserve.
    NoRoom,
    /// The elapsed time is past the window (or the clock wrapped).
    Overrun,
}

impl Refusal {
    /// Wire code for reports: 0 is reserved for "admitted".
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Refusal::NotTracking => 1,
            Refusal::AlreadyUsed => 2,
            Refusal::NoRoom => 3,
            Refusal::Overrun => 4,
        }
    }
}

/// One-shot admission state for a single powered campaign.
#[derive(Debug)]
pub struct Restart {
    used: bool,
}

impl Default for Restart {
    fn default() -> Self {
        Self::new()
    }
}

impl Restart {
    #[must_use]
    pub const fn new() -> Self {
        Self { used: false }
    }

    /// Has the attempt been consumed?
    #[must_use]
    pub const fn used(&self) -> bool {
        self.used
    }

    /// Decide whether `stop`, `elapsed_us` into a `window_us` campaign, may
    /// restart, given that the recovery needs `need_us` before it can hold
    /// (off time, ordinary startup, ramp back to target).
    ///
    /// On admission returns the drive time left for the second segment: the
    /// remainder of the original window. The attempt is consumed either way.
    pub fn admit(&mut self, stop: Reason, elapsed_us: u32, window_us: u32, need_us: u32) -> Result<u32, Refusal> {
        if self.used {
            return Err(Refusal::AlreadyUsed);
        }
        self.used = true;
        if stop != Reason::Tracking {
            return Err(Refusal::NotTracking);
        }
        let Some(remaining) = window_us.checked_sub(elapsed_us) else {
            return Err(Refusal::Overrun);
        };
        let Some(required) = need_us.checked_add(RESERVE_US) else {
            return Err(Refusal::NoRoom);
        };
        if remaining < required {
            return Err(Refusal::NoRoom);
        }
        Ok(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tracking_stop_returns_the_rest_of_the_original_window() {
        let mut r = Restart::new();
        // 44 s window, tracking lost 14 s in, 13.4 s needed to be back at target.
        assert_eq!(
            r.admit(Reason::Tracking, 14_000_000, 44_000_000, 13_400_000),
            Ok(30_000_000)
        );
        assert!(r.used());
    }

    #[test]
    fn only_one_attempt_per_campaign() {
        let mut r = Restart::new();
        assert!(r.admit(Reason::Tracking, 1_000_000, 44_000_000, 1_000_000).is_ok());
        assert_eq!(
            r.admit(Reason::Tracking, 2_000_000, 44_000_000, 1_000_000),
            Err(Refusal::AlreadyUsed)
        );
    }

    #[test]
    fn every_other_stop_stays_latched_and_still_consumes_the_attempt() {
        for stop in [
            Reason::CampaignDeadline,
            Reason::SegmentDeadline,
            Reason::TickGap,
            Reason::FeedbackStale,
            Reason::Current,
            Reason::Driver,
            Reason::HostAbort,
            Reason::AverageCurrent,
            Reason::FastBusSag,
            Reason::CompStorm,
            Reason::HandlerOverrun,
        ] {
            let mut r = Restart::new();
            assert_eq!(
                r.admit(stop, 1_000_000, 44_000_000, 1_000_000),
                Err(Refusal::NotTracking),
                "{stop:?}"
            );
            // Consumed: a later tracking stop in the same campaign is refused.
            assert_eq!(
                r.admit(Reason::Tracking, 2_000_000, 44_000_000, 1_000_000),
                Err(Refusal::AlreadyUsed)
            );
        }
    }

    #[test]
    fn the_startup_and_reserve_must_fit_exactly() {
        let need = 13_400_000;
        let window = 44_000_000;
        let edge = window - need - RESERVE_US;
        assert_eq!(
            Restart::new().admit(Reason::Tracking, edge, window, need),
            Ok(need + RESERVE_US)
        );
        assert_eq!(
            Restart::new().admit(Reason::Tracking, edge + 1, window, need),
            Err(Refusal::NoRoom)
        );
    }

    #[test]
    fn an_overrun_or_wrapped_session_is_refused() {
        assert_eq!(
            Restart::new().admit(Reason::Tracking, 44_000_001, 44_000_000, 0),
            Err(Refusal::Overrun)
        );
        assert_eq!(
            Restart::new().admit(Reason::Tracking, u32::MAX, 44_000_000, 0),
            Err(Refusal::Overrun)
        );
        assert_eq!(
            Restart::new().admit(Reason::Tracking, 0, 44_000_000, u32::MAX),
            Err(Refusal::NoRoom)
        );
    }

    #[test]
    fn refusal_codes_are_distinct_and_nonzero() {
        let codes = [
            Refusal::NotTracking.code(),
            Refusal::AlreadyUsed.code(),
            Refusal::NoRoom.code(),
            Refusal::Overrun.code(),
        ];
        for (i, a) in codes.iter().enumerate() {
            assert_ne!(*a, 0);
            for b in &codes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
