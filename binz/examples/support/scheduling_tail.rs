//! Diagnostic IRQ chronology, not control authority or a blackout watchdog.
//! Caller serializes each sample/transition. TIM17 must be sampled at least
//! once per 65.536 ms: a whole-wrap blackout cannot be detected from u16.
pub const CAPACITY: usize = 64;
const INDEX_MASK: usize = CAPACITY - 1;
const _: () = assert!(CAPACITY.is_power_of_two());
const STACK_MASK: u32 = (1 << 18) - 1;
pub const ENTER: u32 = 0;
pub const LEAVE: u32 = 1;
pub const STOP: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Row {
    pub elapsed_us: u32,
    /// Post-transition stack (six 3-bit IDs), kind at bit18, ID at bit20.
    /// A truncated tail therefore retains the active nesting context.
    pub packed: u32,
}
impl Row {
    pub const fn stack(self) -> u32 {
        self.packed & STACK_MASK
    }
    pub const fn kind(self) -> u32 {
        (self.packed >> 18) & 3
    }
    pub const fn id(self) -> u8 {
        ((self.packed >> 20) & 7) as u8
    }
}

pub struct Tail {
    rows: [Row; CAPACITY],
    pub count: u32,
    pub elapsed_us: u32,
    pub max_gap_us: u16,
    last: u16,
    stack: u32,
    depth: u8,
    mask: u8,
    pub fault: u8,
    pub started: bool,
    pub active: bool,
}
impl Tail {
    pub const fn new() -> Self {
        Self {
            rows: [Row {
                elapsed_us: 0,
                packed: 0,
            }; CAPACITY],
            count: 0,
            elapsed_us: 0,
            max_gap_us: 0,
            last: 0,
            stack: 0,
            depth: 0,
            mask: 0,
            fault: 0,
            started: false,
            active: false,
        }
    }
    pub fn begin(&mut self, now: u16) {
        if !self.started {
            self.last = now;
            self.started = true;
            self.active = true;
        }
    }
    fn fail(&mut self, code: u8) {
        self.fault = code;
        self.active = false;
    }
    fn stamp(&mut self, now: u16) -> bool {
        let gap = now.wrapping_sub(self.last);
        self.max_gap_us = self.max_gap_us.max(gap);
        if gap > 1000 {
            self.fail(1);
            return false;
        }
        if self.elapsed_us > 600_000_000 - gap as u32 || self.count == u32::MAX {
            self.fail(2);
            return false;
        }
        self.elapsed_us += gap as u32;
        self.last = now;
        true
    }
    fn push(&mut self, kind: u32, id: u8) {
        self.rows[self.count as usize & INDEX_MASK] = Row {
            elapsed_us: self.elapsed_us,
            packed: self.stack | (kind << 18) | ((id as u32) << 20),
        };
        self.count += 1;
    }
    pub fn enter(&mut self, id: u8, now: u16) -> bool {
        if !self.active {
            return false;
        }
        if id == 0 || id > 6 || self.depth == 6 || self.mask & (1 << id) != 0 {
            self.fail(3);
            return false;
        }
        if !self.stamp(now) {
            return false;
        }
        self.stack = (self.stack << 3) | id as u32;
        self.depth += 1;
        self.mask |= 1 << id;
        self.push(ENTER, id);
        true
    }
    pub fn leave(&mut self, id: u8, now: u16) {
        if !self.active {
            return;
        }
        if self.depth == 0 || self.stack & 7 != id as u32 {
            self.fail(4);
            return;
        }
        if !self.stamp(now) {
            return;
        }
        self.stack >>= 3;
        self.depth -= 1;
        self.mask &= !(1 << id);
        self.push(LEAVE, id);
    }
    pub fn finish(&mut self, now: u16) {
        if !self.active || !self.stamp(now) {
            return;
        }
        self.push(STOP, 0);
        self.active = false;
    }
    pub fn retained(&self) -> usize {
        (self.count as usize).min(CAPACITY)
    }
    pub fn omitted(&self) -> u32 {
        self.count.saturating_sub(CAPACITY as u32)
    }
    /// Chronological order; no invented entry for a leading partial handler.
    pub fn row(&self, index: usize) -> Option<Row> {
        if index >= self.retained() {
            return None;
        }
        let first = self.count as usize - self.retained();
        Some(self.rows[(first + index) & INDEX_MASK])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_actual_nesting_and_nested_stop_without_late_mutation() {
        let mut t = Tail::new();
        t.begin(65530);
        assert!(t.enter(2, 65535));
        assert!(t.enter(1, 5));
        assert_eq!(
            t.row(1),
            Some(Row {
                elapsed_us: 11,
                packed: 17 | (1 << 20)
            })
        );
        t.leave(1, 10);
        t.enter(6, 12);
        t.finish(14);
        let stop = t.row(4).unwrap();
        assert_eq!(
            (stop.elapsed_us, stop.stack(), stop.kind(), stop.id()),
            (20, 22, STOP, 0)
        );
        t.leave(6, 20);
        t.leave(2, 30);
        t.enter(1, 40);
        t.begin(50);
        assert_eq!(t.count, 5);
        assert_eq!(t.row(4), Some(stop));
    }
    #[test]
    fn truncated_history_keeps_stack_and_explicit_omission() {
        let mut t = Tail::new();
        t.begin(0);
        t.enter(2, 1);
        for n in 0..80 {
            t.enter(1, 2 + n * 2);
            t.leave(1, 3 + n * 2);
        }
        assert_eq!(t.count, 161);
        assert_eq!(t.retained(), 64);
        assert_eq!(t.omitted(), 97);
        let first = t.row(0).unwrap();
        assert_eq!((first.kind(), first.id(), first.stack()), (ENTER, 1, 17));
        for i in 1..64 {
            assert!(t.row(i).unwrap().elapsed_us > t.row(i - 1).unwrap().elapsed_us);
        }
        assert!(t.row(64).is_none());
        assert_eq!(core::mem::size_of::<Row>(), 8);
        assert!(core::mem::size_of::<Tail>() <= 544);
    }
    #[test]
    fn invalid_transitions_and_observed_gaps_freeze_evidence() {
        for id in [0, 7, 255] {
            let mut t = Tail::new();
            t.begin(0);
            assert!(!t.enter(id, 1));
            assert_eq!((t.fault, t.count), (3, 0));
        }
        let mut t = Tail::new();
        t.begin(0);
        t.enter(2, 1);
        t.enter(2, 2);
        assert_eq!((t.fault, t.count), (3, 1));
        let mut t = Tail::new();
        t.begin(0);
        t.enter(2, 1);
        t.leave(1, 2);
        assert_eq!((t.fault, t.count), (4, 1));
        let mut t = Tail::new();
        t.begin(0);
        t.enter(2, 1001);
        assert_eq!((t.fault, t.count), (1, 0));
        let mut t = Tail::new();
        t.begin(0);
        t.elapsed_us = 600_000_000;
        t.enter(2, 1);
        assert_eq!(t.fault, 2);
        let mut t = Tail::new();
        t.begin(0);
        t.count = u32::MAX;
        t.enter(2, 1);
        assert_eq!(t.fault, 2);
    }
    #[test]
    fn full_depth_and_repeated_clock_wraps() {
        let mut t = Tail::new();
        t.begin(0);
        for id in 1..=6 {
            assert!(t.enter(id, id as u16));
        }
        assert_eq!(t.row(5).unwrap().stack(), 0o123456);
        for id in (1..=6).rev() {
            t.leave(id, 13 - id as u16);
        }
        t.finish(13);
        assert_eq!(t.fault, 0);
        let mut t = Tail::new();
        t.begin(65000);
        for n in 0..100_000u32 {
            let at = 65000 + n * 100;
            t.enter(2, at as u16);
            t.enter(1, (at + 5) as u16);
            t.leave(1, (at + 10) as u16);
            t.leave(2, (at + 20) as u16);
        }
        t.finish((65000u32 + 10_000_000) as u16);
        assert_eq!((t.fault, t.elapsed_us, t.count), (0, 10_000_000, 400001));
    }
}
