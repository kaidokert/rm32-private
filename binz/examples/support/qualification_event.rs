//! Per-accepted-boundary path counts. No hardware reads or output authority.
//! Epoch ownership and retained-row storage belong to the live adapter.
use super::comp_paths::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub accepted_before: u32,
    pub counts: [u16; 5],
    pub first_open: u16,
    pub last_open: u16,
    pub has_open: bool,
    pub step: u8,
    pub stopped: bool,
}
impl Row {
    const fn empty(accepted_before: u32) -> Self {
        Self {
            accepted_before,
            counts: [0; 5],
            first_open: 0,
            last_open: 0,
            has_open: false,
            step: 0,
            stopped: false,
        }
    }
}

pub struct Accumulator {
    row: Row,
    expected_step: u8,
    frozen: bool,
    invalid: bool,
}
impl Accumulator {
    pub const fn new(accepted_before: u32) -> Self {
        Self {
            row: Row::empty(accepted_before),
            expected_step: 0,
            frozen: false,
            invalid: false,
        }
    }
    pub fn invalid(&self) -> bool {
        self.invalid
    }
    pub fn frozen(&self) -> bool {
        self.frozen
    }
    /// A stop outside a dispatched call preserves an incomplete bucket, not
    /// a fabricated accepted boundary. Only the adapter may label/dump it.
    pub fn freeze(&mut self) {
        self.frozen = true;
    }
    pub fn partial(&self) -> Row {
        self.row
    }
    pub fn reject(&mut self) -> Option<Row> {
        self.invalid = true;
        self.frozen = true;
        None
    }
    /// `path` is classified from the real reference's first interval read.
    /// Open count is required only for open paths, never a replacement read.
    pub fn visit(&mut self, step: u8, path: Path, open_count: Option<u16>) -> Option<Row> {
        if self.accumulate(step, path, open_count) {
            self.finish()
        } else {
            None
        }
    }
    // Frequent path returns only a terminal flag, never a Row temporary.
    fn accumulate(&mut self, step: u8, path: Path, open_count: Option<u16>) -> bool {
        if self.frozen {
            return false;
        }
        if !(1..=6).contains(&step)
            || (self.expected_step != 0 && self.expected_step != step)
            || (self.row.step != 0 && self.row.step != step)
        {
            self.reject();
            return false;
        }
        let open = matches!(path, Path::OpenNoAccept | Path::Accepted);
        if open != open_count.is_some() {
            self.reject();
            return false;
        }
        let index = path as usize;
        let Some(count) = self.row.counts[index].checked_add(1) else {
            self.reject();
            return false;
        };
        self.row.step = step;
        self.row.counts[index] = count;
        if let Some(value) = open_count {
            if !self.row.has_open {
                self.row.first_open = value;
                self.row.has_open = true;
            }
            self.row.last_open = value;
        }
        if path == Path::StoppedOrUnknown {
            self.row.stopped = true;
            self.frozen = true;
            return true;
        }
        path == Path::Accepted
    }
    fn finish(&mut self) -> Option<Row> {
        if self.row.stopped {
            return Some(self.row);
        }
        let Some(next) = self.row.accepted_before.checked_add(1) else {
            return self.reject();
        };
        let result = self.row;
        self.row = Row::empty(next);
        self.expected_step = if result.step == 6 { 1 } else { result.step + 1 };
        Some(result)
    }
}

