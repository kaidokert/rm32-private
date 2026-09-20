//! Fixed-memory suffix of events omitted from the separate startup prefix.
//! Caller serializes push/reset and reads only after acquisition stops.
#[derive(Clone, Copy)]
pub struct Tail {
    rows: [[u16; 4]; 32],
    total: u32,
}
impl Tail {
    pub const fn new() -> Self {
        Self {
            rows: [[0; 4]; 32],
            total: 0,
        }
    }
    pub fn push(&mut self, row: [u16; 4]) {
        self.rows[(self.total % 32) as usize] = row;
        self.total += 1;
    }
    pub fn len(&self) -> usize {
        self.total.min(32) as usize
    }
    pub fn skipped(&self) -> u32 {
        self.total.saturating_sub(32)
    }
    pub fn row(&self, index: usize) -> [u16; 4] {
        assert!(index < self.len());
        self.rows[((self.skipped() + index as u32) % 32) as usize]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_partial_full_and_overwritten_are_chronological() {
        let mut t = Tail::new();
        assert_eq!(t.len(), 0);
        for n in 0..100u32 {
            t.push([n as u16, 0, 1, 1667]);
            assert_eq!(t.len(), (n + 1).min(32) as usize);
            assert_eq!(t.skipped(), (n + 1).saturating_sub(32));
            for i in 0..t.len() {
                assert_eq!(t.row(i)[0] as u32, t.skipped() + i as u32);
            }
        }
    }
    #[test]
    fn two_minutes_at_200ehz_keeps_full_width_timestamps() {
        let mut t = Tail::new();
        for n in 0..144000u32 {
            let us = n * 833;
            t.push([us as u16, (us >> 16) as u16, (n % 6 + 1) as u16, 1666]);
        }
        assert_eq!(t.skipped(), 143968);
        for i in 0..32 {
            let row = t.row(i);
            let n = 143968 + i as u32;
            assert_eq!(row[0] as u32 | ((row[1] as u32) << 16), n * 833);
            assert_eq!(row[2] as u32, n % 6 + 1);
        }
    }
}
