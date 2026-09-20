//! Bounded raw zero-offset statistics, not a calibration applied to guards.
#[derive(Clone, Copy)]
pub struct Stats {
    pub squares: u64,
    pub sum: u32,
    pub n: u16,
    pub min: u16,
    pub max: u16,
}
impl Stats {
    pub const fn new() -> Self {
        Self {
            squares: 0,
            sum: 0,
            n: 0,
            min: 4095,
            max: 0,
        }
    }
    pub fn push(&mut self, raw: u16) -> bool {
        if self.n == 512 || raw > 4095 {
            return false;
        }
        self.n += 1;
        self.sum += raw as u32;
        self.squares += (raw as u64) * (raw as u64);
        self.min = self.min.min(raw);
        self.max = self.max.max(raw);
        true
    }
    pub fn words(&self, ch: u16) -> [u16; 10] {
        [
            ch,
            self.n,
            self.sum as u16,
            (self.sum >> 16) as u16,
            self.squares as u16,
            (self.squares >> 16) as u16,
            (self.squares >> 32) as u16,
            (self.squares >> 48) as u16,
            self.min,
            self.max,
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extrema_count_and_wide_square_sum() {
        let mut s = Stats::new();
        for _ in 0..512 {
            assert!(s.push(4095));
        }
        assert_eq!(s.squares, 512 * 4095u64 * 4095);
        assert!(!s.push(0));
        let w = s.words(4);
        assert_eq!(w[1], 512);
        assert_eq!(w[8..], [4095, 4095]);
        assert_eq!(
            w[4] as u64 | ((w[5] as u64) << 16) | ((w[6] as u64) << 32) | ((w[7] as u64) << 48),
            s.squares
        );
    }
    #[test]
    fn invalid_input_is_not_silently_clipped() {
        let mut s = Stats::new();
        assert!(!s.push(4096));
        assert_eq!(s.n, 0);
        s.push(2047);
        s.push(2049);
        assert_eq!(s.sum, 4096);
        assert_eq!(s.squares, 2047u64 * 2047 + 2049u64 * 2049);
    }
}
