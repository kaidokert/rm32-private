//! Bench time-series recorder: a fixed ring of packed samples taken from the
//! main loop on the control-tick clock, frozen and dumped after a run.
//! Nothing is printed while the bridge drives (UART traffic couples into the
//! comparator and, before the `dprintln!` fix, masked every interrupt), so
//! ramp and step behaviour is recorded in RAM and read back afterwards.
//!
//! Portable and host-tested; the firmware owns the storage and the clock.

/// Most samples a single `ticks` call fills after a stall.
pub const MAX_FILL: u32 = 64;

/// One packed sample (8 bytes).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Applied duty (0..=2000) in bits 0..11, polling-mode flag in bit 11,
    /// low 4 bits of the cumulative desync count in bits 12..16.
    pub duty_flags: u16,
    /// Commutation interval (interval-timer counts per step; 0.5 us on G071).
    pub ci: u16,
    /// Bus current, mA (the firmware records its ~16 ms average).
    pub ma: i16,
    /// Bus voltage, mV.
    pub mv: u16,
}

impl Sample {
    pub fn pack(duty: u16, old_routine: bool, dsy: u32, ci: u32, ma: i32, mv: u16) -> Self {
        Self {
            duty_flags: (duty.min(2047))
                | (u16::from(old_routine) << 11)
                | (((dsy & 0xF) as u16) << 12),
            ci: ci.min(u16::MAX as u32) as u16,
            ma: ma.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            mv,
        }
    }
    pub fn duty(&self) -> u16 {
        self.duty_flags & 0x7FF
    }
    pub fn old_routine(&self) -> bool {
        self.duty_flags & (1 << 11) != 0
    }
    pub fn dsy4(&self) -> u8 {
        (self.duty_flags >> 12) as u8
    }
}

/// Ring of `N` samples. `tick` is fed every control tick; a sample is
/// stored every `period` ticks while armed. Oldest samples are overwritten.
pub struct Recorder<const N: usize> {
    buf: [Sample; N],
    head: usize,
    len: usize,
    period: u32,
    count: u32,
    armed: bool,
    /// Ticks the newest sample is behind the freeze (0 while recording).
    pub total: u32,
}

impl<const N: usize> Recorder<N> {
    pub const fn new() -> Self {
        Self {
            buf: [Sample {
                duty_flags: 0,
                ci: 0,
                ma: 0,
                mv: 0,
            }; N],
            head: 0,
            len: 0,
            // All-zero initial state: a `static` of this type lands in .bss,
            // not .data (16 KB of flash for the initial image). `arm` sets it.
            period: 0,
            count: 0,
            armed: false,
            total: 0,
        }
    }

    /// Clear and start recording one sample every `period` ticks.
    pub fn arm(&mut self, period: u32) {
        self.head = 0;
        self.len = 0;
        self.period = period.max(1);
        // Due at once: the first tick after arming stores a sample.
        self.count = self.period - 1;
        self.total = 0;
        self.armed = true;
    }

    /// Stop recording; the contents stay readable.
    pub fn freeze(&mut self) {
        self.armed = false;
    }

