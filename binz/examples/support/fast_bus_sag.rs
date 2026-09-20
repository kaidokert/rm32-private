//! Fast, relative bus-collapse stop. Complete DMA scans only; the slow
//! carrier-spanning average remains an independent absolute-voltage backstop.
pub struct Guard {
    reference_bus: u16,
    reference_vref: u16,
    lows: u8,
    tripped: bool,
}

impl Guard {
    pub fn low_streak(&self) -> u8 {
        self.lows
    }
    pub fn new(reference_bus: u16, reference_vref: u16) -> Option<Self> {
        if reference_bus == 0
            || reference_bus >= 4095
            || reference_vref == 0
            || reference_vref >= 4095
        {
            return None;
        }
        Some(Self {
            reference_bus,
            reference_vref,
            lows: 0,
            tripped: false,
        })
    }

    /// VBUS is proportional to bus/VREF. Products fit u32:
    /// 4095 * 4095 * 100 = 1_676_902_500. Three consecutive >5% dips trip
    /// within three DMA periods, ignoring isolated PWM notches/ADC outliers.
    /// Equality at 95% is allowed.
    pub fn observe(&mut self, bus: u16, vref: u16) -> bool {
        if self.tripped {
            return false;
        }
        // A single zero bus code could be an ADC glitch; it must pass the
        // same persistence rule. VREF invalidity is separately fail-closed.
        if vref == 0 || vref >= 4095 {
            self.tripped = true;
            return false;
        }
        let low = bus as u32 * self.reference_vref as u32 * 100
            < self.reference_bus as u32 * vref as u32 * 95;
        self.lows = if low { self.lows + 1 } else { 0 };
        self.tripped = self.lows >= 3;
        !self.tripped
    }
}

#[cfg(test)]
mod tests {
    use super::Guard;

    #[test]
    fn one_or_two_notches_are_ignored_but_three_consecutive_dips_trip() {
        let mut g = Guard::new(1200, 1500).unwrap();
        assert!(g.observe(1000, 1500));
        assert!(g.observe(1200, 1500));
        assert!(g.observe(1139, 1500));
        assert!(g.observe(1139, 1500));
        assert!(!g.observe(1139, 1500));
        assert!(!g.observe(1200, 1500));
    }

    #[test]
    fn exact_five_percent_and_vref_tracking_are_not_trips() {
        let mut g = Guard::new(1200, 1500).unwrap();
        assert!(g.observe(1140, 1500));
        assert!(g.observe(2280, 3000));
        assert!(g.observe(1200, 1500));
    }

    #[test]
    fn invalid_adc_is_fail_closed() {
        assert!(Guard::new(0, 1500).is_none());
        assert!(Guard::new(1200, 0).is_none());
        let mut g = Guard::new(1200, 1500).unwrap();
        assert!(!g.observe(1200, 0));
        let mut g = Guard::new(1200, 1500).unwrap();
        assert!(g.observe(0, 1500));
        assert!(g.observe(1200, 1500));
        assert!(g.observe(0, 1500));
    }

    #[test]
    fn captured_50_percent_trip_is_raw_coherent_dip_not_integer_wrap() {
        // Reverse48k advance26 IT86/BS85 fault B, 2026-09-19. The guard
        // decides from the DMA codes below, with no time/event counter.
        let mut g = Guard::new(1212, 1506).unwrap();
        for (bus, vref) in [(1185, 1509), (1192, 1504), (1174, 1508)] {
            assert!(g.observe(bus, vref));
            assert_eq!(g.low_streak(), 0);
        }
        for (index, (bus, vref)) in [(1112, 1505), (1142, 1504), (1103, 1516)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(g.observe(bus, vref), index < 2);
            assert_eq!(g.low_streak(), index as u8 + 1);
        }
        assert!(4095u32 * 4095 * 100 < i32::MAX as u32);
    }
}
