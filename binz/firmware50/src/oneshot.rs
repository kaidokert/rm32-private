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

/// Permission to resume a powered comparator line. Closed-loop ownership
/// takes precedence; acquisition must not bypass its active blanking phase.
/// Phase 4 is a listening recheck, not blanking or an accepted commutation.
#[inline(always)]
#[must_use]
pub const fn comparator_resume_allowed(
    guard: u32, detector: bool, driven: bool, stopped: bool, active: bool, phase: u32,
) -> bool {
    guard == 0 && if detector { arm_allowed(stopped, active) && (phase == 0 || phase == 4) } else { driven }
}

/// Revalidate a sampled sector before committing its estimator update.
/// Only listening phases 0/4 can authorize a fresh acceptance.
#[inline(always)]
pub const fn commit_allowed(stopped: bool, active: bool, detector: bool, phase: u32, sampled_step: u32, current_step: u32) -> bool {
    arm_allowed(stopped, active) && detector && (phase == 0 || phase == 4)
        && sampled_step >= 1 && sampled_step <= 6 && sampled_step == current_step
}

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

/// Remaining entry-relative wait at the final preparation stamp (microseconds).
/// Valid control waits are below half the 16-bit clock modulus. Invalid waits
/// fail closed like an exhausted wait. The caller must establish elapsed time
/// below one clock modulus; limiting wait alone cannot detect a full blackout.
/// The caller still accounts for elapsed time and immediately stops on zero.
#[inline(always)]
pub const fn crossing_left(wait: u32, spent: u32) -> u32 {
    if wait >= 0x8000 { 0 } else { wait.saturating_sub(spent) }
}

