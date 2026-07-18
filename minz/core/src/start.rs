//! R6 — self-paced polling start (AM32's startup architecture,
//! GAP_CLOSING_PLAN rung R6).
//!
//! AM32 never schedules its start commutations on a clock: it pins
//! duty low (~6 %), points the comparator at the floating phase, and
//! commutates when the comparator LEVEL has agreed with the expected
//! post-ZC polarity for `min_bemf_counts` consecutive polls — the
//! rotor itself paces the sequence, so there is no scheduled instant
//! to miss and no forced-ramp/rotor phase mismatch (our engage
//! lottery's root soil). The first crossings use DOUBLED counts and a
//! wait-skip (the rotor needs a kick before its BEMF is trustworthy).
//! Handoff to the interrupt-mode loop happens only once the polled
//! interval is fast and steady.
//!
//! This module is the pure state machine: the firmware feeds it one
//! comparator sample per control tick plus a µs timestamp; it answers
//! with what to do. All constants are polls (ticks), not µs, so the
//! machine is host-testable at any tick rate; the firmware's tick is
//! TIM7 at ~6 kHz (166 µs/poll).

/// Consecutive agreeing polls required for a steady-state crossing.
/// At 6 kHz polling this is ~500 µs of sustained post-ZC level —
/// deep enough to reject PWM-coupled dwell, shallow enough to track
/// a rotor approaching the handoff band.
pub const MIN_BEMF_COUNTS: u8 = 3;
/// The first crossings double the persistence (AM32: doubled counts
/// while `zero_crosses < 5` — the rotor is barely moving and noise
/// dominates).
pub const EARLY_CROSSINGS: u8 = 5;
/// Wait-skip: after each of the first EARLY_CROSSINGS commutations,
/// ignore the comparator for this many polls — the commutation kick
/// itself rings the floating phase, and the slow rotor cannot
/// legitimately cross again this fast.
pub const EARLY_SKIP_POLLS: u8 = 6;
/// Absolute backstop: if no qualified crossing arrives within this
/// many polls, force a blind commutation (AM32's polling loop also
/// steps blind on its interval timeout; without this a stationary
/// rotor never sequences at all). 6 kHz → ~50 ms per blind step,
/// ~8 s/rev — a crawl, but it breaks symmetric-stall deadlock.
pub const BLIND_STEP_POLLS: u16 = 300;
/// Handoff: the polled commutation interval must be at or below this
/// for `HANDOFF_STREAK` consecutive crossings. 2 500 µs ≈ 67 Hz
/// electrical — comfortably inside the proven engage band, and slow
/// enough that the polling grain (166 µs ≈ 7 % of interval) still
/// measures honestly.
pub const HANDOFF_INTERVAL_US: u32 = 2_500;
/// Consecutive fast crossings required before handing off.
pub const HANDOFF_STREAK: u8 = 4;

/// What the firmware should do after feeding one poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartAction {
    /// Keep polling.
    None,
    /// Commutate to the next sector (rotor-paced crossing accepted).
    Commutate,
    /// Commutate blind (backstop timeout — no crossing seen).
    CommutateBlind,
    /// Crossing accepted AND the interval streak qualifies: commutate,
    /// then hand off to the interrupt-mode loop (seed the estimator
    /// with `interval_us`).
    Handoff { interval_us: u32 },
}

#[derive(Debug, Clone, Copy)]
pub struct StartState {
    /// Consecutive polls agreeing with the expected post-ZC level.
    agree_run: u8,
    /// Polls remaining in the post-commutation wait-skip window.
    skip_left: u8,
    /// Total qualified crossings since `reset`.
    pub crossings: u16,
    /// Polls since the last commutation (any kind).
    polls_since_comm: u16,
    /// Timestamp (µs) of the last qualified crossing, 0 = none yet.
    last_cross_us: u32,
    /// Most recent crossing-to-crossing interval (µs).
    pub interval_us: u32,
    /// Consecutive crossings with interval <= HANDOFF_INTERVAL_US.
    fast_streak: u8,
}

