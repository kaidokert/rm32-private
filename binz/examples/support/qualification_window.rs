//! Pure bounded per-acceptance diagnostic accumulator. No control authority.
//! Caller owns serialization and supplies a completed call's measured duration.
//! Guard overlap counts calls that span guard progress, not interrupt latency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Window {
    pub calls: u16,
    pub guard_overlap_calls: u16,
    pub max_call_us: u16,
    pub saturated: bool,
}
impl Window {
    pub const fn new() -> Self {
        Self {
            calls: 0,
            guard_overlap_calls: 0,
            max_call_us: 0,
            saturated: false,
        }
    }
    pub fn completed(&mut self, duration_us: u16, guard_before: u32, guard_after: u32) {
        if self.calls == u16::MAX {
            self.saturated = true;
        } else {
            self.calls += 1;
        }
        if guard_before != guard_after {
            if self.guard_overlap_calls == u16::MAX {
                self.saturated = true;
            } else {
                self.guard_overlap_calls += 1;
            }
        }
        self.max_call_us = self.max_call_us.max(duration_us);
    }
    /// Complete the current acceptance window, returning evidence before reset.
    /// A fault must snapshot/freeze instead: never discard the failing window.
    pub fn take(&mut self) -> Self {
        core::mem::replace(self, Self::new())
    }
}

