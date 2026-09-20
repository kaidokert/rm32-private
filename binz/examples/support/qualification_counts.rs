//! Direct first-read counters, not persistence outcomes or edge timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counts {
    pub dispatched: u16,
    pub closed: u16,
    pub open: u16,
    pub first_open: u16,
    pub last_open: u16,
}
impl Counts {
    pub const ZERO: Self = Self {
        dispatched: 0,
        closed: 0,
        open: 0,
        first_open: 0,
        last_open: 0,
    };
}
// Already in retained-row order: dispatched<<16, closed|open<<16,
// first_open|last_open<<16. Keeping these words avoids five halfword
// loads and repacking at each accepted event on thumbv6m.
const INACTIVE: u8 = 0;
const UNREAD: u8 = 1;
const CONSUMED: u8 = 2;
const READ: u8 = 3; // Current dispatch's first read was open and not accepted yet.
pub struct Counter {
    words: [u32; 3],
    state: u8,
    frozen: bool,
    invalid: bool,
}
impl Counter {
    pub const fn new() -> Self {
        Self {
            words: [0; 3],
            state: INACTIVE,
            frozen: false,
            invalid: false,
        }
    }
    #[inline]
    pub fn active(&self) -> bool {
        self.state != INACTIVE
    }
    pub fn invalid(&self) -> bool {
        self.invalid
    }
    fn unpack(w: [u32; 3]) -> Counts {
        Counts {
            dispatched: (w[0] >> 16) as u16,
            closed: w[1] as u16,
            open: (w[1] >> 16) as u16,
            first_open: w[2] as u16,
            last_open: (w[2] >> 16) as u16,
        }
    }
    pub fn snapshot(&self) -> Counts {
        Self::unpack(self.words)
    }
    /// Presence only: bit i denotes an observed reference mismatch at read i.
    pub fn rejection_mask(&self) -> u16 {
        ((self.words[0] >> 4) & 0xfff) as u16
    }
    /// Caller supplies the actual reference mismatch index, never an inferred
    /// rejection from a missing acceptance. Reserved bits leave sector0..6
    /// and the checked dispatched count unchanged. Not connected in v1 builds.
    #[inline]
    pub fn persistence_rejected(&mut self, read_index: u16) {
        if self.state != READ {
            if !self.frozen {
                self.reject();
            }
            return;
        }
        // READ implies !frozen; preserve frozen partials on the cold path.
        if read_index >= 12 {
            self.reject();
            return;
        }
        self.words[0] |= 1u32 << (read_index as u32 + 4);
    }
    pub fn freeze(&mut self) {
        self.frozen = true;
        self.state = INACTIVE;
    }
    fn reject(&mut self) {
        self.invalid = true;
        self.freeze();
    }
    /// Close first-read collection on every dispatch exit, including no-read.
    #[inline]
    pub fn end(&mut self) {
        self.state = INACTIVE;
    }
    /// Called once per dispatched reference visit, not skipped IRQ.
    #[inline]
    pub fn begin(&mut self) {
        if self.frozen {
            return;
        }
        let Some(next) = self.words[0].checked_add(1 << 16) else {
            self.reject();
            return;
        };
        self.words[0] = next;
        self.state = UNREAD;
    }
    /// First actual interval read only. Caller supplies a gate threshold
    /// consistent with the reference comparison; no new peripheral read.
    #[inline]
    pub fn first_count(&mut self, count: u16, half_average: u32) {
        // Only successful begin enters UNREAD. First read consumes it;
        // every exit/freeze closes both collection and dispatch ownership.
        if self.state != UNREAD {
            return;
        }
        self.state = CONSUMED;
        // Each bin is bounded by dispatched, whose increment is checked.
        if count as u32 <= half_average {
            self.words[1] += 1;
        } else {
            self.state = READ;
            let first = if self.words[1] >> 16 == 0 {
                count as u32
            } else {
                self.words[2] & 65535
            };
            self.words[2] = first | ((count as u32) << 16);
            self.words[1] += 1 << 16;
        }
    }
    /// Caller has independently committed acceptance. No publish on a veto.
    pub fn accepted(&mut self) -> Option<Counts> {
        self.accepted_words().map(Self::unpack)
    }
    /// Same validation, directly usable as the last three retained-row words.
    pub fn accepted_words(&mut self) -> Option<[u32; 3]> {
        if self.state != READ {
            // Freeze always revokes ownership; ordinary frozen partials are
            // not invalid. Other non-read acceptance attempts are invalid.
            if !self.frozen {
                self.reject();
            }
            return None;
        }
        // READ proves this dispatch has an open read and has not published.
        let result = self.words;
        self.words = [0; 3];
        self.state = CONSUMED;
        // Ignore any extra count reads in this same accepted call.
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prior_open_does_not_authorize_closed_dispatch_callbacks() {
        for acceptance in [false, true] {
            let mut c = Counter::new();
            c.begin();
            c.first_count(600, 500);
            c.end();
            c.begin();
            c.first_count(400, 500);
            if acceptance {
                assert_eq!(c.accepted_words(), None);
            } else {
                c.persistence_rejected(0);
            }
            assert!(c.invalid());
            assert_eq!(c.rejection_mask(), 0);
        }
    }
    #[test]
    fn acceptance_consumes_callback_authority() {
        for acceptance in [false, true] {
            let mut c = Counter::new();
            c.begin();
            c.first_count(600, 500);
            assert!(c.accepted_words().is_some());
            c.first_count(700, 500);
            assert_eq!(c.snapshot(), Counts::ZERO);
            if acceptance {
                assert_eq!(c.accepted_words(), None);
            } else {
                c.persistence_rejected(0);
            }
            assert!(c.invalid());
            assert_eq!(c.rejection_mask(), 0);
            c.begin();
            c.first_count(800, 500);
            assert_eq!(c.snapshot(), Counts::ZERO);
        }
    }
    #[test]
    fn rejection_mask_preserves_counts_and_all_twelve_positions() {
        let mut c = Counter::new();
        for index in 0..12 {
            c.begin();
            c.first_count(600 + index, 500);
            let counts = c.snapshot();
            c.persistence_rejected(index);
            c.persistence_rejected(index);
            assert_eq!(c.snapshot(), counts);
            assert_eq!(c.rejection_mask(), (1u16 << (index + 1)) - 1);
            c.end();
        }
        c.begin();
        c.first_count(800, 500);
        let words = c.accepted_words().unwrap();
        assert_eq!(words[0], (13 << 16) | 0xfff0);
        assert_eq!(words[0] & 15, 0); // sector bits remain available
        assert_eq!(c.rejection_mask(), 0);
        assert_eq!(c.snapshot(), Counts::ZERO);
    }
    #[test]
    fn rejection_mask_survives_partial_freeze_and_invalid_indices_refuse() {
        let mut c = Counter::new();
        c.begin();
        c.first_count(600, 500);
        c.persistence_rejected(11);
        c.freeze();
        let counts = c.snapshot();
        c.persistence_rejected(0);
        assert_eq!(c.rejection_mask(), 1 << 11);
        assert_eq!(c.snapshot(), counts);
        assert!(!c.invalid());
        for index in [12, 16, u16::MAX] {
            let mut c = Counter::new();
            c.begin();
            c.first_count(600, 500);
            c.persistence_rejected(index);
            assert!(c.invalid());
            assert!(!c.active());
            assert_eq!(c.rejection_mask(), 0);
            assert_eq!(c.accepted_words(), None);
        }
        for mode in 0..3 {
            let mut c = Counter::new();
            if mode != 0 {
                c.begin();
            }
            if mode == 2 {
                c.first_count(600, 500);
                c.end();
            }
            c.persistence_rejected(0);
            assert!(c.invalid());
        }
    }
    #[test]
    fn rejection_mask_does_not_carry_into_checked_dispatch_count() {
        let mut c = Counter::new();
        for _ in 0..65535 {
            c.begin();
            c.first_count(600, 500);
            c.persistence_rejected(11);
            c.end();
        }
        assert_eq!(c.snapshot().dispatched, 65535);
        assert_eq!(c.snapshot().open, 65535);
        assert_eq!(c.rejection_mask(), 1 << 11);
        let counts = c.snapshot();
        c.begin();
        assert!(c.invalid());
        assert_eq!(c.snapshot(), counts);
        assert_eq!(c.rejection_mask(), 1 << 11);
    }
    #[test]
    fn full_width_threshold_matches_clamp_for_every_count() {
        for threshold in [0, 1, 500, 65534, 65535, 65536, 0x7fff_ffff, u32::MAX] {
            for count in 0..=u16::MAX {
                let mut c = Counter::new();
                c.begin();
                c.first_count(count, threshold);
                let expected_closed = count <= threshold.min(65535) as u16;
                assert_eq!(c.snapshot().closed, expected_closed as u16);
                assert_eq!(c.snapshot().open, (!expected_closed) as u16);
            }
        }
    }
    #[test]
    fn packed_words_preserve_mixed_maximum_and_reuse() {
        let mut c = Counter::new();
        for i in 0..65535 {
            c.begin();
            c.first_count(if i % 2 == 0 { 600 } else { 400 }, 500);
            if i != 65534 {
                c.end();
            }
        }
        assert_eq!(
            c.snapshot(),
            Counts {
                dispatched: 65535,
                closed: 32767,
                open: 32768,
                first_open: 600,
                last_open: 600
            }
        );
        assert_eq!(
            c.accepted_words(),
            Some([65535 << 16, 32767 | (32768 << 16), 600 | (600 << 16)])
        );
        assert_eq!(c.snapshot(), Counts::ZERO);
        c.begin();
        c.first_count(65535, 500);
        assert_eq!(c.accepted_words(), Some([1 << 16, 1 << 16, 0xffff_ffff]));
        assert_eq!(c.snapshot(), Counts::ZERO);
    }
    #[test]
    fn only_first_read_and_strict_gate() {
        let mut c = Counter::new();
        c.begin();
        c.first_count(500, 500);
        c.first_count(999, 500);
        c.begin();
        c.first_count(501, 500);
        c.first_count(0, 500);
        assert_eq!(
            c.accepted(),
            Some(Counts {
                dispatched: 2,
                closed: 1,
                open: 1,
                first_open: 501,
                last_open: 501
            })
        );
        c.first_count(900, 500);
        assert_eq!(c.snapshot(), Counts::ZERO);
    }
    #[test]
    fn ownership_closes_on_exit_and_requires_a_current_read() {
        let mut c = Counter::new();
        assert!(!c.active());
        c.begin();
        assert!(c.active());
        c.first_count(600, 500);
        c.end();
        assert!(!c.active());
        let saved = c.snapshot();
        assert_eq!(c.accepted_words(), None);
        assert!(c.invalid());
        assert_eq!(c.snapshot(), saved);
        let mut c = Counter::new();
        c.begin();
        c.first_count(600, 500);
        c.end();
        c.begin(); // Earlier open count cannot stand in for this visit's read.
        assert_eq!(c.accepted_words(), None);
        assert!(!c.active());
    }
    #[test]
    fn no_gate_and_multiple_open_calls_not_rejections() {
        let mut c = Counter::new();
        c.begin(); // no actual gate read
        for count in [700, 800, 900] {
            c.begin();
            c.first_count(count, 500);
        }
        let r = c.accepted().unwrap();
        assert_eq!(
            (r.dispatched, r.closed, r.open, r.first_open, r.last_open),
            (4, 0, 3, 700, 900)
        );
    }
    #[test]
    fn overflow_is_sticky_and_bins_are_bounded() {
        for open in [false, true] {
            let mut c = Counter::new();
            for _ in 0..65535 {
                c.begin();
                c.first_count(if open { 65535 } else { 0 }, 500);
            }
            assert_eq!(c.snapshot().dispatched, 65535);
            c.begin();
            assert!(c.invalid());
            let r = c.snapshot();
            c.first_count(999, 500);
            assert_eq!(c.snapshot(), r);
            assert_eq!(c.accepted(), None);
        }
    }
    #[test]
    fn partial_stop_and_bad_acceptance_do_not_fabricate_rows() {
        let mut c = Counter::new();
        c.begin();
        c.first_count(800, 500);
        c.freeze();
        assert_eq!(c.accepted(), None);
        assert_eq!(c.snapshot().open, 1);
        assert!(!c.invalid());
        let mut c = Counter::new();
        assert_eq!(c.accepted(), None);
        assert!(c.invalid());
        let mut c = Counter::new();
        c.begin();
        c.first_count(100, 500);
        assert_eq!(c.accepted(), None);
        assert!(c.invalid());
        assert!(core::mem::size_of::<Counter>() <= 16);
    }
    #[test]
    fn ended_and_frozen_no_read_calls_ignore_late_reads() {
        let mut c = Counter::new();
        c.begin();
        c.end();
        let saved = c.snapshot();
        c.first_count(900, 500);
        assert_eq!(c.snapshot(), saved);
        c.begin();
        c.freeze();
        let saved = c.snapshot();
        c.first_count(900, 500);
        assert_eq!(c.snapshot(), saved);
        c.begin();
        c.first_count(900, 500);
        assert_eq!(c.snapshot(), saved);
    }
}
