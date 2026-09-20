//! Whole-stream integer timing moments. Evidence, not a rotor-lock classifier.
//! No division, floating point, formatting or unbounded storage on event path.
#[derive(Clone, Copy)]
pub struct Moments {
    pub n: u32,
    pub min: u32,
    pub max: u32,
    pub sum: u64,
    pub squares: u64,
    pub excluded: u32,
}
impl Moments {
    const fn new() -> Self {
        Self {
            n: 0,
            min: u32::MAX,
            max: 0,
            sum: 0,
            squares: 0,
            excluded: 0,
        }
    }
    fn push(&mut self, us: u32) {
        // Limit multiply to32-bit on M0; long/invalid gaps remain explicit.
        if us > 65535 {
            self.excluded += 1;
            return;
        }
        self.n += 1;
        self.min = self.min.min(us);
        self.max = self.max.max(us);
        self.sum += us as u64;
        self.squares += (us * us) as u64;
    }
}
#[derive(Clone, Copy)]
pub struct Stats {
    pub events: u32,
    pub first: u32,
    pub last: u32,
    pub order_bad: u32,
    last_step: u8,
    at: [u32; 6],
    seen: u8,
    pub gaps: Moments,
    pub cycles: Moments,
}
impl Stats {
    pub const fn new() -> Self {
        Self {
            events: 0,
            first: 0,
            last: 0,
            order_bad: 0,
            last_step: 0,
            at: [0; 6],
            seen: 0,
            gaps: Moments::new(),
            cycles: Moments::new(),
        }
    }
    pub fn push(&mut self, us: u32, step: u8) {
        if !(1..=6).contains(&step) {
            self.order_bad += 1;
            return;
        }
        if self.events == 0 {
            self.first = us;
        } else {
            if step != self.last_step % 6 + 1 {
                self.order_bad += 1;
            }
            self.gaps.push(us.wrapping_sub(self.last));
        }
        let i = (step - 1) as usize;
        let bit = 1 << i;
        if self.seen & bit != 0 {
            self.cycles.push(us.wrapping_sub(self.at[i]));
        }
        self.at[i] = us;
        self.seen |= bit;
        self.events += 1;
        self.last = us;
        self.last_step = step;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sustained_stream_moments_do_not_overflow_u32() {
        let mut s = Stats::new();
        for n in 0..150000u32 {
            s.push(n * 800, (n % 6 + 1) as u8);
        }
        assert_eq!(s.events, 150000);
        assert_eq!(s.order_bad, 0);
        assert_eq!(s.gaps.n, 149999);
        assert_eq!(s.gaps.sum, 149999 * 800);
        assert_eq!(s.gaps.squares, 149999 * 640000u64);
        assert_eq!(s.cycles.n, 149994);
        assert_eq!(s.cycles.min, 4800);
        assert_eq!(s.cycles.max, 4800);
        assert_eq!(s.cycles.squares, 149994 * 23040000u64);
    }
    #[test]
    fn wrap_order_errors_and_long_gaps_are_visible() {
        let mut s = Stats::new();
        s.push(u32::MAX - 99, 1);
        s.push(700, 2);
        assert_eq!(s.gaps.min, 800);
        s.push(701, 4);
        assert_eq!(s.order_bad, 1);
        s.push(100000, 5);
        assert_eq!(s.gaps.excluded, 1);
    }
}
