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
//! * **The arm is atomic** (E182). Ordering and a latch were not enough: the
//!   decision is two flags, the enable is a separate write, and there are two
//!   callers, so a guard trip *or* the other caller could land inside the
//!   sequence. `roots::com_arm` therefore runs its whole body — decision and
//!   writes — inside one critical section, and [`Atomicity`] lets the model
//!   express both that shape and the defective one.
//!
//! **Zero observed violations is not proof**, which is why this is a model
//! with interleaving tests and a shared decision, not a counter. And a model
//! that cannot fail the broken firmware proves nothing either: the pre-run
//! review of the 50% cohort found that the *defective* unmasked recheck passed
//! every test here unchanged (E181 §3.5). Both hazards are now run against
//! both variants, and the tests assert that the interruptible one is unsafe.
//!
//! The firmware runs the same decision and the same order; [`ARM_ORDER`] is
//! the order its straight-line code follows, `the_firmware_order_is_the_tested_order`
//! pins the list, and [`FIRMWARE_ARM_IS_ATOMIC`] pins the critical section.
//!
//! **One modelled divergence, deliberately kept** (E181 §3.6): the firmware's
//! resumed arm re-stores `phase` after `com_stop` has zeroed it, so it can end
//! with a stale non-zero purpose and the timer off, where the model's `stop`
//! leaves `purpose` at 0. It is harmless — `com_root` returns on `!active`
//! before it reads the purpose, and nothing can dispatch with the timer off —
//! but it is a difference between the model and the code, and an undocumented
//! one is how the last two of these were missed.

/// Is the firmware's arm — its decision *and* all of its writes — atomic with
/// respect to the contexts that can interleave with it?
///
/// `true` since E182: `roots::com_arm` runs its whole body inside
/// `cortex_m::interrupt::free`. This constant exists because the model below
/// can express both, and the pre-run review of the 50% cohort found that it
/// could not previously: with the enable's latch recheck written *unmasked* —
/// the defective firmware — every test in this file still passed (E181 §3.5).
/// A model that cannot fail the broken version proves nothing, so
/// [`Atomicity`] is now a parameter, the interleavings are run against both,
/// and the tests assert that the unmasked variant *is* unsafe.
///
/// If `roots::com_arm` ever loses its critical section, flip this to `false`
/// and `the_firmware_is_the_safe_variant` fails.
pub const FIRMWARE_ARM_IS_ATOMIC: bool = true;

/// Whether an arm can be interleaved by the guard root or by the other caller.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Atomicity {
    /// The firmware's: decision and writes inside one critical section.
    Atomic,
    /// The defective shape, kept so the tests can show it failing: the writes
    /// are open to a stop, or to a second arm, landing between them.
    Interruptible,
}

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
    /// Which firmware shape this instance models.
    pub atomicity: Atomicity,
    /// The purpose a firing would be served as; 0 is "nothing armed".
    pub purpose: u32,
    /// The instant the bookkeeping says the firing is for.
    pub scheduled_at: u32,
    /// The instant the *hardware* will actually fire at: the reload, written
    /// at the enable, against the clock the enable read. Two writers landing
    /// inside one arm are visible here and nowhere else — the bookkeeping ends
    /// up describing one arm and the reload the other (E181 §3.4).
    pub fires_at: u32,
    /// Can the timer raise an interrupt?
    pub can_fire: bool,
}

impl OneShot {
    /// The firmware's shape: an atomic arm.
    #[must_use]
    pub const fn idle() -> Self {
        Self::with_atomicity(Atomicity::Atomic)
    }

    #[must_use]
    pub const fn with_atomicity(atomicity: Atomicity) -> Self {
        Self {
            stop: Stop::new(),
            active: true,
            atomicity,
            purpose: 0,
            scheduled_at: 0,
            fires_at: 0,
            can_fire: false,
        }
    }

    /// Can another context observe — or write — this arm half finished?
    #[must_use]
    pub const fn interleavable(&self) -> bool {
        matches!(self.atomicity, Atomicity::Interruptible)
    }

    /// **The interleaving the critical section exists to forbid**: a stop
    /// landing between the arm's decision and its enable.
    ///
    /// Returns whether the interleaving was *reachable* at all. On an atomic
    /// arm it is not, and nothing happens — the guard runs either before the
    /// decision (which then refuses) or after the enable (and `com_stop`
    /// turns the timer off itself). On an interruptible arm it is reachable,
    /// and the enable writes on a decision that is already stale, which is
    /// precisely the defect.
    pub fn stop_inside_the_arm(&mut self, at: u32, us: u32, purpose: u32) -> bool {
        if !arm_allowed(self.stop.latched(), self.active) {
            return false;
        }
        if !self.interleavable() {
            return false;
        }
        // The decision has passed; the guard lands here.
        self.stop();
        // ... and the rest of the arm runs against it.
        self.scheduled_at = at.wrapping_add(us);
        self.purpose = purpose;
        self.fires_at = at.wrapping_add(us);
        self.can_fire = true;
        true
    }

