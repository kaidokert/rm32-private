//! Last eight late mismatch callbacks. No motor authority or per-read work.
pub struct Ring {
    pub total: u32,
    pub invalid: bool,
    pub rows: [[u32; 5]; 8],
}
impl Ring {
    pub const fn new() -> Self {
        Self {
            total: 0,
            invalid: false,
            rows: [[0; 5]; 8],
        }
    }
    pub fn reset(&mut self) {
        self.total = 0;
        self.invalid = false;
    }
    #[inline]
    pub fn push(&mut self, row: [u32; 5]) {
        if self.invalid {
            return;
        }
        let Some(next) = self.total.checked_add(1) else {
            self.invalid = true;
            return;
        };
        self.rows[(self.total & 7) as usize] = row;
        self.total = next;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_last_eight_and_reset_hides_old_rows() {
        let mut r = Ring::new();
        for i in 0..40 {
            r.push([i; 5]);
        }
        for i in 32..40 {
            assert_eq!(r.rows[(i & 7) as usize], [i; 5]);
        }
        assert_eq!(r.total, 40);
        r.reset();
        assert_eq!(r.total, 0);
        r.push([77; 5]);
        assert_eq!(r.rows[0], [77; 5]);
        assert_eq!(r.total, 1);
    }
    #[test]
    fn overflow_refuses_without_modifying_retained_rows() {
        let mut r = Ring::new();
        r.total = u32::MAX;
        r.push([1; 5]);
        assert!(r.invalid);
        assert_eq!(r.rows, [[0; 5]; 8]);
        r.push([2; 5]);
        assert_eq!(r.total, u32::MAX);
        r.reset();
        r.push([3; 5]);
        assert!(!r.invalid);
        assert_eq!(r.total, 1);
    }
}
