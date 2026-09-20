//! Six fixed time bins of accepted-event count and inter-event gap range.
//! Not independent windows, qZC coverage, or proof of rotor lock.
//! Caller serializes push with stop; timestamps are segment-relative microseconds.
pub const BINS: usize = 6;
// Base4096 long division. Each intermediate is <=5*4096+4095=24575.
//10923/65536 =1/6+1/196608: error<1/8, while a /6 fraction is<=5/6.
// Thus the shifted32-bit product gives the exact quotient without UMULL.
const fn div6_digit(x: u32) -> u32 {
    (x * 10923) >> 16
}
#[inline(always)]
const fn div6(x: u32) -> u32 {
    let high = x >> 24;
    let q0 = div6_digit(high);
    let middle = ((high - q0 * 6) << 12) | ((x >> 12) & 4095);
    let q1 = div6_digit(middle);
    let low = ((middle - q1 * 6) << 12) | (x & 4095);
    (q0 << 24) | (q1 << 12) | div6_digit(low)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bin {
    pub events: u32,
    /// 65535 means no gap yet for min, or saturated >=65535us for max.
    pub min_gap: u16,
    pub max_gap: u16,
}
impl Bin {
    const EMPTY: Self = Self {
        events: 0,
        min_gap: u16::MAX,
        max_gap: 0,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timeline {
    pub bins: [Bin; BINS],
    pub window_us: u32,
    pub bin_us: u32,
    last: u32,
    /// Events at/beyond planned end remain visible, never silently discarded.
    pub overrun_events: u32,
}
impl Timeline {
    #[inline(always)]
    pub const fn new(window_us: u32) -> Option<Self> {
        if window_us < 20_000 || window_us > 600_000_000 {
            return None;
        }
        Some(Self {
            bins: [Bin::EMPTY; BINS],
            window_us,
            bin_us: div6(window_us + 5),
            last: u32::MAX,
            overrun_events: 0,
        })
    }
    pub fn push(&mut self, us: u32) -> bool {
        // Segment clocks cannot wrap in the supported <=600s window.
        if self.last != u32::MAX && us < self.last {
            return false;
        }
        // Six bins only. new() bounds bin_us<=100_000_000, so all five
        // boundaries fit u32. Preserve exact floor/clamp without a soft
        // division inside the caller's interrupt-masked recorder update.
        let b = self.bin_us;
        let index = if us >= 3 * b {
            if us >= 4 * b {
                if us >= 5 * b { 5 } else { 4 }
            } else {
                3
            }
        } else if us >= b {
            if us >= 2 * b { 2 } else { 1 }
        } else {
            0
        };
        let b = &mut self.bins[index];
        b.events += 1;
        if self.last != u32::MAX {
            let gap = us - self.last;
            let clipped = gap.min(u16::MAX as u32) as u16;
            b.min_gap = b.min_gap.min(clipped);
            b.max_gap = b.max_gap.max(clipped);
        }
        if us >= self.window_us {
            self.overrun_events += 1;
        }
        self.last = us;
        true
    }
    pub fn total(&self) -> u32 {
        self.bins.iter().map(|b| b.events).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_division_digits_and_composition_boundaries() {
        // Exhaust the complete intermediate domain: this proves each digit.
        for x in 0..=24575 {
            assert_eq!(div6_digit(x), x / 6);
        }
        // Every12-bit boundary in the supported constructor range, neighbors,
        // all residue classes, plus full-u32 endpoints for the helper itself.
        for block in 0..=(600_000_005u32 >> 12) {
            for offset in 0..=6 {
                let x = block * 4096 + offset;
                assert_eq!(div6(x), x / 6);
                let y = x.saturating_sub(7);
                assert_eq!(div6(y), y / 6);
            }
        }
        for x in [0, 1, 5, 6, 7, u32::MAX - 1, u32::MAX] {
            assert_eq!(div6(x), x / 6);
        }
        const FIXED: Option<Timeline> = Timeline::new(30_000_001);
        assert_eq!(FIXED.unwrap().bin_us, 5_000_001);
        assert!(Timeline::new(19_999).is_none());
        assert!(Timeline::new(600_000_001).is_none());
    }
    #[test]
    fn size_is_bounded_and_count_does_not_truncate_at_u16() {
        assert_eq!(core::mem::size_of::<Timeline>(), 64);
        let mut t = Timeline::new(600_000_000).unwrap();
        for i in 0..900_000 {
            assert!(t.push(i * 666));
        }
        assert_eq!(t.total(), 900_000);
        assert!(
            t.bins
                .iter()
                .all(|b| b.events > 65535 && b.min_gap == 666 && b.max_gap == 666)
        );
        assert_eq!(t.overrun_events, 0);
    }
    #[test]
    fn bounded_selection_matches_division_and_preserves_overruns() {
        for window in [20_000, 20_001, 30_000, 1_000_000, 27_990_791, 600_000_000] {
            let bin = Timeline::new(window).unwrap().bin_us;
            // Each threshold and both neighbors, plus late/u32-limit events.
            for boundary in 0..=6 {
                for us in [
                    (boundary * bin).saturating_sub(1),
                    boundary * bin,
                    boundary * bin + 1,
                    u32::MAX,
                ] {
                    let mut t = Timeline::new(window).unwrap();
                    assert!(t.push(us));
                    let expected = (us / bin).min(5) as usize;
                    for i in 0..6 {
                        assert_eq!(t.bins[i].events, (i == expected) as u32);
                    }
                    assert_eq!(t.overrun_events, (us >= window) as u32);
                }
            }
        }
        // Exhaust all timestamps over several complete small-window layouts.
        for window in [20_000, 20_001, 30_000] {
            let mut t = Timeline::new(window).unwrap();
            let mut counts = [0u32; 6];
            for us in 0..=window + 10_000 {
                assert!(t.push(us));
                counts[(us / t.bin_us).min(5) as usize] += 1;
            }
            for i in 0..6 {
                assert_eq!(t.bins[i].events, counts[i]);
            }
        }
    }
    #[test]
    fn boundary_gap_belongs_to_arriving_event_bin() {
        let mut t = Timeline::new(30_000).unwrap();
        for us in [0, 4999, 5000, 15000, 29999, 30000] {
            assert!(t.push(us));
        }
        assert_eq!(
            t.bins[0],
            Bin {
                events: 2,
                min_gap: 4999,
                max_gap: 4999
            }
        );
        assert_eq!(
            t.bins[1],
            Bin {
                events: 1,
                min_gap: 1,
                max_gap: 1
            }
        );
        assert_eq!(t.bins[2], Bin::EMPTY);
        assert_eq!(t.bins[3].max_gap, 10000);
        assert_eq!(t.overrun_events, 1);
        assert_eq!(t.total(), 6);
    }
    #[test]
    fn reversal_refused_and_large_gaps_explicitly_saturate() {
        let mut t = Timeline::new(1_000_000).unwrap();
        assert!(t.push(100));
        let before = t;
        assert!(!t.push(99));
        assert_eq!(t, before);
        assert!(t.push(200_000));
        assert_eq!(t.bins[1].max_gap, u16::MAX);
    }
    #[test]
    fn owned_snapshot_survives_reset_and_partial_final_bin() {
        let mut t = Timeline::new(20_001).unwrap();
        assert_eq!(t.bin_us, 3334);
        t.push(20000);
        let saved = t;
        t = Timeline::new(30_000).unwrap();
        assert_eq!(saved.total(), 1);
        assert_eq!(saved.overrun_events, 0);
        assert_eq!(t.total(), 0);
        assert_eq!(saved.bins[5].events, 1);
        assert!(Timeline::new(19999).is_none());
        assert!(Timeline::new(600_000_001).is_none());
    }
}
