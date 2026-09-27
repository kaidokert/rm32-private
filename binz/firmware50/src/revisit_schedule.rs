//! Five bounded observation slots, not continuous foreground polling.
//! This model never schedules a commutation. Hardware owners must apply its
//! decisions atomically with acceptance and stop and clear obsolete IRQs.
use crate::revisit_budget::{MAX_AVERAGE_US, REQUESTS};

#[cfg(test)]
#[path = "revisit_schedule_equivalence.rs"]
mod equivalence;

pub struct Authority {
    pub powered: bool,
    pub phase: u32,
    pub generation: u32,
    pub step: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub pend: bool,
    pub delay_us: Option<u32>,
}

pub struct Schedule {
    generation: u32,
    step: u32,
    origin_us: u32,
    average_us: u32,
    slot: u8,
}

impl Schedule {
    pub const fn elapsed(&self, now: u32) -> u32 { now.wrapping_sub(self.origin_us) }
    pub const fn average(&self) -> u32 { self.average_us }
    pub const fn new(generation: u32, step: u32, origin_us: u32, average_us: u32) -> Self {
        let valid = average_us >= crate::revisit::AVERAGE_MIN_US && average_us <= MAX_AVERAGE_US;
        Self { generation, step, origin_us, average_us, slot: if valid { 0 } else { REQUESTS } }
    }

    /// Only idle/listening phases, never accepted COM or blanking.
    pub const fn owns(&self, a: &Authority) -> bool {
        a.powered && (a.phase == 0 || a.phase == 4)
            && a.generation == self.generation && a.step == self.step
            && self.step >= 1 && self.step <= 6
    }

    fn due(&self) -> Option<u32> {
        if self.slot >= REQUESTS { return None; }
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
            self.slot = REQUESTS;
            return Decision { pend: false, delay_us: None };
        }
        let Some(due) = self.due() else { return Decision { pend: false, delay_us: None }; };
        if elapsed < due { return Decision { pend: false, delay_us: Some(due - elapsed) }; }
        // Each due observation consumes at least one slot; requests <= slots.
        // The old Budget deadline cannot exceed this slot's already-met due.
        let pend = live_admitted;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_owner_disables_only_foreground_revisit() {
        use crate::run::policy::{Bemf, BemfPolicy, ExternalRevisit};
        use crate::bemf::FilterPolicy;
        for seed in [40, 80, 250, 833, 1000] {
            assert_eq!(ExternalRevisit::<BemfPolicy>::estimator(seed).state(), BemfPolicy::estimator(seed).state());
            assert_eq!(ExternalRevisit::<BemfPolicy>::FILTER.level(seed), BemfPolicy::FILTER.level(seed));
        }
        for duty in 0..=800 { assert!(!ExternalRevisit::<BemfPolicy>::allow_revisit(duty)); }
    }
    fn owner() -> Authority { Authority { powered: true, phase: 4, generation: 9, step: 3 } }

    #[test]
    fn listening_phase_can_request_but_blanking_commutation_and_stop_cannot() {
        for phase in 0..6 {
            let mut s = Schedule::new(9, 3, 1000, 80);
            let mut a = owner(); a.phase = phase;
            assert_eq!(s.observe(1041, &a, true).pend, phase == 0 || phase == 4);
        }
        for kind in 0..3 {
            let mut s = Schedule::new(9, 3, 1000, 80);
            let mut a = owner();
            match kind { 0 => a.powered = false, 1 => a.generation += 1, _ => a.step += 1 }
            assert_eq!(s.observe(1041, &a, true), Decision { pend: false, delay_us: None });
            assert!(!s.observe(1041, &owner(), true).pend, "canceled work must not revive");
        }
    }

    #[test]
    fn slots_bound_callbacks_and_skip_catchup_without_fake_commutation() {
        let mut s = Schedule::new(9, 3, 1000, 80);
        assert_eq!(s.observe(1040, &owner(), true), Decision { pend: false, delay_us: Some(1) });
        for (i, age) in [41, 121, 161, 201, 241].into_iter().enumerate() {
            let d = s.observe(1000 + age, &owner(), true);
            assert!(d.pend);
            assert_eq!(d.delay_us, [Some(80), Some(40), Some(40), Some(40), None][i]);
        }
        assert!(!s.observe(1400, &owner(), true).pend);
        let mut late = Schedule::new(9, 3, 1000, 80);
        assert_eq!(late.observe(1300, &owner(), true), Decision { pend: true, delay_us: None });
        assert!(!late.observe(1300, &owner(), true).pend);
    }

    #[test]
    fn failed_live_checks_do_not_spin_or_require_fabricated_edges() {
        let mut s = Schedule::new(9, 3, 1000, 81);
        assert_eq!(s.observe(1041, &owner(), false), Decision { pend: false, delay_us: Some(81) });
        assert!(s.observe(1122, &owner(), true).pend);
        let mut wrapped = Schedule::new(9, 3, u32::MAX - 20, 80);
        assert!(wrapped.observe(20, &owner(), true).pend);
        let mut old = Schedule::new(9, 3, 0, 80);
        assert_eq!(old.observe(65536, &owner(), true), Decision { pend: false, delay_us: None });
    }
}
