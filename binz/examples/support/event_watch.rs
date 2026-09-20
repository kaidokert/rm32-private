//! Independent accepted-event freshness watchdog, NOT proof of rotor lock.
//! Configure the deadline from the external test plan, not a possibly stale
//! controller estimate. Call even when no events arrive. Faults latch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Stale,
    CounterBackwards,
}
pub struct Watch {
    last_time: u32,
    last_count: u32,
    max_age: u32,
    fault: Option<Fault>,
}
impl Watch {
    pub fn new(now: u32, count: u32, max_age_us: u32) -> Self {
        assert!(max_age_us > 0 && max_age_us < 0x8000_0000);
        Self {
            last_time: now,
            last_count: count,
            max_age: max_age_us,
            fault: None,
        }
    }
    pub fn poll(&mut self, now: u32, count: u32) -> Option<Fault> {
        if self.fault.is_some() {
            return self.fault;
        }
        // Test age BEFORE accepting new progress: an event arriving after a
        // blind interval cannot retrospectively erase a missed deadline.
        if now.wrapping_sub(self.last_time) > self.max_age {
            self.fault = Some(Fault::Stale);
        } else {
            let delta = count.wrapping_sub(self.last_count);
            if delta >= 0x8000_0000 {
                self.fault = Some(Fault::CounterBackwards);
            } else if delta != 0 {
                self.last_time = now;
                self.last_count = count;
            }
        }
        self.fault
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_even_without_desync_or_first_event() {
        let mut w = Watch::new(0, 0, 2500);
        assert_eq!(w.poll(2500, 0), None);
        assert_eq!(w.poll(2501, 0), Some(Fault::Stale));
        assert_eq!(w.poll(2502, 1), Some(Fault::Stale));
    }
    #[test]
    fn duplicates_cannot_extend_deadline() {
        let mut w = Watch::new(0, 0, 2500);
        assert_eq!(w.poll(1000, 1), None);
        assert_eq!(w.poll(3000, 1), None);
        assert_eq!(w.poll(3501, 1), Some(Fault::Stale));
    }
    #[test]
    fn late_event_cannot_hide_gap() {
        let mut w = Watch::new(0, 0, 2500);
        assert_eq!(w.poll(2501, 1), Some(Fault::Stale));
    }
    #[test]
    fn clocks_and_sequence_wrap() {
        let mut w = Watch::new(u32::MAX - 100, u32::MAX, 2500);
        assert_eq!(w.poll(200, 0), None);
        assert_eq!(w.poll(2700, 0), None);
        assert_eq!(w.poll(2701, 0), Some(Fault::Stale));
    }
    #[test]
    fn unexpected_counter_reset_latches() {
        let mut w = Watch::new(0, 4, 2500);
        assert_eq!(w.poll(10, 0), Some(Fault::CounterBackwards));
    }
}
