//! Aggregate software IRQ union. Charge only outermost boundaries, including
//! all nested work once. Context0 is foreground/unattributed, NOT idle.
//! Serialized TIM17 reads required. Complete65ms blackout aliasing unsupported.
pub const CONTEXTS: usize = 2;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Meter {
    pub elapsed: u32,
    stack: u32,
    last: u16,
    pub max_gap: u16,
    depth: u8,
    mask: u8,
    pub max_depth: u8,
    pub stopped_depth: u8,
    pub fault: u8,
    pub active: bool,
    pub started: bool,
    pub time: [u32; 2],
    pub calls: [u32; 2],
    #[cfg(feature = "bench-cpu-roots")]
    root: u8,
    #[cfg(feature = "bench-cpu-roots")]
    pub root_time: [u32; 7],
}
impl Meter {
    #[inline(always)]
    pub fn entry_needs_time(&self) -> bool {
        self.active && self.depth == 0
    }
    #[inline(always)]
    pub fn exit_needs_time(&self) -> bool {
        self.active && self.depth == 1
    }
    pub const fn new() -> Self {
        Self {
            elapsed: 0,
            stack: 0,
            last: 0,
            max_gap: 0,
            depth: 0,
            mask: 0,
            max_depth: 0,
            stopped_depth: 0,
            fault: 0,
            active: false,
            started: false,
            time: [0; 2],
            calls: [0; 2],
            #[cfg(feature = "bench-cpu-roots")]
            root: 0,
            #[cfg(feature = "bench-cpu-roots")]
            root_time: [0; 7],
        }
    }
    #[inline(always)]
    pub fn begin(&mut self, now: u16) {
        if !self.started {
            self.last = now;
            self.started = true;
            self.active = true;
        }
    }
    #[inline(always)]
    fn fail(&mut self, code: u8) {
        self.fault = code;
        self.active = false;
    }
    #[inline(always)]
    fn charge(&mut self, now: u16, irq: bool) -> bool {
        let gap = now.wrapping_sub(self.last);
        self.max_gap = self.max_gap.max(gap);
        if gap > 1000 {
            self.fail(1);
            return false;
        }
        if self.elapsed > 600_000_000 - gap as u32 {
            self.fail(2);
            return false;
        }
        self.last = now;
        self.elapsed += gap as u32;
        #[cfg(not(feature = "bench-cpu-roots"))]
        {
            self.time[irq as usize] += gap as u32;
        }
        #[cfg(feature = "bench-cpu-roots")]
        if irq {
            self.root_time[self.root as usize] += gap as u32;
        } else {
            self.time[0] += gap as u32;
        }
        true
    }
    #[inline(always)]
    pub fn enter(&mut self, id: u8, now: u16) -> bool {
        self.enter_clock(id, || now)
    }
    /// Evaluate the clock once, only at a valid outer boundary. Caller keeps
    /// validation and sampling in one critical section.
    #[inline(always)]
    pub fn enter_clock(&mut self, id: u8, clock: impl FnOnce() -> u16) -> bool {
        if !self.active {
            return false;
        }
        if id == 0 || id > 6 || self.depth >= 6 || self.mask & (1 << id) != 0 {
            self.fail(3);
            return false;
        }
        if self.depth == 0 && !self.charge(clock(), false) {
            return false;
        }
        if self.calls[1] == u32::MAX {
            self.fail(2);
            return false;
        }
        #[cfg(feature = "bench-cpu-roots")]
        if self.depth == 0 {
            self.root = id;
        }
        self.calls[1] += 1;
        self.stack = (self.stack << 3) | id as u32;
        let next_mask = self.mask | (1 << id);
        #[cfg(feature = "bench-comp-overlap")]
        let next_mask = {
            // IDs1..6 own bits1..6. Bits0/7 hold guard/DMA overlap;
            // fold evidence into the mask store already required for nesting.
            if id == 2 {
                next_mask & 0x7e
            } else if self.mask & 4 != 0 && id == 1 {
                next_mask | 1
            } else if self.mask & 4 != 0 && id == 6 {
                next_mask | 0x80
            } else {
                next_mask
            }
        };
        self.mask = next_mask;
        self.depth += 1;
        self.max_depth = self.max_depth.max(self.depth);
        true
    }
    #[inline(always)]
    pub fn leave(&mut self, id: u8, now: u16) {
        self.leave_clock(id, || now);
    }
    #[inline(always)]
    pub fn leave_clock(&mut self, id: u8, clock: impl FnOnce() -> u16) {
        if !self.active {
            return;
        }
        if self.depth == 0 || self.stack & 7 != id as u32 {
            self.fail(4);
            return;
        }
        if self.depth == 1 && !self.charge(clock(), true) {
            return;
        }
        self.depth -= 1;
        self.stack >>= 3;
        self.mask &= !(1 << id);
    }
    pub fn finish(&mut self, now: u16) {
        if !self.active {
            return;
        }
        self.charge(now, self.depth != 0);
        self.stopped_depth = self.depth;
        self.active = false;
        // Root mode maintains the IRQ buckets instead of redundantly adding
        // every interval to both a bucket and the union. Publish union at stop.
        #[cfg(feature = "bench-cpu-roots")]
        {
            self.time[1] = self.elapsed - self.time[0];
        }
    }
    #[cfg(feature = "bench-comp-overlap")]
    pub fn stopped_comp_overlap(&self) -> Option<u8> {
        if self.started && !self.active && self.fault == 0 && self.mask & 4 != 0 {
            Some(((self.mask & 1) << 1) | ((self.mask & 0x80) >> 1))
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "bench-comp-overlap")]
    #[test]
    fn overlap_uses_existing_nesting_and_never_samples_nested_clock() {
        let mut m = Meter::new();
        m.begin(0);
        m.enter(1, 1);
        m.leave(1, 2); // guard before COMP is NOT overlap
        m.enter(2, 3);
        assert_eq!(m.mask & 0x81, 0);
        m.enter_clock(1, || panic!("extra clock"));
        m.leave_clock(1, || panic!("extra clock"));
        m.enter_clock(6, || panic!("extra clock"));
        m.leave_clock(6, || panic!("extra clock"));
        m.finish(20);
        assert_eq!(m.stopped_comp_overlap(), Some(66));
        m.leave(2, 30);
        m.enter(2, 40);
        assert_eq!(m.stopped_comp_overlap(), Some(66));
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.enter_clock(1, || panic!("extra clock"));
        m.leave_clock(1, || panic!("extra clock"));
        m.leave(2, 10);
        m.enter(2, 20);
        m.finish(30);
        assert_eq!(m.stopped_comp_overlap(), Some(0)); // new handler resets evidence
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.leave(2, 10);
        m.finish(20);
        assert_eq!(m.stopped_comp_overlap(), None); // foreground stop is not COMP
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.enter(2, 2);
        assert_eq!(m.stopped_comp_overlap(), None); // invalid meter is not evidence
    }
    #[cfg(feature = "bench-cpu-roots")]
    #[test]
    fn roots_include_nested_work_and_freeze_at_nested_stop() {
        let mut m = Meter::new();
        m.begin(65530);
        m.enter_clock(2, || 65535);
        m.enter_clock(1, || panic!("nested clock"));
        m.finish(10);
        m.leave(1, 20);
        m.leave(2, 30);
        assert_eq!(m.root_time, [0, 0, 11, 0, 0, 0, 0]);
        assert_eq!(m.root_time.iter().sum::<u32>(), m.time[1]);
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 10);
        m.enter_clock(1, || panic!("nested clock"));
        m.leave_clock(1, || panic!("nested clock"));
        m.leave(2, 40);
        m.enter(6, 50);
        m.leave(6, 60);
        m.finish(70);
        assert_eq!(m.root_time, [0, 0, 30, 0, 0, 0, 10]);
        assert_eq!(m.time, [30, 40]);
        assert_eq!(m.fault, 0);
        assert_eq!(Meter::new().root_time, [0; 7]);
    }
    #[test]
    fn deferred_clock_skips_inactive_nested_and_invalid_boundaries() {
        let no_clock = || -> u16 { panic!("unexpected clock read") };
        let mut m = Meter::new();
        assert!(!m.enter_clock(2, no_clock));
        m.leave_clock(2, no_clock);
        m.begin(0);
        assert!(m.enter_clock(2, || 10));
        assert!(m.enter_clock(1, no_clock));
        m.leave_clock(1, no_clock);
        m.leave_clock(2, || 40);
        m.finish(50);
        assert_eq!(m.time, [20, 30]);
        let mut m = Meter::new();
        m.begin(0);
        assert!(!m.enter_clock(7, no_clock));
        assert_eq!(m.fault, 3);
        let mut m = Meter::new();
        m.begin(0);
        m.enter_clock(2, || 1);
        m.leave_clock(1, no_clock);
        assert_eq!(m.fault, 4);
    }
    #[test]
    fn nested_union_counts_once_and_stops_inside_guard() {
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 10);
        m.enter(1, 20);
        m.leave(1, 25);
        m.leave(2, 40);
        m.finish(50);
        assert_eq!(m.time, [20, 30]);
        assert_eq!(m.elapsed, 50);
        assert_eq!(m.calls, [0, 2]);
        let mut m = Meter::new();
        m.begin(65530);
        m.enter(2, 65535);
        m.enter(1, 5);
        m.finish(10);
        assert_eq!(m.time, [5, 11]);
        assert_eq!(m.stopped_depth, 2);
        m.leave(1, 30);
        m.leave(2, 40);
        assert_eq!(m.elapsed, 16);
    }
    #[test]
    fn full_nesting_and_reentry() {
        let mut m = Meter::new();
        m.begin(0);
        for id in 1..=6 {
            assert!(m.enter(id, id as u16));
        }
        for id in (1..=6).rev() {
            m.leave(id, 13 - id as u16);
        }
        assert!(m.enter(6, 13));
        m.leave(6, 14);
        m.finish(15);
        assert_eq!(m.time, [3, 12]);
        assert_eq!(m.fault, 0);
        assert_eq!(m.calls[1], 7);
    }
    #[test]
    fn only_outer_boundaries_require_timestamps() {
        let mut m = Meter::new();
        assert!(!m.entry_needs_time());
        m.begin(100);
        assert!(m.entry_needs_time());
        m.enter(2, 110);
        assert!(!m.entry_needs_time());
        m.enter(1, 0);
        assert!(!m.exit_needs_time());
        m.leave(1, 0);
        assert!(m.exit_needs_time());
        m.leave(2, 140);
        m.finish(150);
        assert_eq!(m.time, [20, 30]);
        assert_eq!(m.fault, 0);
    }
    #[test]
    fn malformed_and_overflow_refuse() {
        for id in [0, 7, 255] {
            let mut m = Meter::new();
            m.begin(0);
            m.enter(id, 1);
            assert_eq!(m.fault, 3);
        }
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.enter(2, 2);
        assert_eq!(m.fault, 3);
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.leave(1, 2);
        assert_eq!(m.fault, 4);
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1001);
        assert_eq!(m.fault, 1);
        let mut m = Meter::new();
        m.begin(0);
        m.enter(2, 1);
        m.leave(2, 1002);
        assert_eq!(m.fault, 1);
        let mut m = Meter::new();
        m.begin(0);
        m.elapsed = 600_000_000;
        m.enter(2, 1);
        assert_eq!(m.fault, 2);
        let mut m = Meter::new();
        m.begin(0);
        m.calls[1] = u32::MAX;
        m.enter(2, 1);
        assert_eq!(m.fault, 2);
    }
    #[test]
    fn ten_seconds_matches_per_vector_union() {
        let mut m = Meter::new();
        let mut reference = crate::irq_accounting::Meter::new();
        m.begin(65000);
        reference.begin(65000);
        for n in 0..100_000u32 {
            let t = 65000 + n * 100;
            m.enter(2, t as u16);
            reference.enter(2, t as u16);
            m.enter(1, (t + 5) as u16);
            reference.enter(1, (t + 5) as u16);
            m.leave(1, (t + 10) as u16);
            reference.leave(1, (t + 10) as u16);
            m.leave(2, (t + 20) as u16);
            reference.leave(2, (t + 20) as u16);
        }
        m.finish((65000u32 + 10_000_000) as u16);
        reference.finish((65000u32 + 10_000_000) as u16);
        assert_eq!(m.elapsed, reference.elapsed);
        assert_eq!(m.time[0], reference.time[0]);
        assert_eq!(m.time[1], reference.time[1..].iter().sum());
        assert_eq!(m.fault, 0);
    }
}
