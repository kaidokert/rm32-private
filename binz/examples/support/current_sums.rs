//! Segment-local raw evidence, NOT calibrated current. Caller owns serialization.
//! Currents are signed relative to2048 only for storage; that is not a zero fit.
pub struct Sums {
    pub sums: [i64; 5],
    pub n: u32,
    pub first: u32,
    pub last: u32,
}
impl Sums {
    pub const fn new() -> Self {
        Self {
            sums: [0; 5],
            n: 0,
            first: 0,
            last: 0,
        }
    }
    pub fn clear(&mut self) {
        *self = Self::new();
    }
    pub fn push(&mut self, raw: [u16; 5], at: u32) -> bool {
        self.push_period::<101>(raw, at)
    }
    pub fn push_period<const PERIOD: u32>(&mut self, raw: [u16; 5], at: u32) -> bool {
        if !matches!(PERIOD, 101 | 201 | 209 | 226)
            || self.n >= 6_000_000
            || raw.iter().any(|&v| v > 4095)
            || (self.n != 0 && at.wrapping_sub(self.last) != PERIOD)
        {
            return false;
        }
        for i in 0..5 {
            self.sums[i] += raw[i] as i64 - if i < 3 { 2048 } else { 0 };
        }
        if self.n == 0 {
            self.first = at;
        }
        self.last = at;
        self.n += 1;
        true
    }
    pub fn words(&self, ch: usize) -> [u16; 11] {
        let sum = self.sums[ch] as u64;
        [
            ch as u16,
            self.n as u16,
            (self.n >> 16) as u16,
            self.first as u16,
            (self.first >> 16) as u16,
            self.last as u16,
            (self.last >> 16) as u16,
            sum as u16,
            (sum >> 16) as u16,
            (sum >> 32) as u16,
            (sum >> 48) as u16,
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_slower_cadence_preserves_order_and_wrap() {
        let mut s = Sums::new();
        assert!(s.push_period::<201>([2048; 5], u32::MAX - 100));
        assert!(!s.push_period::<201>([2048; 5], 0));
        assert!(s.push_period::<201>([2048; 5], 100));
        assert!(!s.push_period::<201>([2048; 5], 502));
        assert!(!s.push_period::<200>([2048; 5], 300));
        assert_eq!(s.n, 2);
        assert_eq!(s.last, 100);

        let mut phase_walk = Sums::new();
        assert!(phase_walk.push_period::<209>([2048; 5], u32::MAX - 108));
        assert!(phase_walk.push_period::<209>([2048; 5], 100));
        assert!(!phase_walk.push_period::<209>([2048; 5], 308));
        assert_eq!(phase_walk.n, 2);
        assert_eq!(phase_walk.last, 100);

        let mut distributed = Sums::new();
        assert!(distributed.push_period::<226>([2048; 5], u32::MAX - 125));
        assert!(distributed.push_period::<226>([2048; 5], 100));
        assert!(!distributed.push_period::<226>([2048; 5], 325));
    }
    #[test]
    fn signed_sum_wire_and_clock_wrap() {
        let mut s = Sums::new();
        assert!(s.push([2047, 2049, 2048, 1200, 1500], u32::MAX - 50));
        assert!(s.push([2047, 2049, 2048, 1200, 1500], 50));
        assert_eq!(s.sums, [-2, 2, 0, 2400, 3000]);
        assert_eq!(s.words(0)[7..], [65534, 65535, 65535, 65535]);
        s.clear();
        assert_eq!(s.n, 0);
        assert_eq!(s.sums, [0; 5]);
    }
    #[test]
    fn invalid_gap_duplicate_and_capacity_refuse_without_mutation() {
        let mut s = Sums::new();
        assert!(s.push([2048; 5], 101));
        for at in [101, 201, 303] {
            assert!(!s.push([2048; 5], at));
        }
        assert!(!s.push([4096; 5], 202));
        assert_eq!(s.n, 1);
        assert_eq!(s.last, 101);
        s.n = 6_000_000;
        assert!(!s.push([2048; 5], 202));
    }
    #[test]
    fn six_hundred_seconds_does_not_overflow_32bit_or_signed_sums() {
        let mut s = Sums::new();
        for n in 1..=6_000_000 {
            assert!(s.push([4095, 0, 2048, 4095, 4095], n * 101));
        }
        assert_eq!(
            s.sums,
            [
                12_282_000_000,
                -12_288_000_000,
                0,
                24_570_000_000,
                24_570_000_000
            ]
        );
        assert_eq!(core::mem::size_of::<Sums>(), 56);
    }
}
