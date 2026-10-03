//! Explicit timer geometry, not motor authority or a DRV pulse guarantee.
//! Startup stays at10kHz. BEMF candidates include ARR2665 (24006Hz) and
//! ARR1999 (32000Hz), ARR1599 (40000Hz), ARR1332 (48012Hz) at64MHz.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Carrier {
    Khz10,
    Khz20,
    Khz24,
    Khz32,
    Khz40,
    Khz48,
}
// Derive exact bounded reciprocals in const evaluation, never in an ADC ISR.
// n=phase*factor. Error < one divisor remainder unit prevents rounding up;
// the second assertion proves the combined phase*multiplier fits u32.
const fn bin_multiplier(divisor: u32, shift: u32, factor: u32, max_phase: u32) -> u32 {
    let scale = 1u32 << shift;
    let reciprocal = (scale + divisor - 1) / divisor;
    assert!(max_phase * factor * (reciprocal * divisor - scale) < scale);
    assert!(max_phase <= u32::MAX / (factor * reciprocal));
    factor * reciprocal
}
const BIN10_MULT: u32 = bin_multiplier(200, 20, 1, 6399);
const BIN20_MULT: u32 = bin_multiplier(100, 20, 1, 3199);
// phase*32/2666 == phase*16/1333; reduced fraction permits a u32 product.
const BIN24_MULT: u32 = bin_multiplier(1333, 26, 16, 2665);
// phase*32/2000 == phase*2/125; no divide or wide multiply on M0.
const BIN32_MULT: u32 = bin_multiplier(125, 20, 2, 1999);
// phase*32/1600 == phase/50; bounded reciprocal is exact for every tick.
const BIN40_MULT: u32 = bin_multiplier(50, 20, 1, 1599);
// 32/1333 stays 32-bit bounded; the const proof covers all phase ticks.
const BIN48_MULT: u32 = bin_multiplier(1333, 26, 32, 1332);
impl Carrier {
    pub const fn ticks(self) -> u32 {
        match self {
            Self::Khz10 => 6400,
            Self::Khz20 => 3200,
            Self::Khz24 => 2666,
            Self::Khz32 => 2000,
            Self::Khz40 => 1600,
            Self::Khz48 => 1333,
        }
    }
    pub const fn arr(self) -> u32 {
        self.ticks() - 1
    }
    pub const fn hz_floor(self) -> u32 {
        64_000_000 / self.ticks()
    }
    pub const fn from_ticks(ticks: u32) -> Option<Self> {
        match ticks {
            6400 => Some(Self::Khz10),
            3200 => Some(Self::Khz20),
            2666 => Some(Self::Khz24),
            2000 => Some(Self::Khz32),
            1600 => Some(Self::Khz40),
            1333 => Some(Self::Khz48),
            _ => None,
        }
    }
    /// Explicit rejection, never silently clamp a requested duty.
    pub const fn compare(self, duty: u32) -> Option<u32> {
        if duty == 0 || duty > 100 {
            None
        } else {
            Some(self.ticks() * duty / 1000)
        }
    }
    /// Ideal MCU PWM high duration after26timer ticks deadtime, NOT measured
    /// gate/MOSFET conduction and not a guaranteed driver minimum pulse.
    pub const fn ideal_high_ticks(self, duty: u32) -> Option<u32> {
        match self.compare(duty) {
            Some(c) if c > 26 => Some(c - 26),
            _ => None,
        }
    }
    #[inline(always)]
    pub const fn phase_bin(self, phase: u32) -> Option<usize> {
        if phase >= self.ticks() {
            None
        } else {
            Some(match self {
                Self::Khz10 => (phase * BIN10_MULT) >> 20,
                Self::Khz20 => (phase * BIN20_MULT) >> 20,
                Self::Khz24 => (phase * BIN24_MULT) >> 26,
                Self::Khz32 => (phase * BIN32_MULT) >> 20,
                Self::Khz40 => (phase * BIN40_MULT) >> 20,
                Self::Khz48 => (phase * BIN48_MULT) >> 26,
            } as usize)
        }
    }
    /// Idle rolecheck discriminator. Quantization/read boundaries allow192
    /// timer ticks. Reject long brackets so a hidden whole wrap cannot pass.
    pub const fn continuous(self, elapsed_us: u32, delta: u32) -> bool {
        if elapsed_us >= self.ticks() / 64 - 3 || delta >= self.ticks() {
            return false;
        }
        let error = (delta + self.ticks() - (elapsed_us * 64) % self.ticks()) % self.ticks();
        error <= 192 || error >= self.ticks() - 192
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_timer_geometry_and_no_silent_frequency_rounding() {
        assert_eq!(
            (Carrier::Khz10.arr(), Carrier::Khz10.hz_floor()),
            (6399, 10000)
        );
        assert_eq!(
            (Carrier::Khz20.arr(), Carrier::Khz20.hz_floor()),
            (3199, 20000)
        );
        assert_eq!(
            (Carrier::Khz24.arr(), Carrier::Khz24.hz_floor()),
            (2665, 24006)
        );
        assert_eq!(
            (Carrier::Khz32.arr(), Carrier::Khz32.hz_floor()),
            (1999, 32000)
        );
        assert_eq!(
            (Carrier::Khz40.arr(), Carrier::Khz40.hz_floor()),
            (1599, 40000)
        );
        assert_eq!(
            (Carrier::Khz48.arr(), Carrier::Khz48.hz_floor()),
            (1332, 48012)
        );
        for bad in [0, 2665, 2667, 6399, 6401, u32::MAX] {
            assert_eq!(Carrier::from_ticks(bad), None);
        }
    }
    #[test]
    fn compare_scales_at_both_carriers_and_rejects_invalid_duty() {
        for carrier in [
            Carrier::Khz10,
            Carrier::Khz20,
            Carrier::Khz24,
            Carrier::Khz32,
            Carrier::Khz40,
            Carrier::Khz48,
        ] {
            for bad in [0, 101, u32::MAX] {
                assert_eq!(carrier.compare(bad), None);
            }
            for duty in 1..=100 {
                let c = carrier.compare(duty).unwrap();
                assert!(c * 1000 <= carrier.ticks() * duty);
                assert!((c + 1) * 1000 > carrier.ticks() * duty);
            }
        }
        assert_eq!(Carrier::Khz10.compare(54), Some(345));
        assert_eq!(Carrier::Khz24.compare(54), Some(143));
        assert_eq!(Carrier::Khz32.compare(100), Some(200));
        assert_eq!(Carrier::Khz32.ideal_high_ticks(100), Some(174));
        assert_eq!(Carrier::Khz40.compare(100), Some(160));
        assert_eq!(Carrier::Khz40.ideal_high_ticks(100), Some(134));
        assert_eq!(Carrier::Khz48.compare(100), Some(133));
        assert_eq!(Carrier::Khz48.ideal_high_ticks(100), Some(107));
        assert_eq!(Carrier::Khz24.ideal_high_ticks(40), Some(80));
        assert_eq!(Carrier::Khz24.ideal_high_ticks(60), Some(133));
        assert_eq!(Carrier::Khz24.ideal_high_ticks(62), Some(139));
    }
    #[test]
    fn all_phase_ticks_map_to_valid_balanced_bins() {
        for carrier in [
            Carrier::Khz10,
            Carrier::Khz20,
            Carrier::Khz24,
            Carrier::Khz32,
            Carrier::Khz40,
            Carrier::Khz48,
        ] {
            let mut bins = [0u32; 32];
            for phase in 0..carrier.ticks() {
                let bin = carrier.phase_bin(phase).unwrap();
                assert_eq!(bin, (phase * 32 / carrier.ticks()) as usize);
                bins[bin] += 1;
            }
            assert!(bins.iter().max().unwrap() - bins.iter().min().unwrap() <= 1);
            assert_eq!(carrier.phase_bin(carrier.ticks()), None);
            assert_eq!(carrier.phase_bin(u32::MAX), None);
        }
        // The memo's smaller reciprocal is not exact at this boundary.
        assert_eq!(Carrier::Khz10.phase_bin(1199), Some(5));
        assert_eq!((1199 * 41) >> 13, 6);
        assert_eq!((BIN10_MULT, BIN24_MULT), (5243, 805520));
    }
    #[test]
    fn bounded_counter_discriminator_accepts_reads_not_hidden_wraps() {
        for carrier in [
            Carrier::Khz10,
            Carrier::Khz20,
            Carrier::Khz24,
            Carrier::Khz32,
            Carrier::Khz40,
            Carrier::Khz48,
        ] {
            for delta in [492, 494, 512] {
                assert!(carrier.continuous(8, delta));
            }
            assert!(!carrier.continuous(8, 2000));
            assert!(!carrier.continuous(8, carrier.ticks()));
            assert!(!carrier.continuous(100, 0));
            assert!(!carrier.continuous(u32::MAX, 0));
        }
    }

    #[test]
    fn adc_226us_scans_cover_32k_carrier_within_one_50_scan_block() {
        let mut phases = [0u32; 50];
        for (index, phase) in phases.iter_mut().enumerate() {
            *phase = (index as u32 * 226 * 64) % Carrier::Khz32.ticks();
        }
        phases.sort();
        assert!(phases.windows(2).all(|pair| pair[0] != pair[1]));
        let largest_gap = phases
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain(core::iter::once(
                phases[0] + Carrier::Khz32.ticks() - phases[49],
            ))
            .max()
            .unwrap();
        assert_eq!(largest_gap, 80); // 1.25us at64MHz; no ripple-phase alias.
    }
    #[test]
    fn adc_226us_scans_cover_40k_carrier_within_one_50_scan_block() {
        let mut phases = [0u32; 50];
        for (index, phase) in phases.iter_mut().enumerate() {
            *phase = (index as u32 * 226 * 64) % Carrier::Khz40.ticks();
        }
        phases.sort();
        assert_eq!(
            phases
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            25
        );
        let largest_gap = phases
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain(core::iter::once(
                phases[0] + Carrier::Khz40.ticks() - phases[49],
            ))
            .max()
            .unwrap();
        assert_eq!(largest_gap, 64); // 1us at64MHz; 25 positions twice per block.
    }
    #[test]
    fn adc_226us_scans_cover_48k_carrier_within_one_50_scan_block() {
        let mut phases = [0u32; 50];
        for (index, phase) in phases.iter_mut().enumerate() {
            *phase = (index as u32 * 226 * 64) % Carrier::Khz48.ticks();
        }
        phases.sort();
        assert!(phases.windows(2).all(|pair| pair[0] != pair[1]));
        let largest_gap = phases
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain(core::iter::once(
                phases[0] + Carrier::Khz48.ticks() - phases[49],
            ))
            .max()
            .unwrap();
        assert_eq!(largest_gap, 41); // 0.641us at64MHz; no ripple-phase alias.
    }
}
