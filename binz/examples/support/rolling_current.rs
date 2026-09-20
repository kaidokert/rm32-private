//! Division-free rolling signed-current warning. This never owns a stop.
//! The caller retains the original complete-block terminal current policy.
pub struct Rolling<const N: usize> {
    zero: i32,
    allowance: i32,
    sum: i32,
    samples: [u16; N],
    head: usize,
    filled: usize,
}

impl<const N: usize> Rolling<N> {
    pub const fn new(zero: i32, allowance: i32) -> Option<Self> {
        if N == 0 || zero < 0 || allowance <= 0 {
            return None;
        }
        Some(Self {
            zero,
            allowance,
            sum: 0,
            samples: [0; N],
            head: 0,
            filled: 0,
        })
    }

    /// Returns only over-limit residuals after one complete fresh window.
    pub fn scan(&mut self, raw: [u16; 3]) -> Option<u32> {
        let sample = raw[0] as u32 + raw[1] as u32 + raw[2] as u32;
        if sample > u16::MAX as u32 {
            return None;
        }
        if self.filled < N {
            self.samples[self.filled] = sample as u16;
            self.sum += sample as i32;
            self.filled += 1;
            if self.filled != N {
                return None;
            }
            self.head = 0;
        } else {
            let old = self.samples[self.head];
            self.samples[self.head] = sample as u16;
            self.sum += sample as i32 - old as i32;
            self.head += 1;
            if self.head == N {
                self.head = 0;
            }
        }
        let residual = self.zero - self.sum;
        (residual > self.allowance).then_some(residual as u32)
    }

    /// A changed duty invalidates every mixed old/new-duty sample.
    pub fn applied_reduction(&mut self) {
        self.sum = 0;
        self.samples = [0; N];
        self.head = 0;
        self.filled = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_window_then_one_sample_updates_without_division() {
        let mut r = Rolling::<4>::new(24_576, 4).unwrap();
        for _ in 0..3 {
            assert_eq!(r.scan([2048; 3]), None);
        }
        assert_eq!(r.scan([2048; 3]), None);
        assert_eq!(r.scan([2040, 2048, 2048]), Some(8));
        assert_eq!(r.scan([2048; 3]), Some(8));
        assert_eq!(r.scan([2048; 3]), Some(8));
        assert_eq!(r.scan([2048; 3]), Some(8));
        assert_eq!(r.scan([2048; 3]), None);
    }

    #[test]
    fn applied_reduction_requires_an_entire_fresh_window() {
        let mut r = Rolling::<4>::new(24_576, 1).unwrap();
        for _ in 0..4 {
            let _ = r.scan([2047, 2048, 2048]);
        }
        assert_eq!(r.scan([2047, 2048, 2048]), Some(4));
        r.applied_reduction();
        for _ in 0..3 {
            assert_eq!(r.scan([2047, 2048, 2048]), None);
        }
        assert_eq!(r.scan([2047, 2048, 2048]), Some(4));
    }

    #[test]
    fn invalid_configuration_and_sample_are_refused() {
        assert!(Rolling::<0>::new(1, 1).is_none());
        assert!(Rolling::<4>::new(-1, 1).is_none());
        assert!(Rolling::<4>::new(1, 0).is_none());
        let mut r = Rolling::<4>::new(1, 1).unwrap();
        assert_eq!(r.scan([u16::MAX; 3]), None);
    }
}
