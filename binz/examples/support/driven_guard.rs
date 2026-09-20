//! Proposed driven-acquisition safety policy. No hardware caller/output authority.
//! Open-loop commands are NOT accepted BEMF events. A separate independent
//! timer must call tick; calling authorize must not refresh its timestamp.
use crate::powered_guard::{Fault, Feedback, validate_feedback};

pub struct Guard {
    start: u32,
    window: u32,
    last_tick: u32,
    last_feedback: u32,
    step: u8,
    fault: Option<Fault>,
}
impl Guard {
    /// Maximum20ms diagnostic acquisition, not a production blind-start limit.
    /// Caller retains the original campaign deadline outside this local window.
    pub fn new(now: u32, window: u32, step: u8, sample: Feedback) -> Result<Self, Fault> {
        let max = if cfg!(feature = "bench-startup-adc") {
            40_000
        } else {
            20_000
        };
        if !(1..=max).contains(&window) {
            return Err(Fault::SegmentDeadline);
        }
        if !(1..=6).contains(&step) {
            return Err(Fault::InvalidSeed);
        }
        validate_feedback(sample)?;
        Ok(Self {
            start: now,
            window,
            last_tick: now,
            last_feedback: now,
            step,
            fault: None,
        })
    }
    fn fail(&mut self, f: Fault) -> Result<(), Fault> {
        self.fault = Some(f);
        Err(f)
    }
    pub fn age_initial_feedback(&mut self, age: u32) -> Result<(), Fault> {
        if let Some(f) = self.fault {
            return Err(f);
        }
        if age > 1000 {
            return self.fail(Fault::FeedbackStale);
        }
        self.last_feedback = self.start.wrapping_sub(age);
        Ok(())
    }
    fn check(&mut self, now: u32) -> Result<(), Fault> {
        if let Some(f) = self.fault {
            return Err(f);
        }
        if now.wrapping_sub(self.start) >= self.window {
            return self.fail(Fault::SegmentDeadline);
        }
        if now.wrapping_sub(self.last_tick) > 200 {
            return self.fail(Fault::TickGap);
        }
        if now.wrapping_sub(self.last_feedback) > 1000 {
            return self.fail(Fault::FeedbackStale);
        }
        Ok(())
    }
    pub fn tick(&mut self, now: u32, no_fault: bool, abort: bool) -> Result<(), Fault> {
        if let Some(f) = self.fault {
            return Err(f);
        }
        if abort {
            return self.fail(Fault::HostAbort);
        }
        if !no_fault {
            return self.fail(Fault::Driver);
        }
        self.check(now)?;
        self.last_tick = now;
        Ok(())
    }
    pub fn feedback(&mut self, now: u32, age: u32, sample: Feedback) -> Result<(), Fault> {
        self.check(now)?;
        if age > 1000 {
            return self.fail(Fault::FeedbackStale);
        }
        let acquired = now.wrapping_sub(age);
        if acquired.wrapping_sub(self.last_feedback) > 1000 {
            return self.fail(Fault::FeedbackStale);
        }
        if let Err(f) = validate_feedback(sample) {
            return self.fail(f);
        }
        self.last_feedback = acquired;
        Ok(())
    }
    /// Every physical write must be serialized with stop/tick by the caller.
    /// This validates only command order,never rotor phase or a handoff seed.
    pub fn authorize(&mut self, now: u32, step: u8) -> Result<(), Fault> {
        self.check(now)?;
        let next = if self.step == 6 { 1 } else { self.step + 1 };
        if step != self.step && step != next {
            return self.fail(Fault::InvalidSeed);
        }
        self.step = step;
        Ok(())
    }
    pub fn stop(&mut self) {
        if self.fault.is_none() {
            self.fault = Some(Fault::HostAbort);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Feedback {
        Feedback {
            phase: [2048; 3],
            bus_mv: 11700,
            vref: 1500,
        }
    }
    #[test]
    fn baseline_age_survives_start_and_wrap() {
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        g.age_initial_feedback(900).unwrap();
        assert_eq!(g.tick(100, true, false), Ok(()));
        assert_eq!(g.tick(101, true, false), Err(Fault::FeedbackStale));
        assert_eq!(g.age_initial_feedback(0), Err(Fault::FeedbackStale));
        let mut g = Guard::new(u32::MAX - 10, 20_000, 1, sample()).unwrap();
        assert_eq!(g.age_initial_feedback(1001), Err(Fault::FeedbackStale));
    }
    #[test]
    fn bounded_commands_are_not_bemf_and_cannot_feed_independent_tick() {
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        assert_eq!(g.authorize(100, 2), Ok(()));
        assert_eq!(g.authorize(200, 3), Ok(()));
        assert_eq!(g.authorize(201, 4), Err(Fault::TickGap));
        assert_eq!(g.tick(202, true, false), Err(Fault::TickGap));
    }
    #[test]
    fn absolute_window_wrap_and_post_stop_refusal() {
        let start = u32::MAX - 1000;
        let mut g = Guard::new(start, 20_000, 1, sample()).unwrap();
        for elapsed in (100..20_000).step_by(100) {
            let now = start.wrapping_add(elapsed);
            assert_eq!(g.tick(now, true, false), Ok(()));
            assert_eq!(g.feedback(now, 0, sample()), Ok(()));
        }
        assert_eq!(
            g.authorize(start.wrapping_add(20_000), 2),
            Err(Fault::SegmentDeadline)
        );
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        g.stop();
        assert_eq!(g.authorize(1, 2), Err(Fault::HostAbort));
    }
    #[test]
    fn stale_reordered_electrical_and_wrong_order_latch() {
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        for t in (100..=1000).step_by(100) {
            g.tick(t, true, false).unwrap();
        }
        assert_eq!(g.feedback(1001, 0, sample()), Err(Fault::FeedbackStale));
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        g.feedback(100, 0, sample()).unwrap();
        assert_eq!(g.feedback(150, 100, sample()), Err(Fault::FeedbackStale));
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        let mut bad = sample();
        bad.phase[2] = 3249;
        assert_eq!(g.feedback(1, 0, bad), Err(Fault::Current));
        assert_eq!(g.authorize(2, 2), Err(Fault::Current));
        let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
        assert_eq!(g.authorize(1, 3), Err(Fault::InvalidSeed));
        assert!(Guard::new(0, 20_001, 1, sample()).is_err());
        assert!(Guard::new(0, 100, 0, sample()).is_err());
    }
    #[test]
    fn bus_driver_and_host_faults_cannot_be_cleared_by_new_commands() {
        let mut bad = sample();
        bad.bus_mv = 8399;
        assert!(matches!(Guard::new(0, 100, 1, bad), Err(Fault::Bus)));
        for (no_fault, abort, expected) in [
            (false, false, Fault::Driver),
            (true, true, Fault::HostAbort),
        ] {
            let mut g = Guard::new(0, 20_000, 1, sample()).unwrap();
            assert_eq!(g.tick(1, no_fault, abort), Err(expected));
            assert_eq!(g.feedback(2, 0, sample()), Err(expected));
            assert_eq!(g.authorize(3, 2), Err(expected));
        }
    }
}
