//! Independent accepted-event envelope, not rotor-lock certification.
//! Feed every event at its acquisition timestamp, not batched poll time.
//! Caller chooses one wrapping-u32 time unit and externally qualified limits.
//! Call poll even without events. All faults latch; no gate/peripheral access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Stale,
    TooFast,
    SectorOrder,
}

pub struct Monitor {
    last: u32,
    sector: Option<u8>,
    min_interval: u32,
    max_interval: u32,
    fault: Option<Fault>,
    fast_count: u32,
    fast_min: u32,
}

impl Monitor {
    pub fn new(now: u32, min_interval: u32, max_interval: u32) -> Self {
        assert!(min_interval > 0 && min_interval <= max_interval);
        assert!(max_interval < 0x8000_0000);
        Self {
            last: now,
            sector: None,
            min_interval,
            max_interval,
            fault: None,
            fast_count: 0,
            fast_min: u32::MAX,
        }
    }

    pub fn poll(&mut self, now: u32) -> Option<Fault> {
        if self.fault.is_none() && now.wrapping_sub(self.last) > self.max_interval {
            self.fault = Some(Fault::Stale);
        }
        self.fault
    }

    /// Sector is logical 1..6; the first accepted event establishes phase,
    /// NOT an interval measurement or a lock declaration.
    pub fn event(&mut self, now: u32, sector: u8) -> Option<Fault> {
        self.event_policy::<false>(now, sector)
    }

    pub fn fast_events(&self) -> (u32, u32) {
        (
            self.fast_count,
            if self.fast_count == 0 {
                0
            } else {
                self.fast_min
            },
        )
    }
    /// Tighten the missing-event deadline without allowing a slowing or
    /// corrupted estimator to buy more energized dwell time.
    pub fn tighten_max_interval(&mut self, max_interval: u32) -> bool {
        if max_interval < self.min_interval || max_interval >= 0x8000_0000 {
            return false;
        }
        self.max_interval = self.max_interval.min(max_interval);
        true
    }
    pub fn max_interval(&self) -> u32 {
        self.max_interval
    }
    /// Read retained operational state after shutdown; no new event work.
    pub fn stopped_state(&self) -> [u32; 3] {
        [
            match self.fault {
                None => 0,
                Some(Fault::Stale) => 1,
                Some(Fault::TooFast) => 2,
                Some(Fault::SectorOrder) => 3,
            },
            self.last,
            self.sector.unwrap_or(0) as u32,
        ]
    }

    /// Exploration may report short intervals; stale/order faults still latch.
    /// Update progress only after validating freshness and the next sector.
    pub fn event_policy<const REPORT_FAST: bool>(&mut self, now: u32, sector: u8) -> Option<Fault> {
        if self.poll(now).is_some() {
            return self.fault;
        }
        if !(1..=6).contains(&sector)
            || self
                .sector
                .is_some_and(|previous| sector != if previous == 6 { 1 } else { previous + 1 })
        {
            self.fault = Some(Fault::SectorOrder);
        } else if !REPORT_FAST
            && self.sector.is_some()
            && now.wrapping_sub(self.last) < self.min_interval
        {
            self.fault = Some(Fault::TooFast);
        } else {
            let gap = now.wrapping_sub(self.last);
            #[cfg(not(feature = "bench-lean-irq"))]
            if REPORT_FAST && self.sector.is_some() && gap < self.min_interval {
                self.fast_count = self.fast_count.saturating_add(1);
                self.fast_min = self.fast_min.min(gap);
            }
            self.last = now;
            self.sector = Some(sector);
        }
        self.fault
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_fast_keeps_order_and_missing_progress_stops() {
        let mut m = Monitor::new(u32::MAX - 50, 100, 1000);
        assert_eq!(m.event_policy::<true>(u32::MAX - 10, 6), None);
        assert_eq!(m.event_policy::<true>(20, 1), None);
        assert_eq!(m.fast_events(), (1, 31));
        assert_eq!(m.event_policy::<true>(20, 2), None);
        assert_eq!(m.fast_events(), (2, 0));
        assert_eq!(m.poll(1020), None);
        assert_eq!(m.poll(1021), Some(Fault::Stale));
        assert_eq!(m.event_policy::<true>(1022, 3), Some(Fault::Stale));
        let mut m = Monitor::new(0, 100, 1000);
        assert_eq!(m.event_policy::<true>(10, 1), None);
        assert_eq!(m.event_policy::<true>(20, 1), Some(Fault::SectorOrder));
        assert_eq!(m.fast_events(), (0, 0));
    }
    #[test]
    fn ordered_train_wraps_clock_and_sector() {
        let mut m = Monitor::new(u32::MAX - 100, 100, 200);
        assert_eq!(m.event(u32::MAX - 50, 6), None);
        assert_eq!(m.event(99, 1), None);
        assert_eq!(m.event(249, 2), None);
    }
    #[test]
    fn absent_first_and_late_event_cannot_hide_staleness() {
        let mut m = Monitor::new(0, 100, 200);
        assert_eq!(m.poll(201), Some(Fault::Stale));
        assert_eq!(m.event(202, 1), Some(Fault::Stale));
    }
    #[test]
    fn boundaries_inclusive_and_fast_fault_latches() {
        let mut m = Monitor::new(0, 100, 200);
        assert_eq!(m.event(100, 1), None);
        assert_eq!(m.event(200, 2), None);
        assert_eq!(m.event(400, 3), None);
        assert_eq!(m.event(499, 4), Some(Fault::TooFast));
        assert_eq!(m.event(600, 4), Some(Fault::TooFast));
    }
    #[test]
    fn duplicate_skip_and_invalid_are_not_progress() {
        for sector in [0, 1, 3, 7] {
            let mut m = Monitor::new(0, 100, 200);
            m.event(50, 1);
            assert_eq!(m.event(150, sector), Some(Fault::SectorOrder));
        }
    }
    #[test]
    fn stale_limit_only_tightens_and_remains_wrap_safe() {
        let mut m = Monitor::new(u32::MAX - 100, 100, 1000);
        assert!(m.tighten_max_interval(400));
        assert!(m.tighten_max_interval(700));
        assert_eq!(m.max_interval(), 400);
        assert!(!m.tighten_max_interval(99));
        assert_eq!(m.event(u32::MAX - 50, 1), None);
        assert_eq!(m.poll(349), None);
        assert_eq!(m.poll(350), Some(Fault::Stale));
    }
}
