//! The commutation one-shot's arm/stop discipline, as testable logic
//! (campaign 9 step 2).
//!
//! Two hazards were identified by review in campaign 8 and neither was
//! structural — both rested on an invariant nobody had written down:
//!
//! 1. **A half-written arm.** `com_arm` stamps the schedule, stores the
//!    timer's purpose and then configures the timer. With COM at a higher
//!    priority than COMP (the `com-top` A/B) a dispatch from a *previously*
//!    armed one-shot could land between the stores and the configuration, so
//!    the handler would run the new purpose against the old firing.
//! 2. **An arm that outlives a stop.** A protection latch runs from the guard
//!    root, above COMP, and calls `com_stop`. If it interrupts COMP in the
//!    middle of an acceptance, COMP's own `com_arm` then runs *after* the
//!    bridge has been de-energised and re-creates timer activity — a
//!    commutation scheduled after a shutdown.
//!
//! The answers are ordering and a latch, and both live here so they can be
//! tested by interleaving on the host rather than argued about:
//!
//! * **Disarm first.** The sequence is [`ARM_ORDER`]: take the timer's
//!   interrupt away, *then* stamp and store, *then* configure and enable.
//!   Nothing can dispatch while the bookkeeping is inconsistent, because
//!   between the first step and the last the timer cannot raise an interrupt
//!   at all.
//! * **A stop latches.** [`Stop::latch`] refuses every later arm until the
//!   foreground hands the loop over again. `com_arm` asks [`arm_allowed`]
//!   before it touches anything, so "interrupted work must not recreate timer
//!   activity after shutdown" is a property of one function rather than of
//!   whichever caller remembered to check.
//!
//! **Zero observed violations is not proof**, which is why this is a model
//! with interleaving tests and a shared decision, not a counter. The firmware
//! runs the same decision and the same order; [`ARM_ORDER`] is the order its
//! straight-line code follows, and `the_firmware_order_is_the_tested_order`
//! pins the list.

/// May the one-shot be armed at all?
///
/// The one place that decides, called by `roots::com_arm` before it writes
/// anything: a latched stop refuses, and so does an inactive loop.
#[inline(always)]
#[must_use]
pub const fn arm_allowed(stopped: bool, active: bool) -> bool {
    active && !stopped
}

/// One step of the arm sequence. The order is the safety property.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ArmStep {
    /// Take the update interrupt away, so nothing can dispatch mid-sequence.
    Disarm,
    /// Stamp the instant the firing is scheduled for.
    StampSchedule,
    /// Store what the firing is *for* (the timer's purpose).
    SetPurpose,
    /// Load the reload, clear the flag, enable the interrupt and the counter.
    ConfigureAndEnable,
}

/// The order `roots::com_arm` writes in, and the order the tests interleave
/// against. `Disarm` first and `ConfigureAndEnable` last are the two that
/// matter; the middle pair may be in either order and are written in this one.
pub const ARM_ORDER: [ArmStep; 4] = [
    ArmStep::Disarm,
    ArmStep::StampSchedule,
    ArmStep::SetPurpose,
    ArmStep::ConfigureAndEnable,
];

/// Whether a stop has latched. Cleared only by the foreground's handover.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Stop {
    latched: bool,
}

impl Stop {
    #[must_use]
    pub const fn new() -> Self {
        Self { latched: false }
    }

    /// A protection or foreground stop: every later arm is refused.
    #[inline]
    pub fn latch(&mut self) {
        self.latched = true;
    }

    /// The foreground hands the loop over again.
    #[inline]
    pub fn release(&mut self) {
        self.latched = false;
    }

    #[must_use]
    pub const fn latched(&self) -> bool {
        self.latched
    }
}

/// A model of the one-shot as the two roots can observe it, for interleaving
/// tests. It is not a register map: it records only what the hazards are about
/// — whether the timer can fire, and what a firing would be taken to mean.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct OneShot {
    pub stop: Stop,
    pub active: bool,
    /// The purpose a firing would be served as; 0 is "nothing armed".
    pub purpose: u32,
    pub scheduled_at: u32,
    /// Can the timer raise an interrupt?
    pub can_fire: bool,
}

impl OneShot {
    #[must_use]
    pub const fn idle() -> Self {
        Self {
            stop: Stop::new(),
            active: true,
            purpose: 0,
            scheduled_at: 0,
            can_fire: false,
        }
    }

    /// Run the arm sequence, stopping after `steps` of it — so a test can
    /// interleave a stop at every point inside the sequence.
    pub fn arm_partial(&mut self, at: u32, us: u32, purpose: u32, steps: usize) -> bool {
        if !arm_allowed(self.stop.latched(), self.active) {
            return false;
        }
        for step in ARM_ORDER.iter().take(steps) {
            match step {
                ArmStep::Disarm => self.can_fire = false,
                ArmStep::StampSchedule => self.scheduled_at = at.wrapping_add(us),
                ArmStep::SetPurpose => self.purpose = purpose,
                // The enable re-checks the latch, because a stop can land
                // between the first check and this step; in the firmware that
                // recheck is inside a critical section so the guard cannot
                // interleave with it.
                ArmStep::ConfigureAndEnable => self.can_fire = !self.stop.latched(),
            }
        }
        true
    }

    /// The whole sequence.
    pub fn arm(&mut self, at: u32, us: u32, purpose: u32) -> bool {
        self.arm_partial(at, us, purpose, ARM_ORDER.len())
    }

