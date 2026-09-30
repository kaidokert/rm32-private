//! Accepted-event tracking: the closed loop's missing-commutation watchdog.
//!
//! Transcribed from `binz/examples/support/accepted_timing.rs::Monitor` and
//! `powered_guard.rs::speed_event_limit_us`, the qualified image's
//! accepted-event envelope ("independent accepted-event envelope, not
//! rotor-lock certification"). It is what turns a missed crossing into a named
//! `Tracking` stop, so the closed loop can run without any scripted
//! commutation (goal gate 1) and still never drive a lost rotor for long.
//!
//! Three verdicts, all latching:
//!
//! * **Stale**: no accepted event for longer than `max_interval`. It starts at
//!   the reference's 1000 µs and is *tightened* -- never loosened -- by
//!   [`speed_event_limit_us`] to three controller periods, so a slowing or
//!   corrupted estimator cannot buy itself more energized dwell time.
//! * **SectorOrder**: each accepted event must be the previous sector + 1.
//! * **TooFast**: an interval below `min_interval`. With `REPORT_FAST` it is
//!   counted, not latched -- the qualified image's `bench-fast-cycle-report`,
//!   under which it ran 2.1 keHz at 50% with a 238 µs event minimum on the
//!   books (`DUTY_50_CAMPAIGN.md:1117-1119`).

/// Why tracking stopped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    Stale,
    TooFast,
    SectorOrder,
}

/// The reference's missing-event ceiling, µs.
pub const EVENT_MAX_US: u32 = 1_000;

/// Three controller periods in µs, from a period in half-µs, floored at 200
/// and capped at 1000 (reference `speed_event_limit_us`, with its own
/// compile-time table). Split at 667 so the only multiply stays small.
#[must_use]
pub const fn speed_event_limit_us(reference_half_us: u32) -> u32 {
    if reference_half_us >= 667 {
        1_000
    } else {
        let us = (reference_half_us * 3 + 1) >> 1;
        if us < 200 {
            200
        } else {
            us
        }
    }
}

const _: () = {
    assert!(speed_event_limit_us(0) == 200);
    assert!(speed_event_limit_us(133) == 200);
    assert!(speed_event_limit_us(200) == 300);
    assert!(speed_event_limit_us(666) == 999);
    assert!(speed_event_limit_us(667) == 1_000);
    assert!(speed_event_limit_us(u32::MAX) == 1_000);
};

/// The accepted-event envelope. `REPORT_FAST` selects the reference's
/// report-only treatment of short intervals.
#[derive(Copy, Clone, Debug)]
pub struct EventWatch<const REPORT_FAST: bool> {
    last: u32,
    sector: Option<u8>,
    min_interval: u32,
    max_interval: u32,
    fault: Option<Fault>,
    fast_count: u32,
    fast_min: u32,
}

