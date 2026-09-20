//! Sparse completed-dispatch evidence. No hardware access or control authority.
//! Caller checks `wanted` before collecting metadata. Ordinary calls are absent,
//! not counted as good; omissions below count overwritten selected rows only.
pub const THRESHOLD_US: u16 = 40;
pub const CAPACITY: usize = 16;
const MASK: usize = CAPACITY - 1;
const _: () = assert!(CAPACITY.is_power_of_two());

#[inline(always)]
pub const fn wanted(duration_us: u16, stopped: bool) -> bool {
    duration_us > THRESHOLD_US || stopped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub accepted_before: u32,
    pub entry_tick: u16,
    pub duration_us: u16,
    pub step: u8,
    pub accepted: bool,
    pub guard_overlap: bool,
    pub stopped: bool,
}
const EMPTY: Row = Row {
    accepted_before: 0,
    entry_tick: 0,
    duration_us: 0,
    step: 0,
    accepted: false,
    guard_overlap: false,
    stopped: false,
};

pub struct Tail {
    rows: [Row; CAPACITY],
    epoch: u32,
    total: u32,
    minimum_accepted: u32,
    frozen: bool,
    invalid: bool,
}
impl Tail {
    pub const fn new(epoch: u32) -> Self {
        Self {
            rows: [EMPTY; CAPACITY],
            epoch,
            total: 0,
            minimum_accepted: 0,
            frozen: epoch == 0,
            invalid: epoch == 0,
        }
    }
    /// Inactive producer only. Old storage is unreachable while len==0; reset
    /// metadata instead of clearing192bytes after a fresh handoff seed arrives.
    pub fn reset_epoch(&mut self, epoch: u32) {
        self.epoch = epoch;
        self.total = 0;
        self.minimum_accepted = 0;
        self.frozen = epoch == 0;
        self.invalid = epoch == 0;
    }
    /// One writer, only AFTER the selected call completes. The caller must not
    /// freeze from inside a safing callback while that call is still in flight.
    #[inline(always)]
    pub fn push(&mut self, epoch: u32, row: Row) -> bool {
        if self.frozen {
            return false;
        }
        // A stale writer cannot silently contaminate a new sensing epoch.
        if epoch != self.epoch
            || !(1..=6).contains(&row.step)
            || !wanted(row.duration_us, row.stopped)
            || (row.accepted && row.accepted_before == u32::MAX)
            || self.total == u32::MAX
        {
            self.invalidate();
            return false;
        }
        // Cache exactly the bound established by the last retained row. Avoid
        // indexing and unpacking that row again on every selected completion.
        if row.accepted_before < self.minimum_accepted {
            self.invalidate();
            return false;
        }
        self.minimum_accepted = row.accepted_before + row.accepted as u32;
        self.rows[self.total as usize & MASK] = row;
        self.total += 1;
        self.frozen = row.stopped;
        true
    }
    pub fn invalidate(&mut self) {
        self.invalid = true;
        self.frozen = true;
    }
    /// Only after producer quiescence; no fake stopped-call row is invented.
    pub fn freeze_idle(&mut self) {
        self.frozen = true;
    }
    pub fn row(&self, index: usize) -> Option<Row> {
        let len = self.len();
        if index >= len {
            None
        } else {
            Some(self.rows[(self.total as usize - len + index) & MASK])
        }
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn len(&self) -> usize {
        self.total.min(CAPACITY as u32) as usize
    }
    pub fn omitted(&self) -> u32 {
        self.total - self.len() as u32
    }
    pub fn frozen(&self) -> bool {
        self.frozen
    }
    pub fn invalid(&self) -> bool {
        self.invalid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row(n: u32) -> Row {
        Row {
            accepted_before: n,
            entry_tick: n as u16,
            duration_us: 41,
            step: 3,
            accepted: false,
            guard_overlap: false,
            stopped: false,
        }
    }
    #[test]
    fn trigger_is_strict_and_stop_is_unconditional() {
        for us in 0..=u16::MAX {
            assert_eq!(wanted(us, false), us > 40);
            assert!(wanted(us, true));
        }
    }
    #[test]
    fn ring_retains_selected_order_and_short_final_fault() {
        let mut t = Tail::new(7);
        for i in 0..50 {
            assert!(t.push(7, row(i)));
        }
        assert_eq!((t.len(), t.omitted()), (16, 34));
        for i in 0..16 {
            assert_eq!(t.row(i).unwrap().accepted_before, 34 + i as u32);
        }
        let last = Row {
            duration_us: 1,
            stopped: true,
            accepted: true,
            guard_overlap: true,
            ..row(50)
        };
        assert!(t.push(7, last));
        assert_eq!(t.row(15), Some(last));
        assert!(!t.push(7, row(51)));
        assert_eq!(t.row(15), Some(last));
        assert!(t.frozen());
        assert!(!t.invalid());
        assert!(core::mem::size_of::<Tail>() <= 224);
    }
    #[test]
    fn rejects_stale_epoch_bad_metadata_and_count_reversal() {
        assert!(Tail::new(0).invalid());
        for bad in [
            Row { step: 0, ..row(0) },
            Row { step: 7, ..row(0) },
            Row {
                duration_us: 40,
                ..row(0)
            },
            Row {
                accepted: true,
                ..row(u32::MAX)
            },
        ] {
            let mut t = Tail::new(1);
            assert!(!t.push(1, bad));
            assert!(t.invalid());
        }
        let mut t = Tail::new(2);
        assert!(!t.push(1, row(0)));
        assert!(t.invalid());
        let mut t = Tail::new(2);
        assert!(t.push(
            2,
            Row {
                accepted: true,
                ..row(10)
            }
        ));
        assert!(!t.push(2, row(10)));
        assert!(t.invalid());
        assert_eq!(t.len(), 1);
    }
    #[test]
    fn counter_overflow_and_idle_freeze_do_not_fabricate_rows() {
        let mut t = Tail::new(1);
        t.total = u32::MAX;
        assert!(!t.push(1, row(0)));
        assert!(t.invalid());
        let mut t = Tail::new(1);
        t.freeze_idle();
        assert_eq!(t.len(), 0);
        assert!(!t.invalid());
        assert!(!t.push(1, row(0)));
    }
    #[test]
    fn tick_wrap_is_retained_not_unwrapped_into_fake_time() {
        let mut t = Tail::new(1);
        assert!(t.push(
            1,
            Row {
                entry_tick: 65530,
                ..row(0)
            }
        ));
        assert!(t.push(
            1,
            Row {
                entry_tick: 12,
                ..row(0)
            }
        ));
        assert_eq!(t.row(1).unwrap().entry_tick, 12);
        assert_eq!(t.epoch(), 1);
        assert_eq!(t.row(2), None);
    }
    #[test]
    fn epoch_reset_hides_all_old_rows_without_clearing_storage() {
        let mut t = Tail::new(1);
        for i in 0..40 {
            assert!(t.push(1, row(i)));
        }
        let storage = t.rows;
        t.reset_epoch(2);
        assert_eq!(t.rows, storage);
        assert_eq!((t.len(), t.omitted()), (0, 0));
        for i in 0..CAPACITY {
            assert_eq!(t.row(i), None);
        }
        assert!(!t.invalid());
        assert!(!t.frozen());
        assert_eq!(t.epoch(), 2);
        assert!(t.push(2, row(0)));
        assert_eq!(t.row(0), Some(row(0)));
        assert_eq!(t.row(1), None);
        // A stale completion cannot resurrect hidden epoch1 records.
        assert!(!t.push(1, row(41)));
        assert!(t.invalid());
        assert_eq!(t.len(), 1);
        t.reset_epoch(0);
        assert!(t.invalid());
        assert!(t.frozen());
        assert_eq!(t.len(), 0);
    }
}