impl StartState {
    pub const fn new() -> Self {
        Self {
            agree_run: 0,
            skip_left: 0,
            crossings: 0,
            polls_since_comm: 0,
            last_cross_us: 0,
            interval_us: 0,
            fast_streak: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Required agreement depth for the current crossing count.
    fn need(&self) -> u8 {
        if self.crossings < EARLY_CROSSINGS as u16 {
            MIN_BEMF_COUNTS * 2
        } else {
            MIN_BEMF_COUNTS
        }
    }

    /// Feed one poll. `level` is the raw comparator VALUE bit;
    /// `expected_post_zc` is the level the floating phase settles at
    /// after the true ZC for the CURRENT sector (zc::expected_post_zc);
    /// `now_us` is a wrapping µs timestamp.
    pub fn poll(&mut self, level: bool, expected_post_zc: bool, now_us: u32) -> StartAction {
        self.polls_since_comm = self.polls_since_comm.saturating_add(1);

        // Post-commutation wait-skip (early crossings only).
        if self.skip_left > 0 {
            self.skip_left -= 1;
            return StartAction::None;
        }

        // Blind backstop: symmetric stall (rotor parked at the
        // equilibrium where the floating phase never crosses).
        if self.polls_since_comm >= BLIND_STEP_POLLS {
            self.commutated(false);
            return StartAction::CommutateBlind;
        }

        if level == expected_post_zc {
            self.agree_run = self.agree_run.saturating_add(1);
            if self.agree_run >= self.need() {
                let interval = if self.last_cross_us != 0 {
                    now_us.wrapping_sub(self.last_cross_us)
                } else {
                    0
                };
                self.last_cross_us = now_us;
                self.interval_us = interval;
                self.crossings = self.crossings.saturating_add(1);
                // Handoff qualification: real interval, fast,
                // streaked — and only past the early phase (early
                // crawl intervals are not lock-worthy evidence).
                if self.crossings > EARLY_CROSSINGS as u16
                    && interval != 0
                    && interval <= HANDOFF_INTERVAL_US
                {
                    self.fast_streak = self.fast_streak.saturating_add(1);
                } else {
                    self.fast_streak = 0;
                }
                let hand = self.fast_streak >= HANDOFF_STREAK;
                self.commutated(true);
                return if hand {
                    StartAction::Handoff {
                        interval_us: interval,
                    }
                } else {
                    StartAction::Commutate
                };
            }
        } else {
            self.agree_run = 0;
        }
        StartAction::None
    }

    fn commutated(&mut self, qualified: bool) {
        self.agree_run = 0;
        self.polls_since_comm = 0;
        if !qualified {
            // A blind step breaks the measured chain: the next
            // crossing's interval is meaningless and must not count
            // toward handoff.
            self.last_cross_us = 0;
            self.fast_streak = 0;
        }
        if self.crossings < EARLY_CROSSINGS as u16 {
            self.skip_left = EARLY_SKIP_POLLS;
        }
    }
}

impl Default for StartState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drive the machine with `agree` consecutive agreeing polls.
    fn feed_agree(s: &mut StartState, n: u32, t: &mut u32) -> Option<StartAction> {
        for _ in 0..n {
            *t += 166;
            let a = s.poll(true, true, *t);
            if a != StartAction::None {
                return Some(a);
            }
        }
        None
    }

    #[test]
    fn early_crossings_need_doubled_counts_and_skip() {
        let mut s = StartState::new();
        let mut t = 0u32;
        // 5 agreeing polls: below the doubled requirement (6) -> none.
        assert_eq!(feed_agree(&mut s, 5, &mut t), None);
        // 6th agreeing poll -> first commutation.
        assert_eq!(feed_agree(&mut s, 1, &mut t), Some(StartAction::Commutate));
        assert_eq!(s.crossings, 1);
        // Wait-skip: the next EARLY_SKIP_POLLS polls are ignored even
        // if they agree.
        for _ in 0..EARLY_SKIP_POLLS {
            t += 166;
            assert_eq!(s.poll(true, true, t), StartAction::None);
        }
        // ...then 6 more agreeing polls commutate again.
        assert_eq!(feed_agree(&mut s, 6, &mut t), Some(StartAction::Commutate));
    }

    #[test]
    fn steady_state_uses_base_counts_no_skip() {
        let mut s = StartState::new();
        let mut t = 0u32;
        // Walk through the early phase.
        while s.crossings < EARLY_CROSSINGS as u16 {
            feed_agree(&mut s, 20, &mut t);
        }
        // Steady state: exactly MIN_BEMF_COUNTS agreeing polls fire.
        assert_eq!(
            feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t),
            Some(StartAction::Commutate)
        );
        // No wait-skip in steady state: a fresh agree-run can start
        // immediately.
        assert_eq!(
            feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t),
            Some(StartAction::Commutate)
        );
    }

    #[test]
    fn disagreeing_poll_resets_run() {
        let mut s = StartState::new();
        let mut t = 0;
        assert_eq!(feed_agree(&mut s, 5, &mut t), None);
        t += 166;
        assert_eq!(s.poll(false, true, t), StartAction::None);
        // The run restarts: 5 more still not enough.
        assert_eq!(feed_agree(&mut s, 5, &mut t), None);
        assert_eq!(feed_agree(&mut s, 1, &mut t), Some(StartAction::Commutate));
    }

    #[test]
    fn blind_backstop_fires_and_breaks_interval_chain() {
        let mut s = StartState::new();
        let mut t = 0u32;
        // Never agree: after BLIND_STEP_POLLS the machine steps blind.
        let mut action = StartAction::None;
        for i in 0..2 * BLIND_STEP_POLLS as u32 {
            t += 166;
            action = s.poll(false, true, t);
            if action != StartAction::None {
                assert!(i + 1 >= BLIND_STEP_POLLS as u32);
                break;
            }
        }
        assert_eq!(action, StartAction::CommutateBlind);
        assert_eq!(s.crossings, 0);
    }

    #[test]
    fn handoff_needs_streak_of_fast_real_intervals() {
        let mut s = StartState::new();
        let mut t = 0u32;
        // Early phase.
        while s.crossings < EARLY_CROSSINGS as u16 {
            feed_agree(&mut s, 20, &mut t);
        }
        // Fast crossings: 3 polls each = ~500 us intervals (fast).
        // The FIRST fast one starts the streak; HANDOFF_STREAK-th
        // hands off.
        let mut actions = heapless_vec();
        for _ in 0..2 * HANDOFF_STREAK {
            if let Some(a) = feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t) {
                let stop = matches!(a, StartAction::Handoff { .. });
                actions.push(a);
                if stop {
                    break;
                }
            }
        }
        assert!(matches!(
            actions.last(),
            Some(StartAction::Handoff { interval_us }) if *interval_us <= HANDOFF_INTERVAL_US
        ));
        // Everything before the handoff was a plain commutation.
        assert!(
            actions[..actions.len() - 1]
                .iter()
                .all(|a| *a == StartAction::Commutate)
        );
    }

    #[test]
    fn slow_interval_resets_handoff_streak() {
        let mut s = StartState::new();
        let mut t = 0u32;
        while s.crossings < EARLY_CROSSINGS as u16 {
            feed_agree(&mut s, 20, &mut t);
        }
        // Two fast crossings...
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        // ...then a slow one (idle polls stretch the interval past
        // the handoff bound; disagreeing polls keep the run at 0).
        for _ in 0..40 {
            t += 166;
            s.poll(false, true, t);
        }
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        // The streak restarted: the next HANDOFF_STREAK-1 fast
        // crossings do NOT hand off...
        for _ in 0..HANDOFF_STREAK - 1 {
            assert_eq!(
                feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t),
                Some(StartAction::Commutate)
            );
        }
        // ...and the one after does.
        assert!(matches!(
            feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t),
            Some(StartAction::Handoff { .. })
        ));
    }

    #[test]
    fn blind_step_never_counts_toward_handoff() {
        let mut s = StartState::new();
        let mut t = 0u32;
        while s.crossings < EARLY_CROSSINGS as u16 {
            feed_agree(&mut s, 20, &mut t);
        }
        // Build a partial fast streak.
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t);
        // Blind timeout (stall): streak must be void afterward.
        loop {
            t += 166;
            if s.poll(false, true, t) == StartAction::CommutateBlind {
                break;
            }
        }
        // The next fast crossing has NO valid reference (interval 0)
        // and cannot hand off; nor can the following streak until it
        // rebuilds fully.
        for _ in 0..HANDOFF_STREAK {
            let a = feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t).unwrap();
            assert_eq!(a, StartAction::Commutate);
        }
        assert!(matches!(
            feed_agree(&mut s, MIN_BEMF_COUNTS as u32, &mut t),
            Some(StartAction::Handoff { .. })
        ));
    }

    // Tiny fixed-capacity vec so the test crate stays dep-free.
    fn heapless_vec() -> Vec<StartAction> {
        Vec::new()
    }
}