    /// **Resume an arm that was interrupted after `done` steps** -- the case
    /// the first version of these tests never exercised: a stop that lands
    /// mid-sequence and then the arm's *remaining* writes running. The
    /// original check has already passed, so this deliberately skips it.
    pub fn arm_resume(&mut self, at: u32, us: u32, purpose: u32, done: usize) {
        for step in ARM_ORDER.iter().skip(done) {
            match step {
                ArmStep::Disarm => self.can_fire = false,
                ArmStep::StampSchedule => self.scheduled_at = at.wrapping_add(us),
                ArmStep::SetPurpose => self.purpose = purpose,
                ArmStep::ConfigureAndEnable => self.can_fire = !self.stop.latched(),
            }
        }
    }

    /// A stop, from the guard root or the foreground: the timer goes quiet,
    /// nothing is armed, and the latch refuses every later arm.
    pub fn stop(&mut self) {
        self.can_fire = false;
        self.purpose = 0;
        self.stop.latch();
    }

    /// What a dispatch would be served as, if one can happen at all.
    #[must_use]
    pub const fn dispatch(&self) -> Option<u32> {
        if self.can_fire && self.purpose != 0 {
            Some(self.purpose)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_at_every_point_inside_the_arm_leaves_the_timer_quiet() {
        // The interleaving the review asked for: a protection latch landing
        // after each step of the sequence. Whatever it interrupts, the timer
        // must end unable to fire and nothing must be armed.
        for steps in 0..=ARM_ORDER.len() {
            let mut os = OneShot::idle();
            assert!(os.arm_partial(1_000, 50, 1, steps));
            os.stop();
            assert!(!os.can_fire, "steps={steps}");
            assert_eq!(os.purpose, 0, "steps={steps}");
            assert_eq!(os.dispatch(), None, "steps={steps}");
        }
    }

    /// The interleaving that matters, and the one the first version missed: a
    /// stop lands *inside* the arm, and then the arm's remaining writes run.
    /// The timer must still end unable to fire — the enable re-checks the
    /// latch, which in the firmware is a masked recheck so the guard cannot
    /// interleave with it (found by the binz reviewer against E173/E174).
    #[test]
    fn an_arm_interrupted_by_a_stop_does_not_enable_the_timer_when_it_resumes() {
        for done in 0..=ARM_ORDER.len() {
            let mut os = OneShot::idle();
            assert!(os.arm_partial(1_000, 50, 1, done));
            os.stop();
            os.arm_resume(1_000, 50, 1, done);
            assert!(
                !os.can_fire,
                "resumed after a stop at step {done} and the timer went live"
            );
            assert_eq!(os.dispatch(), None, "step {done}");
        }
    }

    #[test]
    fn interrupted_work_cannot_recreate_timer_activity_after_a_stop() {
        // COMP is mid-acceptance, the guard trips, and COMP then arms. The
        // arm must be refused -- this is the hazard that had no structure.
        let mut os = OneShot::idle();
        os.stop();
        assert!(!os.arm(2_000, 50, 1), "an arm after a latched stop must be refused");
        assert!(!os.can_fire);
        assert_eq!(os.dispatch(), None);
        // ... and it stays refused until the foreground hands over again.
        for _ in 0..3 {
            assert!(!os.arm(2_000, 50, 1));
        }
        os.stop.release();
        assert!(os.arm(2_000, 50, 1));
        assert_eq!(os.dispatch(), Some(1));
    }

    #[test]
    fn a_dispatch_inside_the_sequence_can_never_see_a_half_written_arm() {
        // Disarm first, enable last: at every intermediate point either the
        // timer cannot fire, or the purpose and schedule already belong to
        // this arm. A dispatch in between is impossible, and the test says so
        // for every prefix.
        let mut os = OneShot::idle();
        assert!(os.arm(0, 10, 2));
        assert_eq!(os.dispatch(), Some(2));
        for steps in 1..ARM_ORDER.len() {
            let mut mid = os;
            assert!(mid.arm_partial(5_000, 40, 1, steps));
            assert_eq!(mid.dispatch(), None, "steps={steps} could dispatch mid-sequence");
        }
        // Only the last step makes it live, and then everything is consistent.
        let mut done = os;
        assert!(done.arm(5_000, 40, 1));
        assert_eq!(done.dispatch(), Some(1));
        assert_eq!(done.scheduled_at, 5_040);
    }

    #[test]
    fn an_inactive_loop_refuses_an_arm_without_latching() {
        let mut os = OneShot::idle();
        os.active = false;
        assert!(!os.arm(0, 10, 1));
        assert!(!os.stop.latched(), "refusing is not latching");
        os.active = true;
        assert!(os.arm(0, 10, 1));
    }

    #[test]
    fn the_firmware_order_is_the_tested_order() {
        // `roots::com_arm` writes straight-line in this order; if it is ever
        // reordered, this list is what the reviewer checks it against.
        assert_eq!(
            ARM_ORDER,
            [
                ArmStep::Disarm,
                ArmStep::StampSchedule,
                ArmStep::SetPurpose,
                ArmStep::ConfigureAndEnable
            ]
        );
        assert_eq!(ARM_ORDER[0], ArmStep::Disarm, "nothing may dispatch mid-sequence");
        assert_eq!(
            *ARM_ORDER.last().unwrap(),
            ArmStep::ConfigureAndEnable,
            "the timer goes live last"
        );
    }

    #[test]
    fn arm_allowed_is_the_only_rule() {
        assert!(arm_allowed(false, true));
        assert!(!arm_allowed(true, true), "a latched stop refuses");
        assert!(!arm_allowed(false, false), "an inactive loop refuses");
        assert!(!arm_allowed(true, false));
    }
}
