//! Owned ADC scans. Caller serializes producer/consumer access; no DMA aliases.
//! Full means a latched refusal, never overwrite or drop an old sample.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub raw: [u16; 5],
    pub acquired: u32,
}
pub struct Queue {
    frames: [Frame; 8],
    head: u8,
    len: u8,
    peak: u8,
    failed: bool,
}
/// Test-only policy:0 armed,1 holding,2 spent. Preparation after a hold ends it.
pub struct StallOnce(u8);
impl StallOnce {
    pub const fn new() -> Self {
        Self(0)
    }
    pub fn prepare(&mut self) {
        if self.0 == 1 {
            self.0 = 2;
        }
    }
    pub fn hold(&mut self, delivered: u32) -> bool {
        if self.0 == 0 && delivered >= 20_000 {
            self.0 = 1;
        }
        self.0 == 1
    }
    pub fn state(&self) -> u8 {
        self.0
    }
}
impl Queue {
    pub const fn new() -> Self {
        Self {
            frames: [Frame {
                raw: [0; 5],
                acquired: 0,
            }; 8],
            head: 0,
            len: 0,
            peak: 0,
            failed: false,
        }
    }
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
        self.peak = 0;
        self.failed = false;
    }
    pub fn push(&mut self, frame: Frame) -> bool {
        if self.failed || self.len == 8 {
            self.failed = true;
            return false;
        }
        self.frames[((self.head + self.len) & 7) as usize] = frame;
        self.len += 1;
        #[cfg(not(feature = "bench-lean-irq"))]
        {
            self.peak = self.peak.max(self.len);
        }
        true
    }
    pub fn pop(&mut self) -> Option<Frame> {
        if self.failed || self.len == 0 {
            return None;
        }
        let frame = self.frames[self.head as usize];
        self.head = (self.head + 1) & 7;
        self.len -= 1;
        Some(frame)
    }
    pub fn peak(&self) -> u8 {
        self.peak
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(n: u32) -> Frame {
        Frame {
            raw: [n as u16; 5],
            acquired: n.wrapping_mul(101),
        }
    }
    #[test]
    fn stall_is_once_per_boot_and_prepare_releases_it() {
        let mut s = StallOnce::new();
        s.prepare();
        assert!(!s.hold(19999));
        assert!(s.hold(20000));
        assert!(s.hold(0));
        assert_eq!(s.state(), 1);
        s.prepare();
        assert!(!s.hold(20000));
        s.prepare();
        assert!(!s.hold(u32::MAX));
        assert_eq!(s.state(), 2);
    }
    #[test]
    fn preserves_order_and_original_timestamps_across_wraps() {
        let mut q = Queue::new();
        for base in 0..100 {
            for i in 0..8 {
                assert!(q.push(frame(base * 8 + i)));
            }
            for i in 0..8 {
                assert_eq!(q.pop(), Some(frame(base * 8 + i)));
            }
            assert_eq!(q.pop(), None);
        }
        assert_eq!(q.peak(), 8);
    }
    #[test]
    fn overflow_latches_until_explicit_restart() {
        let mut q = Queue::new();
        for i in 0..8 {
            assert!(q.push(frame(i)));
        }
        assert!(!q.push(frame(8)));
        assert_eq!(q.pop(), None);
        assert!(!q.push(frame(9)));
        q.clear();
        assert_eq!(q.pop(), None);
        assert_eq!(q.peak(), 0);
        assert!(q.push(frame(u32::MAX)));
        assert_eq!(q.pop(), Some(frame(u32::MAX)));
    }
    #[test]
    fn queued_old_scan_cannot_refresh_guard_age() {
        use crate::powered_guard::{Fault, Feedback, Guard};
        let sample = Feedback {
            phase: [2048; 3],
            bus_mv: 11800,
            vref: 1500,
        };
        let mut guard = Guard::with_limits(0, 0, 1, sample, 100_000, 10_000).unwrap();
        let mut q = Queue::new();
        assert!(q.push(Frame {
            raw: [2048, 2048, 2048, 1200, 1500],
            acquired: 101
        }));
        let scan = q.pop().unwrap();
        assert_eq!(guard.feedback_aged(900, 900 - scan.acquired, sample), None);
        assert_eq!(
            guard.feedback_aged(1102, 1102 - scan.acquired, sample),
            Some(Fault::FeedbackStale)
        );
    }
}
