//! Event-rate bookkeeping: wrapping count-delta windows and the
//! ticks→per-second conversion. Three inline copies of this pattern
//! existed in the firmware (1 s boundary, `o`/`p` rate resets, the
//! `i`-key irq/s block); wrapping deltas are a standing hazard class
//! here (see the watchdog timestamp-race incident).

/// A wrapping counter window: `latch` returns the delta since the
/// last latch (or construction) and restarts the window.
pub struct RateWindow {
    start: u32,
}

impl RateWindow {
    pub const fn new(start: u32) -> Self {
        Self { start }
    }

    pub fn latch(&mut self, now_count: u32) -> u32 {
        let d = now_count.wrapping_sub(self.start);
        self.start = now_count;
        d
    }
}

/// Events/second from a count delta over a 10 µs-tick delta.
/// `None` when no time elapsed (back-to-back reads inside one
/// microloop — the divide-by-zero guard).
pub fn rate_per_s(delta_count: u32, delta_ticks_10us: u32) -> Option<u32> {
    if delta_ticks_10us == 0 {
        None
    } else {
        Some((delta_count as u64 * 100_000 / delta_ticks_10us as u64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_returns_delta_and_restarts() {
        let mut w = RateWindow::new(100);
        assert_eq!(w.latch(150), 50);
        assert_eq!(w.latch(150), 0);
        assert_eq!(w.latch(400), 250);
    }

    #[test]
    fn latch_survives_u32_wrap() {
        let mut w = RateWindow::new(u32::MAX - 5);
        assert_eq!(w.latch(4), 10);
    }

    #[test]
    fn rate_math_and_zero_dt_guard() {
        // 20 000 events over 1 s (100 000 ticks) = 20 kHz.
        assert_eq!(rate_per_s(20_000, 100_000), Some(20_000));
        // Half-second window doubles the rate.
        assert_eq!(rate_per_s(20_000, 50_000), Some(40_000));
        assert_eq!(rate_per_s(123, 0), None);
        // Large counts don't overflow (u64 intermediate).
        assert_eq!(rate_per_s(u32::MAX, 100_000), Some(u32::MAX));
    }
}
