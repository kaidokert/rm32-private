//! Current smoothing (moving average filter) and consumed charge.

use crate::units::CentiAmps;

/// A main-loop-owned integral of current over control ticks. Fractional mAh
/// remain in the accumulator; only telemetry truncates to whole mAh.
pub(crate) struct ConsumedCharge<P: crate::platform::Platform> {
    _platform: core::marker::PhantomData<P>,
    centiamp_ticks: u64,
}

impl<P: crate::platform::Platform> ConsumedCharge<P> {
    // 1 mAh = 3600 mA*s = 360 cA*s.
    const TICKS_PER_MAH: u64 = 360 * P::TICK_HZ as u64;
    const MAX: u64 = u16::MAX as u64 * Self::TICKS_PER_MAH;

    /// Integrate the last observed current over the entire elapsed interval.
    /// Negative readings do not subtract consumed charge. Once the KISS u16
    /// range is full, latch at 65535 mAh until the state is reinitialized.
    pub(crate) fn accumulate(&mut self, current: CentiAmps, elapsed_ticks: u32) {
        if elapsed_ticks == 0 || current.0 <= 0 || self.centiamp_ticks == Self::MAX {
            return;
        }
        // u32 ticks * positive i16 centiamps plus MAX fits comfortably in u64.
        // No division in this per-service path; the state is main-only.
        let increment = current.0 as u64 * elapsed_ticks as u64;
        self.centiamp_ticks = (self.centiamp_ticks + increment).min(Self::MAX);
    }

    pub(crate) fn milliamp_hours(&self) -> u16 {
        (self.centiamp_ticks / Self::TICKS_PER_MAH) as u16
    }
}

impl<P: crate::platform::Platform> Default for ConsumedCharge<P> {
    fn default() -> Self {
        Self {
            centiamp_ticks: 0,
            _platform: core::marker::PhantomData,
        }
    }
}
impl<P: crate::platform::Platform> Clone for ConsumedCharge<P> {
    fn clone(&self) -> Self {
        Self {
            centiamp_ticks: self.centiamp_ticks,
            _platform: core::marker::PhantomData,
        }
    }
}

const NUM_READINGS: usize = 50;

#[derive(Clone)]
pub(crate) struct CurrentFilter {
    readings: [u16; NUM_READINGS],
    index: usize,
    total: u32,
}

impl CurrentFilter {
    pub(crate) const fn new() -> Self {
        Self {
            readings: [0; NUM_READINGS],
            index: 0,
            total: 0,
        }
    }

    /// Feed a new raw ADC current reading, returns smoothed value.
    pub(crate) fn update(&mut self, raw: u16) -> u16 {
        self.total -= self.readings[self.index] as u32;
        self.readings[self.index] = raw;
        self.total += raw as u32;
        self.index += 1;
        if self.index >= NUM_READINGS {
            self.index = 0;
        }
        (self.total / NUM_READINGS as u32) as u16
    }
}

impl Default for CurrentFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_running_average() {
        let mut f = CurrentFilter::new();
        for _ in 0..50 {
            f.update(1000);
        }
        assert_eq!(f.update(1000), 1000);
    }

    #[test]
    fn wraps_index() {
        let mut f = CurrentFilter::new();
        for _ in 0..60 {
            f.update(500);
        }
        assert_eq!(f.index, 10); // 60 % 50
    }

    #[test]
    fn starts_at_zero() {
        let f = CurrentFilter::new();
        assert_eq!(f.total, 0);
        assert_eq!(f.index, 0);
    }

    #[test]
    fn averages_mixed_values() {
        let mut f = CurrentFilter::new();
        // Fill half with 0, half with 1000
        for _ in 0..25 {
            f.update(0);
        }
        for _ in 0..25 {
            f.update(1000);
        }
        assert_eq!(f.update(1000), 520); // (25*0 + 26*1000) / 50 = 520
    }
}