impl<const REPORT_FAST: bool> EventWatch<REPORT_FAST> {
    /// `now` is the arming instant; the first stale deadline runs from it.
    #[must_use]
    pub const fn new(now: u32, min_interval: u32, max_interval: u32) -> Self {
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

    #[must_use]
    pub const fn fault(&self) -> Option<Fault> {
        self.fault
    }

    #[must_use]
    pub const fn max_interval(&self) -> u32 {
        self.max_interval
    }

    /// Short intervals seen under `REPORT_FAST`: count and minimum.
    #[must_use]
    pub const fn fast_events(&self) -> (u32, u32) {
        (self.fast_count, if self.fast_count == 0 { 0 } else { self.fast_min })
    }

    /// Check for a stale event, with or without a new one.
    pub fn poll(&mut self, now: u32) -> Option<Fault> {
        if self.fault.is_none() && now.wrapping_sub(self.last) > self.max_interval {
            self.fault = Some(Fault::Stale);
        }
        self.fault
    }

    /// Tighten the missing-event deadline; refuses to loosen it or to go
    /// below the minimum interval.
    pub fn tighten_max_interval(&mut self, max_interval: u32) -> bool {
        if max_interval < self.min_interval || max_interval >= 0x8000_0000 {
            return false;
        }
        if max_interval < self.max_interval {
            self.max_interval = max_interval;
        }
        true
    }

    /// **`n` accepted events, the newest at `now` in sector `sector`** (goal B
    /// step 2: the guard tick feeds the watch, and at high speed more than one
    /// crossing lands between ticks). The order check becomes "the newest sector
    /// is the last one seen plus `n`"; the short-interval report compares the
    /// whole gap against `n` minimum intervals and records a minimum only for
    /// `n == 1`. `n == 1` is exactly [`Self::event`]. No division: `n` is reduced
    /// modulo 6 by bounded subtraction, and a batch of 18 or more crossings
    /// between two ticks (impossible while the tick-gap stop holds)
    /// re-establishes phase instead of judging order.
    pub fn events(&mut self, now: u32, sector: u8, n: u32) -> Option<Fault> {
        if n == 0 {
            return self.poll(now);
        }
        if n == 1 {
            return self.event(now, sector);
        }
        if self.poll(now).is_some() {
            return self.fault;
        }
        let mut k = n;
        if k >= 12 {
            k -= 12;
        }
        if k >= 6 {
            k -= 6;
        }
        let in_order = match self.sector {
            None => true,
            Some(_) if k >= 6 => true,
            Some(prev) => {
                let s = u32::from(prev) + k;
                u32::from(sector) == if s > 6 { s - 6 } else { s }
            }
        };
        if !(1..=6).contains(&sector) || !in_order {
            self.fault = Some(Fault::SectorOrder);
        } else {
            let gap = now.wrapping_sub(self.last);
            // A plain 32-bit multiply: `n` is below 18 here (reduced above only
            // for the order check, but a batch that large is past the tick-gap
            // stop) and `min_interval` is a µs constant, so no overflow and no
            // 64-bit helper (`saturating_mul` pulls `__aeabi_lmul`, which the
            // ISR math audit refuses).
            let span = if n >= 64 {
                u32::MAX
            } else {
                self.min_interval.wrapping_mul(n)
            };
            if self.sector.is_some() && gap < span {
                if REPORT_FAST {
                    self.fast_count = self.fast_count.saturating_add(n);
                } else {
                    self.fault = Some(Fault::TooFast);
                    return self.fault;
                }
            }
            self.last = now;
            self.sector = Some(sector);
        }
        self.fault
    }

    /// One accepted event in logical sector `sector` (1..=6). The first event
    /// establishes phase only.
    pub fn event(&mut self, now: u32, sector: u8) -> Option<Fault> {
        if self.poll(now).is_some() {
            return self.fault;
        }
        let in_order = match self.sector {
            None => true,
            Some(prev) => sector == if prev == 6 { 1 } else { prev + 1 },
        };
        if !(1..=6).contains(&sector) || !in_order {
            self.fault = Some(Fault::SectorOrder);
        } else {
            let gap = now.wrapping_sub(self.last);
            if self.sector.is_some() && gap < self.min_interval {
                if REPORT_FAST {
                    self.fast_count = self.fast_count.saturating_add(1);
                    if gap < self.fast_min {
                        self.fast_min = gap;
                    }
                } else {
                    self.fault = Some(Fault::TooFast);
                    return self.fault;
                }
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

    type Watch = EventWatch<false>;
    type Report = EventWatch<true>;

    /// Goal B step 2: a batch of `n` crossings between two guard ticks.
    #[test]
    fn batched_events_judge_order_by_count() {
        let mut w = Report::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.events(100, 3, 1), None, "first event sets phase");
        assert_eq!(w.events(200, 5, 2), None, "3 + 2 = 5");
        assert_eq!(w.events(300, 2, 3), None, "5 + 3 wraps to 2");
        assert_eq!(w.events(350, 2, 0), None, "no new crossing is a poll");
        let mut bad = Report::new(0, 238, EVENT_MAX_US);
        assert_eq!(bad.events(100, 3, 1), None);
        assert_eq!(
            bad.events(200, 4, 2),
            Some(Fault::SectorOrder),
            "two crossings but one sector"
        );
        let mut wrap = Report::new(0, 238, EVENT_MAX_US);
        assert_eq!(wrap.events(100, 1, 1), None);
        assert_eq!(wrap.events(200, 2, 7), None, "seven crossings advance one sector mod 6");
    }

    #[test]
    fn batched_short_gaps_are_reported_not_latched() {
        let mut w = Report::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.events(100, 1, 1), None);
        assert_eq!(w.events(200, 3, 2), None, "100 us for two crossings is short");
        assert_eq!(w.fast_events().0, 2);
        let mut strict = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(strict.events(100, 1, 1), None);
        assert_eq!(strict.events(200, 3, 2), Some(Fault::TooFast));
    }

    #[test]
    fn a_batch_still_goes_stale() {
        let mut w = Report::new(0, 238, 300);
        assert_eq!(w.events(100, 1, 1), None);
        assert_eq!(
            w.events(500, 3, 2),
            Some(Fault::Stale),
            "the batch arrived after the deadline"
        );
    }

    #[test]
    fn a_steady_ordered_stream_is_healthy() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        let mut t = 0;
        for k in 0..60u32 {
            t += 420;
            assert_eq!(w.event(t, (k % 6) as u8 + 1), None, "event {k}");
        }
        assert_eq!(w.poll(t + 1_000), None);
    }

    #[test]
    fn a_missing_event_goes_stale_and_latches() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.event(400, 1), None);
        assert_eq!(w.poll(1_400), None, "exactly at the limit is fine");
        assert_eq!(w.poll(1_401), Some(Fault::Stale));
        // Latched: a later healthy-looking event cannot clear it.
        assert_eq!(w.event(1_500, 2), Some(Fault::Stale));
    }

    #[test]
    fn the_first_deadline_runs_from_arming() {
        let mut w = Watch::new(10_000, 238, EVENT_MAX_US);
        assert_eq!(w.poll(11_000), None);
        assert_eq!(w.poll(11_001), Some(Fault::Stale));
    }

    #[test]
    fn out_of_order_sectors_fault() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.event(400, 3), None);
        assert_eq!(w.event(800, 5), Some(Fault::SectorOrder));
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.event(400, 6), None);
        assert_eq!(w.event(800, 1), None, "6 wraps to 1");
        assert_eq!(Watch::new(0, 238, 1_000).event(100, 7), Some(Fault::SectorOrder));
    }

    #[test]
    fn too_fast_latches_unless_reported() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.event(400, 1), None);
        assert_eq!(w.event(600, 2), Some(Fault::TooFast));

        // The qualified image's report-only policy: counted, not latched --
        // 237 us sectors at 15% are legitimately below a 238 us minimum.
        let mut r = Report::new(0, 238, EVENT_MAX_US);
        assert_eq!(r.event(400, 1), None);
        assert_eq!(r.event(637, 2), None);
        assert_eq!(r.event(874, 3), None);
        assert_eq!(r.fast_events(), (2, 237));
    }

    #[test]
    fn the_first_event_is_exempt_from_the_fast_check() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert_eq!(w.event(10, 4), None);
    }

    #[test]
    fn the_deadline_only_tightens() {
        let mut w = Watch::new(0, 238, EVENT_MAX_US);
        assert!(w.tighten_max_interval(711));
        assert_eq!(w.max_interval(), 711);
        assert!(w.tighten_max_interval(900), "a looser request is accepted but ignored");
        assert_eq!(w.max_interval(), 711);
        assert!(!w.tighten_max_interval(100), "never below the minimum interval");
        assert_eq!(w.max_interval(), 711);
    }

    #[test]
    fn speed_limit_at_the_goal_speeds() {
        // 10%: ~419 us sector = 838 half-us -> capped at 1000 us.
        assert_eq!(speed_event_limit_us(838), 1_000);
        // 15%: ~237 us sector = 474 half-us -> 3 periods = 711 us.
        assert_eq!(speed_event_limit_us(474), 711);
    }
}