    /// **The two-writer interleaving** (E181 §3.4): the foreground's handover
    /// arms while the detector is live, so COMP's own arm can land inside it.
    ///
    /// Returns whether the mixed state was reachable. An atomic arm cannot be
    /// interleaved, so both arms complete in some order and the state belongs
    /// to one of them. An interruptible arm ends with the schedule and purpose
    /// of one writer and the enable of the other.
    pub fn arm_interleaved_by_another_arm(&mut self, a: (u32, u32, u32), b: (u32, u32, u32)) -> bool {
        if !self.interleavable() {
            // Serialised: A then B, and the state is entirely B's.
            let _ = self.arm(a.0, a.1, a.2);
            let _ = self.arm(b.0, b.1, b.2);
            return false;
        }
        // A gets as far as its stores; B arms completely; A then enables.
        let _ = self.arm_partial(a.0, a.1, a.2, 3);
        let _ = self.arm(b.0, b.1, b.2);
        self.arm_resume(a.0, a.1, a.2, 3);
        true
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
                ArmStep::ConfigureAndEnable => {
                    self.fires_at = at.wrapping_add(us);
                    self.can_fire = !self.stop.latched();
                }
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
                ArmStep::ConfigureAndEnable => {
                    self.fires_at = at.wrapping_add(us);
                    self.can_fire = !self.stop.latched();
                }
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

    /// Do the bookkeeping and the hardware agree about when the firing is for?
    /// Only meaningful while something is armed.
    #[must_use]
    pub const fn consistent(&self) -> bool {
        !self.can_fire || self.fires_at == self.scheduled_at
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

    /// **The test the suite was missing**: it fails the defective firmware.
    ///
    /// E181 §3.5 found that an unmasked latch recheck — the shape the binz
    /// reviewer caught — passed every test in this file unchanged, because the
    /// model had no concurrency to express. It does now, and the two variants
    /// come out differently.
    #[test]
    fn a_stop_inside_an_interruptible_arm_leaves_the_timer_live() {
        let mut bad = OneShot::with_atomicity(Atomicity::Interruptible);
        assert!(
            bad.stop_inside_the_arm(1_000, 50, 1),
            "the interleaving must be reachable on the defective shape"
        );
        // This is the defect, asserted rather than described: a latched stop,
        // and a timer that will fire anyway, after the bridge is dead.
        assert!(bad.stop.latched());
        assert!(bad.can_fire, "the whole point: the stop was lost");
        assert_eq!(bad.dispatch(), Some(1));
    }

    #[test]
    fn a_stop_cannot_land_inside_an_atomic_arm_at_all() {
        let mut good = OneShot::idle();
        assert!(
            !good.stop_inside_the_arm(1_000, 50, 1),
            "an atomic arm admits no interleaving"
        );
        // Whichever side of the critical section the guard lands on, the timer
        // ends off: before it, the decision refuses; after it, the stop runs.
        let mut before = OneShot::idle();
        before.stop();
        assert!(!before.arm(1_000, 50, 1));
        assert_eq!(before.dispatch(), None);
        let mut after = OneShot::idle();
        assert!(after.arm(1_000, 50, 1));
        after.stop();
        assert!(!after.can_fire);
        assert_eq!(after.dispatch(), None);
    }

    /// The two-writer window (E181 §3.4): the foreground's handover arms while
    /// the detector is live, so COMP's arm can land inside it.
    #[test]
    fn two_arms_in_flight_cannot_mix_unless_the_arm_is_interruptible() {
        let a = (1_000_u32, 50_u32, 1_u32);
        let b = (1_010_u32, 90_u32, 3_u32);

        let mut bad = OneShot::with_atomicity(Atomicity::Interruptible);
        assert!(bad.arm_interleaved_by_another_arm(a, b));
        assert!(
            !bad.consistent(),
            "the defective shape must be able to end with one arm's schedule \
             and the other's reload"
        );

        let mut good = OneShot::idle();
        assert!(!good.arm_interleaved_by_another_arm(a, b));
        assert!(good.consistent(), "an atomic arm always ends describing one arm");
        assert_eq!(good.purpose, b.2, "serialised: the later arm wins outright");
        assert_eq!(good.scheduled_at, b.0 + b.1);
        assert_eq!(good.fires_at, good.scheduled_at);
    }

    #[test]
    fn every_interleaving_keeps_an_atomic_arm_consistent() {
        // The exhaustive version of the two tests above, over both hazards and
        // every prefix: an atomic arm is never left describing two firings.
        for done in 0..=ARM_ORDER.len() {
            let mut os = OneShot::idle();
            assert!(os.arm_partial(1_000, 50, 1, done));
            os.stop();
            os.arm_resume(1_000, 50, 1, done);
            assert!(os.consistent(), "done={done}");
            assert!(!os.can_fire, "done={done}");
        }
    }

    #[test]
    fn the_firmware_is_the_safe_variant() {
        // `roots::com_arm` runs its whole body inside `interrupt::free`, so the
        // shape the tests above prove safe is the one that is compiled. This
        // test runs both hazards against *whatever shape the constant names*,
        // so flipping the constant fails it rather than merely documenting a
        // regression; the thing to re-check by reading `roots.rs` is the
        // constant itself.
        let shape = if FIRMWARE_ARM_IS_ATOMIC {
            Atomicity::Atomic
        } else {
            Atomicity::Interruptible
        };
        let mut os = OneShot::with_atomicity(shape);
        assert!(
            !os.stop_inside_the_arm(1_000, 50, 1),
            "com_arm must arm inside a critical section: no stop may land inside the arm"
        );
        let mut two = OneShot::with_atomicity(shape);
        assert!(
            !two.arm_interleaved_by_another_arm((1_000, 50, 1), (1_010, 90, 3)),
            "com_arm must arm inside a critical section: no second arm may land inside one"
        );
        assert!(two.consistent());
    }

    /// **The constant is now derived from the source, not asserted beside it.**
    ///
    /// E186 SS2 found the hole: `FIRMWARE_ARM_IS_ATOMIC` was a hand-written
    /// literal, so deleting the `interrupt::free` from `roots::com_arm` left
    /// every test in this file passing. The safety-relevant direction --
    /// firmware made unsafe without anyone flipping the constant -- was as
    /// uncaught as before, and E182 claimed credit in the wrong direction.
    ///
    /// This reads `roots.rs` at compile time and checks the body of `com_arm`
    /// itself. Crude, and it would have caught all three of the shutdown
    /// defects this campaign found by other means.
    #[test]
    fn the_constant_is_read_off_com_arm_itself() {
        let src = include_str!("roots.rs");
        let start = src.find("pub fn com_arm(").expect("com_arm must exist in roots.rs");
        // The body ends at the next item at column zero.
        let rest = &src[start..];
        let end = rest[1..]
            .find(
                "
pub ",
            )
            .map_or(rest.len(), |i| i + 1);
        let body = &rest[..end];
        let atomic = body.contains("cortex_m::interrupt::free");
        assert_eq!(
            atomic, FIRMWARE_ARM_IS_ATOMIC,
            "com_arm's critical section and FIRMWARE_ARM_IS_ATOMIC disagree:              the arm is {} in roots.rs but the constant says {}",
            if atomic { "atomic" } else { "interruptible" },
            FIRMWARE_ARM_IS_ATOMIC
        );
        // And the decision inside it is the shared one, not a re-implementation.
        assert!(
            body.contains("arm_allowed("),
            "com_arm must ask `oneshot::arm_allowed`, not decide for itself"
        );
    }

    /// `ARM_ORDER` against the source too, for the same reason: the old test
    /// compared the constant with its own literal (E186 SS2).
    #[test]
    fn the_firmware_writes_the_tested_order() {
        let src = include_str!("roots.rs");
        let start = src.find("pub fn com_arm(").unwrap();
        let rest = &src[start..];
        let end = rest[1..]
            .find(
                "
pub ",
            )
            .map_or(rest.len(), |i| i + 1);
        let body = &rest[..end];
        // The four steps, as the straight-line code writes them: disarm, stamp
        // the schedule, store the purpose, enable.
        let marks = ["disable_interrupt()", "sched_raw", ".phase.store(", "com_timer::arm("];
        let mut at = 0usize;
        for (i, m) in marks.iter().enumerate() {
            let found = body[at..]
                .find(m)
                .unwrap_or_else(|| panic!("com_arm is missing step {i}: {m}"));
            at += found + m.len();
        }
        assert_eq!(ARM_ORDER.len(), marks.len());
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
