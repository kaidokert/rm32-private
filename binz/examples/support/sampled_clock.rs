//! Extend a sampled 16-bit microsecond counter without a second timer ISR.
//! Caller must serialize reset/sample AND the hardware read, and sample at
//! least once per 65536 us. This is not a watchdog for longer CPU blackouts.
//! The powered guard samples every 100 us and independently rejects >200 us
//! dispatch gaps. Accumulation preserves those gaps; it does not count ticks.
pub struct Clock {
    last: u16,
    elapsed: u32,
}
impl Clock {
    pub const fn new(counter: u16) -> Self {
        Self {
            last: counter,
            elapsed: 0,
        }
    }
    pub fn sample(&mut self, counter: u16) -> u32 {
        self.elapsed = self
            .elapsed
            .wrapping_add(counter.wrapping_sub(self.last) as u32);
        self.last = counter;
        self.elapsed
    }
    /// Map a source stamp into another clock without altering its age. Caller
    /// supplies one serialized hardware snapshot and keeps both clocks fed.
    pub fn map_stamp(&mut self, other: &mut Self, counter: u16, stamp: u32) -> u32 {
        let age = self.sample(counter).wrapping_sub(stamp);
        other.sample(counter).wrapping_sub(age)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_preserves_age_across_segment_reset_and_timestamp_wrap() {
        for (elapsed, stamp) in [(4_000_000, 3_999_950), (u32::MAX - 10, u32::MAX - 60)] {
            let mut source = Clock {
                last: 65000,
                elapsed,
            };
            let mut segment = Clock::new(65000);
            let mapped = source.map_stamp(&mut segment, 65025, stamp);
            assert_eq!(segment.sample(65025).wrapping_sub(mapped), 75);
            assert_eq!(source.sample(65025).wrapping_sub(stamp), 75);
        }
    }
    #[test]
    fn ten_seconds_across_many_hardware_wraps() {
        let origin = 65000u16;
        let mut c = Clock::new(origin);
        for elapsed in (0..=10_000_000u32).step_by(100) {
            assert_eq!(c.sample(origin.wrapping_add(elapsed as u16)), elapsed);
        }
    }
    #[test]
    fn sixty_seconds_crosses_powers_of_two_and_many_hardware_wraps() {
        let origin = 65000u16;
        let mut c = Clock::new(origin);
        for elapsed in (0..=60_000_000u32).step_by(100) {
            assert_eq!(c.sample(origin.wrapping_add(elapsed as u16)), elapsed);
        }
        // Covers 2^24 and 2^25 microseconds and 915 TIM17 wraps. The
        // observed high-duty faults do not land at either software boundary.
        assert_eq!(c.sample(origin.wrapping_add(60_000_000u32 as u16)), 60_000_000);
    }
    #[test]
    fn irregular_reads_preserve_real_dispatch_gap() {
        let mut c = Clock::new(65500);
        assert_eq!(c.sample(65530), 30);
        assert_eq!(c.sample(65530), 30);
        assert_eq!(c.sample(195), 231); // 201 us, not one nominal 100 us tick
        assert_eq!(c.sample(202), 238);
    }
    #[test]
    fn new_session_does_not_inherit_elapsed() {
        let mut c = Clock::new(123);
        assert_eq!(c.sample(123 + 1000), 1000);
        c = Clock::new(60000);
        assert_eq!(c.sample(60000), 0);
        assert_eq!(c.sample(20), 5556);
    }
    #[test]
    fn software_timestamp_wrap_is_modular() {
        let mut c = Clock {
            last: 65530,
            elapsed: u32::MAX - 5,
        };
        assert_eq!(c.sample(4), 4);
    }
}