pub const CAPACITY: usize = 16;
const MASK: usize = CAPACITY - 1;
const _: () = assert!(CAPACITY.is_power_of_two());
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub window: Window,
    /// Diagnostic accepted count, NOT a physical rotor edge ordinal.
    pub accepted_total: u32,
    pub step: u8,
    pub accepted: bool,
    pub stopped: bool,
}
const EMPTY: Row = Row {
    window: Window::new(),
    accepted_total: 0,
    step: 0,
    accepted: false,
    stopped: false,
};
pub struct History {
    rows: [Row; CAPACITY],
    pending: Window,
    next: usize,
    len: usize,
    accepted_total: u32,
    total: u32,
    frozen: bool,
    invalid: bool,
}
impl History {
    pub const fn new() -> Self {
        Self {
            rows: [EMPTY; CAPACITY],
            pending: Window::new(),
            next: 0,
            len: 0,
            accepted_total: 0,
            total: 0,
            frozen: false,
            invalid: false,
        }
    }
    /// Publish AFTER the reference call returns, including a call whose
    /// callback stopped the motor. No-accept calls update only the accumulator.
    pub fn complete(
        &mut self,
        duration_us: u16,
        guard_before: u32,
        guard_after: u32,
        step: u8,
        accepted_delta: u32,
        stopped: bool,
    ) -> bool {
        if self.frozen {
            return false;
        }
        if !(1..=6).contains(&step)
            || accepted_delta > 1
            || (accepted_delta == 1 && self.accepted_total == u32::MAX)
            || self.total == u32::MAX
        {
            self.invalid = true;
            self.frozen = true;
            return false;
        }
        self.pending
            .completed(duration_us, guard_before, guard_after);
        if accepted_delta == 0 && !stopped {
            return true;
        }
        self.publish(step, accepted_delta, stopped);
        true
    }
    // Keep row-publication register pressure off the frequent no-accept path.
    // Validation and accumulation above are unchanged; this is not a fast path
    // that skips metadata checks or drops completed calls.
    #[inline(never)]
    fn publish(&mut self, step: u8, accepted_delta: u32, stopped: bool) {
        self.accepted_total += accepted_delta;
        self.rows[self.next] = Row {
            window: self.pending.take(),
            accepted_total: self.accepted_total,
            step,
            accepted: accepted_delta == 1,
            stopped,
        };
        self.next = (self.next + 1) & MASK;
        self.len = (self.len + 1).min(CAPACITY);
        self.total += 1;
        self.frozen = stopped;
    }
    /// External stop with no call in flight: retain pending evidence without
    /// fabricating an acceptance or a zero-duration completed call.
    pub fn freeze_idle(&mut self) {
        self.frozen = true;
    }
    pub fn pending(&self) -> Window {
        self.pending
    }
    pub fn row(&self, index: usize) -> Option<Row> {
        if index >= self.len {
            None
        } else {
            Some(self.rows[(self.next + CAPACITY - self.len + index) & MASK])
        }
    }
    pub fn len(&self) -> usize {
        self.len
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
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acceptance_history_wraps_and_retains_fault_call() {
        let mut h = History::new();
        for i in 0..40 {
            assert!(h.complete(6, 0, 0, 1, 0, false));
            assert!(h.complete(30, 0, 1, (i % 6 + 1) as u8, 1, false));
        }
        assert_eq!((h.len(), h.omitted()), (16, 24));
        assert_eq!(h.row(0).unwrap().accepted_total, 25);
        assert_eq!(h.row(15).unwrap().window.calls, 2);
        assert!(h.complete(42, 1, 2, 5, 0, true));
        let last = h.row(15).unwrap();
        assert_eq!(last.accepted_total, 40);
        assert!(!last.accepted);
        assert!(last.stopped);
        assert_eq!(last.window.max_call_us, 42);
        assert!(!h.complete(1, 2, 2, 5, 1, false));
        assert_eq!(h.row(15), Some(last));
        assert!(core::mem::size_of::<History>() <= 320);
    }
    #[test]
    fn stop_after_accept_and_idle_stop_are_distinct() {
        let mut h = History::new();
        assert!(h.complete(8, 0, 0, 1, 1, true));
        assert!(h.row(0).unwrap().accepted);
        assert!(h.frozen());
        let mut h = History::new();
        h.complete(9, 0, 1, 1, 0, false);
        h.freeze_idle();
        assert_eq!(h.len(), 0);
        assert_eq!(h.pending().calls, 1);
        assert!(!h.complete(7, 1, 1, 1, 1, false));
    }
    #[test]
    fn invalid_metadata_and_ordinal_overflow_freeze_diagnostics() {
        for (step, delta) in [(0, 0), (7, 1), (1, 2)] {
            let mut h = History::new();
            assert!(!h.complete(1, 0, 0, step, delta, false));
            assert!(h.invalid());
            assert_eq!(h.len(), 0);
        }
        let mut h = History::new();
        h.accepted_total = u32::MAX;
        assert!(!h.complete(1, 0, 0, 1, 1, false));
        assert!(h.invalid());
        let mut h = History::new();
        h.total = u32::MAX;
        assert!(!h.complete(1, 0, 0, 1, 0, true));
        assert!(h.invalid());
    }
    #[test]
    fn aggregates_completed_calls_without_claiming_tick_count() {
        let mut w = Window::new();
        w.completed(7, 10, 10);
        w.completed(25, 10, 12);
        w.completed(9, u32::MAX, 0);
        assert_eq!(
            w,
            Window {
                calls: 3,
                guard_overlap_calls: 2,
                max_call_us: 25,
                saturated: false
            }
        );
        assert_eq!(w.take().calls, 3);
        assert_eq!(w, Window::new());
    }
    #[test]
    fn saturates_with_explicit_loss_not_wrap() {
        let mut w = Window::new();
        for _ in 0..=u16::MAX {
            w.completed(1, 0, 1);
        }
        assert_eq!(w.calls, u16::MAX);
        assert_eq!(w.guard_overlap_calls, u16::MAX);
        assert!(w.saturated);
        assert!(w.take().saturated);
        assert!(!w.saturated);
    }
    #[test]
    fn copied_fault_snapshot_survives_later_reset() {
        let mut w = Window::new();
        w.completed(40, 3, 4);
        let fault = w;
        w.take();
        assert_eq!(fault.max_call_us, 40);
        assert_eq!(w.calls, 0);
        assert!(core::mem::size_of::<Window>() <= 8);
    }
}
