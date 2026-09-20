//! Full-carrier-phase average for asynchronous DC-link samples.
//! At 201 us cadence, 50 scans visit every phase of the 20 kHz carrier once.
//! Comparing cross-product sums avoids division and rejects PWM-locked notches.
pub struct Guard<const N: u32> {
    bus_scaled: u32,
    reference_scaled: u32,
    count: u32,
    tripped: bool,
}

impl<const N: u32> Guard<N> {
    pub const fn new() -> Self {
        Self {
            bus_scaled: 0,
            reference_scaled: 0,
            count: 0,
            tripped: false,
        }
    }

    /// Returns true while operation may continue.
    pub fn observe(&mut self, bus: u16, vref: u16, vcal: u32) -> bool {
        if self.tripped {
            return false;
        }
        self.bus_scaled += bus as u32 * vcal;
        self.reference_scaled += 963u32 * vref as u32;
        self.count += 1;
        if self.count == N {
            self.tripped = self.bus_scaled < self.reference_scaled;
            self.bus_scaled = 0;
            self.reference_scaled = 0;
            self.count = 0;
        }
        !self.tripped
    }
    pub fn discard_partial(&mut self) {
        self.bus_scaled = 0;
        self.reference_scaled = 0;
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolated_notch_does_not_poison_phase_average() {
        let mut g = Guard::<50>::new();
        assert!(g.observe(0, 1500, 1662));
        for _ in 1..50 {
            assert!(g.observe(1100, 1500, 1662));
        }
    }

    #[test]
    fn sustained_low_trips_at_one_complete_phase_sweep() {
        let mut g = Guard::<50>::new();
        for _ in 1..50 {
            assert!(g.observe(800, 1500, 1662));
        }
        assert!(!g.observe(800, 1500, 1662));
        assert!(!g.observe(1100, 1500, 1662));
    }
}
