//! Staged recovery-timer policy, NOT connected to motor or timer hardware.
//!
//! Start a gate-inhibited timer before sensing the final edge. Once the existing
//! acquisition has qualified that edge, publish its EXACT edge+reference-wait
//! deadline into the running counter. No predicted edge or free-run fallback.
//! The adapter must measure/validate its publication budget before powered use.
#![no_std]

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Range,
    Expired,
    Late,
    State,
}
/// TIM17 is quantized to1us; TIM16 counts half-us ticks. Read TIM17, TIM16,
/// TIM17 with IRQs masked. Mapping chooses the late endpoint, never an earlier
/// commutation, with the returned explicit quantization/bracket uncertainty.
#[derive(Debug, PartialEq, Eq)]
pub struct Mapped {
    pub arr: u16,
    pub max_late_ticks: u32,
}
pub fn map_deadline(
    before: u32,
    after: u32,
    counter: u16,
    deadline: u32,
    write_budget: u32,
) -> Result<Mapped, Fault> {
    let bracket = after.wrapping_sub(before);
    if before & 1 != 0 || after & 1 != 0 || bracket > 2 || write_budget == 0 {
        return Err(Fault::Range);
    }
    let remaining = deadline.wrapping_sub(after);
    if remaining > 65_535 || remaining < write_budget {
        return Err(Fault::Late);
    }
    let target = counter as u32 + remaining + bracket;
    if target > 65_535 {
        return Err(Fault::Expired);
    }
    Ok(Mapped {
        arr: (target - 1) as u16,
        max_late_ticks: bracket + 2,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Waiting,
    Scheduled(u32),
    Finished,
    Failed(Fault),
}
pub struct Prepared {
    origin: u32,
    span: u32,
    state: State,
}
impl Prepared {
    /// All values are half-us ticks. A transaction must finish before either
    /// the original campaign deadline or the first 16-bit counter wrap.
    pub fn new(origin: u32, original_end: u32) -> Result<Self, Fault> {
        let span = original_end.wrapping_sub(origin);
        if span == 0 || span > 65_535 {
            return Err(Fault::Range);
        }
        Ok(Self {
            origin,
            span,
            state: State::Waiting,
        })
    }
    fn fail<T>(&mut self, fault: Fault) -> Result<T, Fault> {
        self.state = State::Failed(fault);
        Err(fault)
    }
    fn elapsed(&mut self, now: u32) -> Result<u32, Fault> {
        if let State::Failed(f) = self.state {
            return Err(f);
        }
        let elapsed = now.wrapping_sub(self.origin);
        if elapsed >= self.span {
            return self.fail(Fault::Expired);
        }
        Ok(elapsed)
    }
    /// Caller supplies a genuinely qualified onset, never confirmation time.
    /// `write_budget` is a separately measured adapter bound, NOT an assumed
    /// replacement for the live firmware's current 64-tick arm allowance.
    /// Returns ARR for an already-running counter (overflow at ARR+1).
    pub fn publish(
        &mut self,
        onset: u32,
        wait: u32,
        now: u32,
        write_budget: u32,
    ) -> Result<u16, Fault> {
        let elapsed = self.elapsed(now)?;
        if self.state != State::Waiting {
            return self.fail(Fault::State);
        }
        if wait == 0 || wait > 65_535 || write_budget == 0 {
            return self.fail(Fault::Range);
        }
        let age = now.wrapping_sub(onset);
        let Some(remaining) = wait.checked_sub(age) else {
            return self.fail(Fault::Late);
        };
        if remaining < write_budget {
            return self.fail(Fault::Late);
        }
        let target = elapsed + remaining; // bounded by two u16-sized operands
        if target >= self.span || target > 65_535 {
            return self.fail(Fault::Expired);
        }
        self.state = State::Scheduled(target);
        Ok((target - 1) as u16)
    }
    /// Commit authority is one-shot and conditional on a qualified publication.
    /// A missing edge, cancellation, expired campaign or late ISR never grants it.
    /// Electrical/ownership checks remain the hardware adapter's responsibility.
    pub fn service(&mut self, now: u32, lateness_budget: u32) -> Result<bool, Fault> {
        let elapsed = self.elapsed(now)?;
        match self.state {
            State::Waiting => Ok(false),
            State::Scheduled(target) if elapsed < target => Ok(false),
            State::Scheduled(target) => {
                if elapsed - target > lateness_budget {
                    return self.fail(Fault::Late);
                }
                self.state = State::Finished;
                Ok(true)
            }
            _ => self.fail(Fault::State),
        }
    }
    pub fn cancel(&mut self) {
        self.state = State::Failed(Fault::State);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_mapping_bounds_quantization_wrap_and_deadline() {
        // Physical time in quarter-us units exercises both timer phases.
        for epoch in [0u32, u32::MAX - 199] {
            for sample in 0..400u32 {
                for read_delay in 0..4u32 {
                    let before = epoch.wrapping_add((sample / 4) * 2);
                    let after = epoch.wrapping_add(((sample + read_delay) / 4) * 2);
                    let counter = (sample / 2 + 1000) as u16;
                    let deadline = epoch.wrapping_add(400);
                    let mapped = map_deadline(before, after, counter, deadline, 16).unwrap();
                    // The scheduled count minus sampled count maps to elapsed time;
                    // TIM16's own half-tick phase cancels for a fixed counter clock.
                    let actual = sample / 2 + (mapped.arr as u32 + 1 - counter as u32);
                    assert!(actual >= 400);
                    assert!(actual - 400 <= mapped.max_late_ticks);
                }
            }
        }
        assert_eq!(map_deadline(100, 104, 0, 200, 16), Err(Fault::Range));
        assert_eq!(map_deadline(100, 102, 0, 117, 16), Err(Fault::Late));
        assert_eq!(map_deadline(100, 102, 0, 99, 16), Err(Fault::Late));
        assert_eq!(map_deadline(100, 102, 65530, 200, 16), Err(Fault::Expired));
    }
    #[test]
    fn exact_deadline_across_clock_wrap_and_varied_prepare_times() {
        for origin in [0u32, u32::MAX - 200] {
            for edge_offset in 20..400u32 {
                for wait in [83, 90, 219] {
                    let mut p = Prepared::new(origin, origin.wrapping_add(2000)).unwrap();
                    let edge = origin.wrapping_add(edge_offset);
                    let now = edge.wrapping_add(52); // hypothetical20us dwell+6us work
                    let arr = p.publish(edge, wait, now, 16).unwrap();
                    assert_eq!(arr as u32 + 1, edge_offset + wait);
                    assert_eq!(p.service(origin.wrapping_add(arr as u32), 0), Ok(false));
                    assert_eq!(p.service(origin.wrapping_add(arr as u32 + 1), 0), Ok(true));
                    assert_eq!(
                        p.service(origin.wrapping_add(arr as u32 + 1), 0),
                        Err(Fault::State)
                    );
                }
            }
        }
    }
    #[test]
    fn budget_does_not_hide_dma_or_refresh_onset() {
        let mut p = Prepared::new(0, 2000).unwrap();
        // Observing later cannot move edge100's deadline183 later.
        assert_eq!(p.publish(100, 83, 152 + 52, 16), Err(Fault::Late));
        assert_eq!(p.publish(100, 83, 152, 16), Err(Fault::Late));
        assert_eq!(p.service(183, 100), Err(Fault::Late));
    }
    #[test]
    fn missing_cancelled_and_stale_inputs_never_commit() {
        let mut p = Prepared::new(0, 2000).unwrap();
        assert_eq!(p.service(1999, 0), Ok(false));
        assert_eq!(p.service(2000, 0), Err(Fault::Expired));
        let mut p = Prepared::new(0, 2000).unwrap();
        p.publish(100, 83, 152, 16).unwrap();
        p.cancel();
        assert_eq!(p.service(183, 0), Err(Fault::State));
        let mut p = Prepared::new(0, 2000).unwrap();
        p.publish(100, 83, 152, 16).unwrap();
        assert_eq!(p.service(188, 4), Err(Fault::Late));
    }
    #[test]
    fn exact_publication_boundary_and_original_deadline() {
        let mut p = Prepared::new(0, 2000).unwrap();
        assert_eq!(p.publish(100, 83, 167, 16), Ok(182));
        let mut p = Prepared::new(0, 2000).unwrap();
        assert_eq!(p.publish(100, 83, 168, 16), Err(Fault::Late));
        let mut p = Prepared::new(0, 183).unwrap();
        assert_eq!(p.publish(100, 83, 152, 16), Err(Fault::Expired));
        assert!(matches!(Prepared::new(0, 65_536), Err(Fault::Range)));
    }
}
