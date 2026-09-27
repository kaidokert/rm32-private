//! Frozen E456 schedule, used only as a differential-test oracle.
use super::{Authority, Decision, Schedule};
use crate::revisit_budget::{Budget, REQUESTS};

pub struct Legacy {
    generation: u32,
    step: u32,
    origin_us: u32,
    average_us: u32,
    slot: u8,
    budget: Budget,
}

impl Legacy {
    pub const fn elapsed(&self, now: u32) -> u32 { now.wrapping_sub(self.origin_us) }
    pub const fn average(&self) -> u32 { self.average_us }
    pub const fn new(generation: u32, step: u32, origin_us: u32, average_us: u32) -> Self {
        Self { generation, step, origin_us, average_us, slot: 0,
            budget: Budget::new(generation, average_us) }
    }

    /// Only idle/listening phases, never accepted COM or blanking.
    pub const fn owns(&self, a: &Authority) -> bool {
        a.powered && (a.phase == 0 || a.phase == 4)
            && a.generation == self.generation && a.step == self.step
            && self.step >= 1 && self.step <= 6
    }

    fn due(&self) -> Option<u32> {
        if self.slot >= REQUESTS || self.budget.next_due(self.generation).is_none() { return None; }
        let half = self.average_us >> 1;
        Some(if self.slot == 0 { half + 1 } else {
            self.average_us + half * u32::from(self.slot) + 1
        })
    }

    /// `now` is an extended clock. False live admission still spends this
    /// observation slot, but not a request. Missed slots never become a burst.
    pub fn observe(&mut self, now: u32, a: &Authority, live_admitted: bool) -> Decision {
        let elapsed = now.wrapping_sub(self.origin_us);
        if !self.owns(a) || elapsed >= 0x8000 {
            self.budget.cancel();
            return Decision { pend: false, delay_us: None };
        }
        let Some(due) = self.due() else { return Decision { pend: false, delay_us: None }; };
        if elapsed < due { return Decision { pend: false, delay_us: Some(due - elapsed) }; }
        let pend = self.budget.reserve(self.generation, elapsed, live_admitted);
        // Select the first future rescue slot directly: no callback catchup
        // and no runtime loop on the M0 path. Successful `due` bounds products.
        let half = self.average_us >> 1;
        self.slot = if elapsed <= self.average_us + half { 1 }
            else if elapsed <= self.average_us + 2 * half { 2 }
            else if elapsed <= self.average_us + 3 * half { 3 }
            else if elapsed <= self.average_us + 4 * half { 4 } else { REQUESTS };
        Decision { pend, delay_us: self.due().map(|d| d - elapsed) }
    }
}


fn authority() -> Authority { Authority { powered: true, phase: 4, generation: 9, step: 3 } }

#[test]
fn all_averages_admission_patterns_and_skipped_slots_match() {
    for avg in 32..=10922 {
        for pattern in 0u32..32 {
            for skip in [0, 1, 4] {
                let origin = u32::MAX - 500;
                let mut old = Legacy::new(9, 3, origin, avg);
                let mut new = Schedule::new(9, 3, origin, avg);
                assert_eq!(old.average(), new.average());
                let half = avg >> 1;
                for slot in 0..5 {
                    let due = if slot == 0 { half + 1 } else { avg + half * slot + 1 };
                    let age = due + skip * half;
                    for t in [age.saturating_sub(1), age, age, age + 1] {
                        let now = origin.wrapping_add(t);
                        assert_eq!(old.elapsed(now), new.elapsed(now));
                        let live = (pattern & (1 << slot)) != 0;
                        assert_eq!(old.observe(now, &authority(), live), new.observe(now, &authority(), live),
                            "avg={avg} pattern={pattern} skip={skip} age={t}");
                    }
                }
            }
        }
    }
}

#[test]
fn invalidation_boundaries_and_no_revival_match() {
    for avg in [0, 1, 31, 32, 33, 80, 81, 10922, 10923, u32::MAX] {
        for fault in 0..5 {
            for age in [0, 40, 41, 32766, 32767, 32768, 65536, u32::MAX] {
                let mut old = Legacy::new(9, 3, 0, avg);
                let mut new = Schedule::new(9, 3, 0, avg);
                let mut a = authority();
                match fault { 0 => a.powered=false, 1 => a.generation+=1,
                    2 => a.step+=1, 3 => a.phase=1, _ => {} }
                assert_eq!(old.observe(age, &a, true), new.observe(age, &a, true));
                assert_eq!(old.observe(41, &authority(), true), new.observe(41, &authority(), true));
            }
        }
    }
}