    pub fn armed(&self) -> bool {
        self.armed
    }
    pub fn period(&self) -> u32 {
        self.period
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// One control tick. `sample` is only evaluated when a sample is due.
    pub fn tick(&mut self, sample: impl FnOnce() -> Sample) {
        self.ticks(1, sample);
    }

    /// Advance by `n` ticks at once (the main loop may pass several ticks
    /// between calls). One sample is stored per `period` ticks elapsed (the
    /// first tick after arming samples at once); when the caller fell more
    /// than one period behind, the missed slots are filled with the current
    /// values (at most `MAX_FILL` per call) so the time axis stays uniform.
    pub fn ticks(&mut self, n: u32, sample: impl FnOnce() -> Sample) {
        if !self.armed || n == 0 {
            return;
        }
        self.total = self.total.wrapping_add(n);
        self.count = self.count.saturating_add(n);
        if self.count < self.period {
            return;
        }
        let due = (self.count / self.period).min(MAX_FILL);
        self.count %= self.period;
        let v = sample();
        for _ in 0..due {
            self.buf[self.head] = v;
            self.head = if self.head + 1 == N { 0 } else { self.head + 1 };
            if self.len < N {
                self.len += 1;
            }
        }
    }

    /// Sample `i` in time order (0 = oldest).
    pub fn get(&self, i: usize) -> Option<Sample> {
        if i >= self.len {
            return None;
        }
        let start = if self.len < N { 0 } else { self.head };
        Some(self.buf[(start + i) % N])
    }
}

impl<const N: usize> Default for Recorder<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: u16) -> Sample {
        Sample {
            duty_flags: v,
            ci: v,
            ma: v as i16,
            mv: v,
        }
    }

    #[test]
    fn records_every_period_ticks() {
        let mut r: Recorder<8> = Recorder::new();
        r.arm(3);
        for t in 0..9u16 {
            r.tick(|| s(t));
        }
        assert_eq!(r.len(), 3);
        assert_eq!(r.get(0).unwrap().ci, 0);
        assert_eq!(r.get(1).unwrap().ci, 3);
        assert_eq!(r.get(2).unwrap().ci, 6);
        assert_eq!(r.get(3), None);
    }

    #[test]
    fn wraps_keeping_newest_in_order() {
        let mut r: Recorder<4> = Recorder::new();
        r.arm(1);
        for t in 0..10u16 {
            r.tick(|| s(t));
        }
        assert_eq!(r.len(), 4);
        let got: [u16; 4] = core::array::from_fn(|i| r.get(i).unwrap().ci);
        assert_eq!(got, [6, 7, 8, 9]);
    }

    #[test]
    fn idle_until_armed_and_after_freeze() {
        let mut r: Recorder<4> = Recorder::new();
        r.tick(|| s(1));
        assert!(r.is_empty());
        r.arm(1);
        r.tick(|| s(2));
        r.freeze();
        r.tick(|| s(3));
        assert_eq!(r.len(), 1);
        assert_eq!(r.get(0).unwrap().ci, 2);
    }

    #[test]
    fn rearm_clears() {
        let mut r: Recorder<4> = Recorder::new();
        r.arm(1);
        for t in 0..6u16 {
            r.tick(|| s(t));
        }
        r.arm(2);
        assert!(r.is_empty());
        r.tick(|| s(9));
        assert_eq!(r.get(0).unwrap().ci, 9);
    }

    #[test]
    fn batched_ticks_take_one_sample_per_due_point() {
        let mut r: Recorder<16> = Recorder::new();
        r.arm(200);
        // main loop running ~1 tick per pass, with occasional 3-tick gaps
        let mut stored = 0;
        for k in 0..2000u32 {
            let n = if k % 50 == 0 { 3 } else { 1 };
            let before = r.len();
            r.ticks(n, || s(k as u16));
            if r.len() > before {
                stored += 1;
            }
        }
        let total: u32 = (0..2000u32).map(|k| if k % 50 == 0 { 3 } else { 1 }).sum();
        // one sample per 200-tick period, +1 for the immediate first sample
        assert_eq!(stored, (total / 200 + 1) as usize);
    }

    #[test]
    fn a_stall_fills_the_missed_slots() {
        let mut r: Recorder<64> = Recorder::new();
        r.arm(10);
        r.tick(|| s(1)); // immediate first sample
        r.ticks(10, || s(2)); // one full period -> sample
        r.ticks(35, || s(3)); // 3.5 periods late -> 3 copies, carry 5
        r.ticks(5, || s(4)); // carry reaches a period -> 1
        let got: [u16; 6] = core::array::from_fn(|i| r.get(i).map_or(0, |x| x.ci));
        assert_eq!(got, [1, 2, 3, 3, 3, 4]);
        assert_eq!(r.len(), 6);
    }

    #[test]
    fn pack_round_trips_flags_and_clamps() {
        let p = Sample::pack(2000, true, 0x23, 70_000, -40_000, 11_900);
        assert_eq!(p.duty(), 2000);
        assert!(p.old_routine());
        assert_eq!(p.dsy4(), 3);
        assert_eq!(p.ci, u16::MAX);
        assert_eq!(p.ma, i16::MIN);
        assert_eq!(p.mv, 11_900);
        let q = Sample::pack(5, false, 0, 100, 1234, 0);
        assert_eq!(
            (q.duty(), q.old_routine(), q.dsy4(), q.ci, q.ma),
            (5, false, 0, 100, 1234)
        );
    }
}
