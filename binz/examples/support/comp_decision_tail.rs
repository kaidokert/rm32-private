//! Bounded completed-reference-call history. No hardware access or authority.
//! Single writer; reset and dump require an inactive sensing epoch.
use super::comp_paths;

pub const CAPACITY: usize = 32;
const MASK: usize = CAPACITY - 1;
const _: () = assert!(CAPACITY.is_power_of_two());

/// Words: observation entry us, dispatched ordinal, packed count/average,
/// packed step/path/count-present. Timestamp is NOT physical edge arrival.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row(pub [u32; 4]);
const EMPTY: Row = Row([0; 4]);

pub struct Tail {
    rows: [Row; CAPACITY],
    total: u32,
    last_us: u32,
    next: usize,
    len: usize,
    frozen: bool,
    invalid: bool,
}
impl Tail {
    pub const fn new() -> Self {
        Self {
            rows: [EMPTY; CAPACITY],
            total: 0,
            last_us: 0,
            next: 0,
            len: 0,
            frozen: false,
            invalid: false,
        }
    }
    /// A stopped call is retained as unknown, then freezes history. Invalid
    /// metadata freezes without publishing a misleading row. No wrapping time
    /// inference: caller supplies epoch-relative32-bit time (finite campaign).
    pub fn push(
        &mut self,
        us: u32,
        step: u8,
        count: Option<u32>,
        average: u32,
        accepted_delta: u32,
        stopped: bool,
    ) -> bool {
        if self.frozen {
            return false;
        }
        if !(1..=6).contains(&step)
            || average > 65535
            || count.is_some_and(|v| v > 65535)
            || (self.total != 0 && us < self.last_us)
            || self.total == u32::MAX
        {
            self.invalid = true;
            self.frozen = true;
            return false;
        }
        let path = comp_paths::classify(count, average, accepted_delta, stopped);
        self.total += 1;
        let meta = step as u32 | ((path as u32) << 8) | ((count.is_some() as u32) << 16);
        self.rows[self.next] = Row([us, self.total, count.unwrap_or(0) | (average << 16), meta]);
        self.next = (self.next + 1) & MASK;
        if self.len < CAPACITY {
            self.len += 1;
        }
        self.last_us = us;
        if stopped {
            self.frozen = true;
        }
        true
    }
    pub fn freeze(&mut self) {
        self.frozen = true;
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn total(&self) -> u32 {
        self.total
    }
    pub fn omitted(&self) -> u32 {
        self.total - self.len as u32
    }
    pub fn frozen(&self) -> bool {
        self.frozen
    }
    pub fn invalid(&self) -> bool {
        self.invalid
    }
    pub fn row(&self, index: usize) -> Option<Row> {
        if index >= self.len {
            return None;
        }
        Some(self.rows[(self.next + CAPACITY - self.len + index) & MASK])
    }
}

#[cfg(test)]
mod tests {
    use super::comp_paths::Path;
    use super::*;
    #[test]
    fn wraps_in_order_without_hiding_omissions() {
        let mut t = Tail::new();
        for i in 0..100 {
            assert!(t.push(i, 2, Some(501), 1000, 0, false));
        }
        assert_eq!((t.len(), t.total(), t.omitted()), (32, 100, 68));
        for i in 0..32 {
            let r = t.row(i).unwrap();
            assert_eq!(r.0[0], 68 + i as u32);
            assert_eq!(r.0[1], 69 + i as u32);
        }
        assert_eq!(t.row(32), None);
        assert!(core::mem::size_of::<Tail>() <= 544);
    }
    #[test]
    fn stop_retained_unknown_and_never_overwritten() {
        let mut t = Tail::new();
        assert!(t.push(10, 3, Some(900), 1000, 1, true));
        assert_eq!(
            (t.row(0).unwrap().0[3] >> 8) & 255,
            Path::StoppedOrUnknown as u32
        );
        assert!(!t.push(11, 4, Some(900), 1000, 1, false));
        assert_eq!(t.total(), 1);
        assert!(t.frozen());
        assert!(!t.invalid());
    }
    #[test]
    fn count_presence_and_gate_boundary_are_preserved() {
        let mut t = Tail::new();
        for count in [None, Some(0), Some(500), Some(501), Some(65535)] {
            assert!(t.push(0, 1, count, 1000, 0, false));
        }
        assert_eq!(t.row(0).unwrap().0[3] >> 16, 0);
        assert_eq!(t.row(1).unwrap().0[3] >> 16, 1);
        assert_eq!((t.row(2).unwrap().0[3] >> 8) & 255, Path::Closed as u32);
        assert_eq!(
            (t.row(3).unwrap().0[3] >> 8) & 255,
            Path::OpenNoAccept as u32
        );
        assert_eq!(t.row(4).unwrap().0[2] & 65535, 65535);
    }
    #[test]
    fn bad_metadata_fails_closed_and_reset_is_fresh() {
        for (us, step, count, avg) in [
            (9, 1, Some(1), 1000),
            (11, 0, Some(1), 1000),
            (11, 7, Some(1), 1000),
            (11, 1, Some(65536), 1000),
            (11, 1, None, 65536),
        ] {
            let mut t = Tail::new();
            assert!(t.push(10, 1, None, 1000, 0, false));
            assert!(!t.push(us, step, count, avg, 0, false));
            assert!(t.invalid());
            assert!(t.frozen());
            assert_eq!(t.total(), 1);
        }
        let mut t = Tail::new();
        t.freeze();
        assert!(!t.push(0, 1, None, 1000, 0, false));
        t = Tail::new();
        assert!(t.push(0, 1, None, 1000, 0, false));
    }
    #[test]
    fn ordinal_overflow_does_not_wrap() {
        let mut t = Tail::new();
        t.total = u32::MAX;
        assert!(!t.push(0, 1, None, 1000, 0, false));
        assert!(t.invalid());
    }
}
