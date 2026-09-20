//! Serialized, nesting-aware wall-time partition at software IRQ boundaries.
//! Context0 is foreground/unattributed, NOT idle. Hardware exception overhead
//! and probe overhead are not separately measured. Caller supplies a TIM17
//! sample inside the same critical section as each state mutation.
//! A >65ms blackout can alias: this is not an independent blackout watchdog.
pub const CONTEXTS: usize = 7;
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Meter {
    pub elapsed: u32,
    // Keep hot scalar state in immediate-offset range on Thumbv6-M.
    pub max_gap: u16,
    pub max_depth: u8,
    pub stopped_depth: u8,
    pub fault: u8,
    // Six nested identities fit in18bits; top3bits replace two indexed loads.
    stack: u32,
    depth: u8,
    mask: u8,
    last: u16,
    pub active: bool,
    pub started: bool,
    pub time: [u32; CONTEXTS],
    pub calls: [u32; CONTEXTS],
}
impl Meter {
    pub const fn new() -> Self {
        Self {
            elapsed: 0,
            time: [0; CONTEXTS],
            calls: [0; CONTEXTS],
            max_gap: 0,
            max_depth: 0,
            stopped_depth: 0,
            fault: 0,
            stack: 0,
            depth: 0,
            mask: 0,
            last: 0,
            active: false,
            started: false,
        }
    }
    /// Reset is separate so clearing arrays never delays a fresh seed's arm.
    #[inline(always)]
    pub fn begin(&mut self, now: u16) {
        if self.started {
            return;
        }
        self.last = now;
        self.started = true;
        self.active = true;
    }
    #[inline(always)]
    fn fail(&mut self, code: u8) {
        self.fault = code;
        self.active = false;
    }
    #[inline(always)]
    fn advance(&mut self, now: u16, context: usize) -> bool {
        if !self.active {
            return false;
        }
        let gap = now.wrapping_sub(self.last);
        self.max_gap = self.max_gap.max(gap);
        // Independent guard normally visits every100us. Reject a measurement
        // with a visible gap>1ms; never alter the motor's guard or authority.
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
        self.time[context] += gap as u32;
        true
    }
    #[inline(always)]
    pub fn enter(&mut self, id: u8, now: u16) -> bool {
        if !self.active {
            return false;
        }
        if id == 0
            || id as usize >= CONTEXTS
            || self.depth as usize + 1 >= CONTEXTS
            || self.mask & (1 << id) != 0
        {
            self.fail(3);
            return false;
        }
        if !self.advance(now, (self.stack & 7) as usize) {
            return false;
        }
        if self.calls[id as usize] == u32::MAX {
            self.fail(2);
            return false;
        }
        self.calls[id as usize] += 1;
        self.depth += 1;
        self.mask |= 1 << id;
        self.stack = (self.stack << 3) | id as u32;
        self.max_depth = self.max_depth.max(self.depth);
        true
    }
    #[inline(always)]
    pub fn leave(&mut self, id: u8, now: u16) {
        if !self.active {
            return;
        }
        if self.depth == 0 || self.stack & 7 != id as u32 {
            self.fail(4);
            return;
        }
        if self.advance(now, id as usize) {
            self.depth -= 1;
            self.mask &= !(1 << id);
            self.stack >>= 3;
        }
    }
    /// Stopping inside a guard IRQ is legitimate. Its later Drop is ignored.
    pub fn finish(&mut self, now: u16) {
        if !self.active {
            return;
        }
        self.advance(now, (self.stack & 7) as usize);
        self.stopped_depth = self.depth;
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_time_is_partitioned_not_double_counted() {
        let mut m = Meter::new();
        m.begin(0);
        assert!(m.enter(2, 10));
        assert!(m.enter(1, 20));
        m.leave(1, 25);
        m.leave(2, 40);
        m.finish(50);
        assert_eq!(m.time, [20, 5, 25, 0, 0, 0, 0]);
        assert_eq!(m.elapsed, 50);
        assert_eq!(m.calls, [0, 1, 1, 0, 0, 0, 0]);
        assert_eq!(m.max_depth, 2);
        assert_eq!(m.fault, 0);
    }
    #[test]
    fn all_six_contexts_unwind_and_can_reenter() {
        let mut m = Meter::new();
        m.begin(0);
        for id in 1..=6 {
            assert!(m.enter(id, id as u16));
        }
        for id in (1..=6).rev() {
            m.leave(id, 13 - id as u16);
        }
        assert_eq!(m.depth, 0);
        assert_eq!(m.mask, 0);
        assert_eq!(m.stack, 0);
        assert!(m.enter(6, 13));
        m.leave(6, 14);
        m.finish(15);
        assert_eq!(m.fault, 0);
        assert_eq!(m.max_depth, 6);
        assert_eq!(m.time.iter().sum::<u32>(), 15);
        assert_eq!(m.calls[6], 2);
    }
    #[test]
    fn guard_stop_freezes_open_frames_and_late_exits() {
        let mut m = Meter::new();
        m.begin(65530);
        m.enter(2, 65535);
        m.enter(1, 5);
        m.finish(10);
        let before = m.time;
        m.leave(1, 20);
        m.leave(2, 30);
        m.begin(40);
        assert_eq!(m.time, before);
        assert_eq!(m.elapsed, 16);
        assert_eq!(m.stopped_depth, 2);
        assert!(!m.active);
    }
    #[test]
    fn malformed_nesting_latches_invalid() {
        for id in [0, 7, 255] {
            let mut m = Meter::new();
            m.begin(0);
            assert!(!m.enter(id, 1));
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
        m.leave(1, 2);
        assert_eq!(m.fault, 4);
    }
    #[test]
    fn visible_clock_gap_and_duration_overflow_refuse() {
        let mut m = Meter::new();
        m.begin(0);
        m.enter(1, 1001);
        assert_eq!(m.fault, 1);
        let mut m = Meter::new();
        m.begin(0);
        m.elapsed = 600_000_000;
        m.enter(1, 1);
        assert_eq!(m.fault, 2);
        let mut m = Meter::new();
        m.begin(0);
        m.calls[1] = u32::MAX;
        m.enter(1, 1);
        assert_eq!(m.fault, 2);
    }
    #[test]
    fn long_run_wraps_keep_sum_equal_to_elapsed() {
        let mut m = Meter::new();
        m.begin(65000);
        for n in 0..100_000u32 {
            let base = 65000u32 + n * 100;
            m.enter(1, base as u16);
            m.leave(1, (base + 10) as u16);
        }
        m.finish((65000u32 + 10_000_000) as u16);
        assert_eq!(m.elapsed, 10_000_000);
        assert_eq!(m.time[1], 1_000_000);
        assert_eq!(m.time.iter().sum::<u32>(), m.elapsed);
        assert_eq!(m.fault, 0);
    }
}