/// TIM16 retains the existing two-microsecond minimum for positive remainders.
/// A zero remainder MUST take the stop path, never this reload.
#[inline(always)]
pub const fn crossing_arr(left: u32) -> u16 {
    if left < 2 { 1 } else { (left - 1) as u16 }
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
    #[test]
    fn accepted_commit_requires_same_live_listening_sector() {
        for stopped in [false, true] {
            for active in [false, true] {
                for detector in [false, true] {
                    for phase in 0..=5 {
                        for sampled in 0..=7 {
                            for current in 0..=7 {
                                let expected = !stopped && active && detector
                                    && [0, 4].contains(&phase) && (1..=6).contains(&sampled)
                                    && sampled == current;
                                assert_eq!(super::commit_allowed(stopped, active, detector, phase, sampled, current), expected);
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn comparator_resume_respects_owner_blank_and_stop() {
        use super::comparator_resume_allowed as allowed;
        for guard in [0, 8, 15, 26] {
            for detector in [false, true] {
                for driven in [false, true] {
                    for stopped in [false, true] {
                        for active in [false, true] {
                            for phase in 0..=5 {
                                let expected = guard == 0 && if detector {
                                    !stopped && active && (phase == 0 || phase == 4)
                                } else { driven };
                                assert_eq!(allowed(guard, detector, driven, stopped, active, phase), expected);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn masked_comparator_resume_wins_against_shutdown() {
        use super::comparator_resume_allowed as allowed;
        // before check, between check/write, after write: a guard trip also
        // masks the line. Under PRIMASK its request waits until mask exit.
        let simulate = |masked: bool, stop_at: u8| {
            let mut guard = u32::from(stop_at == 0);
            let permit = allowed(guard, true, false, false, true, 0);
            let mut enabled = false;
            if stop_at == 1 && !masked { guard = 1; enabled = false; }
            if permit { enabled = true; }
            if stop_at == 2 || (stop_at == 1 && masked) { guard = 1; enabled = false; }
            (guard, enabled)
        };
        for at in 0..=2 { assert_eq!(simulate(true, at), (1, false)); }
        assert_eq!(simulate(false, 1), (1, true), "model must expose old race");
    }

    #[test]
    fn powered_line_enable_has_one_masked_writer() {
        let src = include_str!("roots.rs");
        assert_eq!(src.matches("hw::comp::line_enable();").count(), 1);
        let board = include_str!("../bin/board.rs");
        assert!(!board.contains("hw::comp::line_enable()"));
        let deferred = board.split("fn drv_resume_deferred(").nth(1).unwrap()
            .split("fn drv_end(").next().unwrap();
        let mask = deferred.find("cortex_m::interrupt::free").unwrap();
        let check = deferred.find("roots::guard_latched()").unwrap();
        let resume = deferred.find("roots::comp_resume_powered()").unwrap();
        let pend = deferred.find("hw::comp::pend()").unwrap();
        assert!(mask < check && check < resume && resume < pend);
        assert!(deferred[resume..pend].contains("return false"));
        let body = src.split("fn comp_resume_powered()").nth(1).unwrap()
            .split("/// Select this sector").next().unwrap();
        let mask = body.find("cortex_m::interrupt::free").unwrap();
        let check = body.find("comparator_resume_allowed(").unwrap();
        let write = body.find("hw::comp::line_enable();").unwrap();
        assert!(mask < check && check < write);
    }
    use super::*;

    #[test]
    fn entry_relative_wait_exhaustion_minimum_and_wrap() {
        for edge in [0u16, 1, 65_520, u16::MAX] {
            for wait in 0..=2000u32 {
                for elapsed in [0, 1, wait.saturating_sub(1), wait, wait + 1, 4000] {
                    let now = edge.wrapping_add(elapsed as u16);
                    let spent = u32::from(now.wrapping_sub(edge));
                    let left = crossing_left(wait, spent);
                    assert_eq!(left, wait.saturating_sub(elapsed));
                    if left != 0 {
                        assert_eq!(u32::from(crossing_arr(left)) + 1, left.max(2));
                        assert!(elapsed + left.max(2) >= wait);
                    }
                }
            }
        }
        for wait in [0x8000, 0xffff, u32::MAX] {
            assert_eq!(crossing_left(wait, 0), 0, "ambiguous waits fail closed");
        }
        assert_eq!(crossing_arr(crossing_left(0x7fff, 0)), 0x7ffe);
    }

    #[test]
    fn preparation_delay_is_subtracted_not_restarted() {
        let (wait, decision, preparation) = (20, 6, 3);
        let old_fire = decision + preparation + crossing_left(wait, decision);
        let final_stamp = decision + preparation;
        let new_fire = final_stamp + crossing_left(wait, final_stamp);
        assert_eq!(old_fire, 23);
        assert_eq!(new_fire, 20);
        // If preparation consumes the remaining time, do not enable at all.
        assert_eq!(crossing_left(9, final_stamp), 0);
        // Both models deliberately exclude the residual final writes/service.
    }

    #[test]
    fn crossing_arm_source_enforces_stop_atomicity_and_both_callers() {
        let src = include_str!("roots.rs");
        let body = src.split("pub fn com_arm_crossing(").nth(1).unwrap()
            .split("pub fn com_stop(").next().unwrap();
        let marks = ["cortex_m::interrupt::free", "arm_allowed(", "prepare_crossing()",
            "hw::clock::raw()", "crossing_left(wait, spent)", "if left == 0",
            "stop_expired_arm()", "} else {", "sched_raw.store(",
            ".phase.store(1", "start_crossing("];
        let mut pos = 0;
        for mark in marks {
            pos += body[pos..].find(mark).expect(mark) + mark.len();
        }
        assert_eq!(src.matches("arm_marked::<C>(raw, wait)").count(), 2);
        let expired = src.split("fn stop_expired_arm()").nth(1).unwrap()
            .split("pub fn com_stop(").next().unwrap();
        assert!(expired.contains("guard_trip(Reason::LateArm)"));
        // The safety-off primitive called above de-energizes, not just counts.
        let trip = src.split("pub fn guard_trip(").nth(1).unwrap()
            .split("pub fn ").next().unwrap();
        for mark in ["com_stop()", "d.moe_off()", "d.zero_compares()", "d.enable_low()"] {
            assert!(trip.contains(mark));
        }
        let timer = include_str!("hw/timers.rs");
        let prep = timer.split("pub fn prepare_crossing()").nth(1).unwrap()
            .split("pub fn start_crossing(").next().unwrap();
        let marks = ["t.dier().reset()", "t.cr1().write(|w| w.opm().set_bit().urs().set_bit())",
            "t.cnt().write", "t.egr().write", "t.sr().reset()", "nvic::unpend"];
        let mut pos = 0;
        for mark in marks {
            pos += prep[pos..].find(mark).expect(mark) + mark.len();
        }
        assert!(!prep.contains("cen().set_bit"));
        assert!(!prep.contains("arpe().set_bit"));
    }

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
            atomic,
            FIRMWARE_ARM_IS_ATOMIC,
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

    /// **The late-arm predicate, and why the margin minimum was dropped.**
    ///
    /// E208 recorded `min(wait - spent)` per run. Both reviews of it showed the
    /// statistic is redundant: `left` is floored at 0, so a margin minimum of
    /// zero is *equivalent* to `late_arms > 0` by construction (E209 SS5), and
    /// a minimum over ~700 000 acceptances cannot separate a habitually thin
    /// margin from one excursion (E210 SS1). What replaced it is `thin_count`
    /// -- a count of acceptances with `left <= 2` -- and `ci_min_us`, the
    /// causal variable. This test keeps the predicate itself pinned.
    #[test]
    fn a_late_arm_is_exactly_an_exhausted_wait() {
        // `wait.saturating_sub(spent) + 1`, the production expression.
        let margin_p1 = |wait: u32, spent: u32| wait.saturating_sub(spent).saturating_add(1);
        // A late arm is `left == 0`, i.e. spent >= wait, i.e. margin_p1 == 1.
        assert_eq!(margin_p1(12, 11), 2, "one microsecond of margin");
        assert_eq!(margin_p1(11, 11), 1, "no margin: this is the late arm");
        assert_eq!(margin_p1(9, 11), 1, "over by two: still exactly the late arm");
        // `thin_count` is what replaced the minimum: a *count* of acceptances
        // at or below two microseconds of margin, which is what separates
        // "habitually thin" from "one excursion".
        let thin = |wait: u32, spent: u32| wait.saturating_sub(spent) <= 2;
        assert!(thin(12, 11), "the 50% rung's nominal margin is thin");
        assert!(thin(13, 11));
        assert!(!thin(14, 11), "three microseconds is not thin");
        assert!(thin(9, 11), "and an exhausted wait is thin as well as late");
    }

    #[test]
    fn arm_allowed_is_the_only_rule() {
        assert!(arm_allowed(false, true));
        assert!(!arm_allowed(true, true), "a latched stop refuses");
        assert!(!arm_allowed(false, false), "an inactive loop refuses");
        assert!(!arm_allowed(true, false));
    }
}