pub const CAPACITY: usize = 16;
// Thumb-v6M has short immediate load/store offsets. Keep frequently touched
// state ahead of the cold suffix storage instead of letting Rust place the
// 384-byte rows first. Only publication needs the large storage offset.
#[repr(C)]
pub struct History {
    accumulator: Accumulator,
    epoch: u32,
    total: u32,
    next: u8,
    len: u8,
    rows: [Row; CAPACITY],
}
impl History {
    pub const fn new() -> Self {
        Self {
            accumulator: Accumulator::new(0),
            rows: [Row::empty(0); CAPACITY],
            epoch: 0,
            total: 0,
            next: 0,
            len: 0,
        }
    }
    /// Reset metadata only, outside live observation. Old rows become invisible.
    pub fn reset(&mut self, epoch: u32) {
        self.accumulator = Accumulator::new(0);
        self.epoch = epoch;
        self.total = 0;
        self.next = 0;
        self.len = 0;
        if epoch == 0 {
            self.invalidate();
        }
    }
    pub fn invalidate(&mut self) {
        self.accumulator.reject();
    }
    pub fn freeze(&mut self) {
        self.accumulator.freeze();
    }
    pub fn invalid(&self) -> bool {
        self.accumulator.invalid() || self.epoch == 0
    }
    pub fn frozen(&self) -> bool {
        self.accumulator.frozen()
    }
    pub fn epoch(&self) -> u32 {
        self.epoch
    }
    pub fn total(&self) -> u32 {
        self.total
    }
    pub fn len(&self) -> usize {
        self.len as usize
    }
    pub fn omitted(&self) -> u32 {
        self.total - self.len as u32
    }
    pub fn partial(&self) -> Row {
        self.accumulator.partial()
    }
    pub fn row(&self, index: usize) -> Option<Row> {
        if index >= self.len() {
            return None;
        }
        let oldest = (self.next as usize + CAPACITY - self.len()) & (CAPACITY - 1);
        Some(self.rows[(oldest + index) & (CAPACITY - 1)])
    }
    pub fn visit(&mut self, accepted_before: u32, step: u8, path: Path, count: Option<u16>) {
        if self.invalid() || self.frozen() {
            return;
        }
        if accepted_before != self.accumulator.partial().accepted_before {
            self.invalidate();
            return;
        }
        if self.accumulator.accumulate(step, path, count) {
            self.publish();
        }
    }
    // Keep row copy/reset and ring indexing off the frequent call's frame.
    #[inline(never)]
    fn publish(&mut self) {
        if let Some(row) = self.accumulator.finish() {
            let Some(total) = self.total.checked_add(1) else {
                self.invalidate();
                return;
            };
            self.rows[self.next as usize] = row;
            self.next = (self.next + 1) & (CAPACITY as u8 - 1);
            self.len = self.len.saturating_add(1).min(CAPACITY as u8);
            self.total = total;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_and_identity_across_two_cycles() {
        let mut a = Accumulator::new(90);
        for n in 0..12 {
            let step = (n % 6 + 1) as u8;
            assert_eq!(a.visit(step, Path::Closed, None), None);
            assert_eq!(a.visit(step, Path::OpenNoAccept, Some(601)), None);
            let r = a.visit(step, Path::Accepted, Some(950)).unwrap();
            assert_eq!(r.accepted_before, 90 + n);
            assert_eq!(r.counts, [0, 1, 1, 1, 0]);
            assert_eq!((r.first_open, r.last_open), (601, 950));
        }
    }
    #[test]
    fn stop_is_unknown_not_rejection_or_acceptance() {
        let mut a = Accumulator::new(0);
        a.visit(3, Path::NoGate, None);
        let r = a.visit(3, Path::StoppedOrUnknown, None).unwrap();
        assert_eq!(r.counts, [1, 0, 0, 0, 1]);
        assert!(r.stopped);
        assert_eq!(a.visit(3, Path::Accepted, Some(800)), None);
        assert_eq!(a.partial(), r);
        assert!(!a.invalid());
    }
    #[test]
    fn missing_count_and_sector_gap_are_invalid() {
        let mut a = Accumulator::new(0);
        a.visit(1, Path::Accepted, None);
        assert!(a.invalid());
        let mut a = Accumulator::new(0);
        a.visit(1, Path::Accepted, Some(65535));
        a.visit(3, Path::Closed, None);
        assert!(a.invalid());
        let mut a = Accumulator::new(0);
        a.visit(1, Path::Closed, None);
        a.visit(2, Path::Closed, None);
        assert!(a.invalid());
    }
    #[test]
    fn overflow_never_masquerades_as_zero() {
        let mut a = Accumulator::new(0);
        for _ in 0..65535 {
            a.visit(1, Path::Closed, None);
        }
        a.visit(1, Path::Closed, None);
        assert!(a.invalid());
        assert_eq!(a.partial().counts[1], 65535);
        let mut a = Accumulator::new(u32::MAX);
        assert_eq!(a.visit(1, Path::Accepted, Some(800)), None);
        assert!(a.invalid());
    }
    #[test]
    fn external_stop_is_partial_and_new_epoch_clears_it() {
        let mut a = Accumulator::new(3);
        a.visit(4, Path::OpenNoAccept, Some(65535));
        a.freeze();
        assert_eq!(a.visit(4, Path::Accepted, Some(65535)), None);
        assert!(a.partial().has_open);
        assert!(!a.partial().stopped);
        a = Accumulator::new(0);
        assert!(!a.frozen());
        assert_eq!(a.partial().counts, [0; 5]);
        assert!(core::mem::size_of::<Accumulator>() <= 32);
        assert!(core::mem::size_of::<Row>() <= 24);
    }
    #[test]
    fn history_wrap_stop_and_reset() {
        let mut h = History::new();
        h.reset(7);
        for n in 0..40 {
            h.visit(n, (n % 6 + 1) as u8, Path::Accepted, Some(900));
        }
        assert_eq!((h.total(), h.len(), h.omitted()), (40, 16, 24));
        assert_eq!(h.row(0).unwrap().accepted_before, 24);
        h.visit(40, 5, Path::StoppedOrUnknown, None);
        assert!(h.row(15).unwrap().stopped);
        assert!(h.frozen());
        assert_eq!(h.row(0).unwrap().accepted_before, 25);
        h.reset(8);
        assert_eq!(h.row(0), None);
        assert!(!h.frozen());
        assert_eq!(h.epoch(), 8);
        assert_eq!(h.partial().counts, [0; 5]);
        assert!(core::mem::size_of::<History>() <= 448);
    }
    #[test]
    fn history_binding_and_zero_epoch_refuse() {
        let mut h = History::new();
        h.reset(1);
        h.visit(1, 1, Path::Closed, None);
        assert!(h.invalid());
        h.reset(0);
        h.visit(0, 1, Path::Accepted, Some(900));
        assert!(h.invalid());
        assert_eq!(h.total(), 0);
    }
    #[test]
    fn hot_history_metadata_precedes_cold_rows() {
        assert_eq!(core::mem::offset_of!(History, accumulator), 0);
        assert!(core::mem::offset_of!(History, epoch) < 64);
        assert!(core::mem::offset_of!(History, len) < 64);
        assert!(core::mem::offset_of!(History, rows) <= 64);
    }
}
