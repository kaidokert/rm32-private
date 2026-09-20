//! Signed bridge-return block average. Calibration and ADC coverage are caller
//! obligations; this policy never invents amperes from a nominal gain.
//! At 201 us/scan and 20 kHz PWM, 50 scans visit all 50 carrier phases.
//! The explicit 24 kHz A/B uses 226 us to distribute samples around the
//! carrier. The high-duty fast policy uses20 unique phases with a maximum
//! uncovered arc of278/2666 timer ticks (4.35us) and a4.52ms response.
//! This remains an average, not an instantaneous SOA clamp.
pub const SCANS: u32 = if cfg!(feature = "bench-current-fast-20") {
    20
} else if cfg!(all(
    feature = "bench-startup-adc",
    not(feature = "bench-startup-20k")
)) {
    100
} else {
    50
};
/// Match a 128-scan pre-drive zero to one average window. Round zero UP:
/// positive return current is zero minus output (DRV8304 equation 3).
/// Upward rounding cannot underestimate its positive residual. Caller verifies
/// ENABLE epoch, ADC configuration and stationarity before using the result.
pub fn zero_from_128(sums: [u32; 3]) -> Option<i32> {
    if sums.iter().any(|&s| s > 4095 * 128) {
        return None;
    }
    let total = sums[0] + sums[1] + sums[2];
    Some(((total * SCANS + 127) >> 7) as i32)
}
pub struct Limit {
    zero_block: i32,
    limit_block: i32,
    sum: i32,
    count: u32,
    over: u8,
    tripped: bool,
}
impl Limit {
    /// Both quantities use sums of raw ADC counts over SCANS scans. zero_block
    /// must come from the same ENABLE epoch, outputs off and rotor stationary.
    /// limit_block is a positive current threshold BELOW that output zero.
    pub fn new(zero_block: i32, limit_block: i32) -> Option<Self> {
        const MAX: i32 = (4095 * 3 * SCANS) as i32;
        if !(0..=MAX).contains(&zero_block) || !(1..=MAX).contains(&limit_block) {
            return None;
        }
        Some(Self {
            zero_block,
            limit_block,
            sum: 0,
            count: 0,
            over: 0,
            tripped: false,
        })
    }
    /// Every coherent scan, not mid-ON-only samples. Sequential channels are
    /// accumulated as means, not claimed to be an instantaneous Kirchhoff sum.
    /// Phase codes are already bounded by the ADC. Include clipped 0/4095
    /// values in the signed average instead of treating an asynchronous PWM
    /// ripple peak as a DC-link fault; the live wrapper reports clipping.
    pub fn scan(&mut self, raw: [u16; 3]) -> Option<i32> {
        if self.tripped {
            return None;
        }
        self.sum += raw[0] as i32 + raw[1] as i32 + raw[2] as i32;
        self.count += 1;
        if self.count != SCANS {
            return None;
        }
        // Cached SLVSE39B p28 equation 3: I=(VREF/2-VSO)/(gain*R).
        // This is signed bridge return, not an absolute ripple magnitude.
        let residual = self.zero_block - self.sum;
        self.sum = 0;
        self.count = 0;
        if residual > self.limit_block {
            self.over = self.over.saturating_add(1);
            if self.over >= 2 {
                self.tripped = true;
            }
        } else {
            self.over = 0;
        }
        Some(residual)
    }
    pub fn tripped(&self) -> bool {
        self.tripped
    }
    pub fn over_streak(&self) -> u8 {
        self.over
    }
    /// Foreground calls this only after it has successfully published a lower
    /// PWM ceiling for a latched first-over warning. If a second complete
    /// over-limit block already tripped, acknowledgment is too late and the
    /// terminal fault remains latched. A later under-limit block may have
    /// cleared `over`; acknowledging the still-latched warning is conservative.
    pub fn acknowledge_reduction(&mut self) -> bool {
        if self.tripped {
            return false;
        }
        self.over = 0;
        true
    }
    pub fn discard_partial(&mut self) {
        self.sum = 0;
        self.count = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn block(g: &mut Limit, raw: [u16; 3]) -> i32 {
        for _ in 1..SCANS {
            assert_eq!(g.scan(raw), None);
        }
        g.scan(raw).unwrap()
    }
    #[test]
    fn exact_average_boundary_and_latched_fault() {
        let zero = 6144 * SCANS as i32;
        let mut g = Limit::new(zero, SCANS as i32).unwrap();
        assert_eq!(block(&mut g, [2047, 2048, 2048]), SCANS as i32);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(g.tripped());
        assert_eq!(g.scan([2048; 3]), None);
    }
    #[test]
    fn first_over_is_warning_and_second_consecutive_over_is_terminal() {
        let zero = 6144 * SCANS as i32;
        let mut g = Limit::new(zero, SCANS as i32).unwrap();
        assert_eq!(g.over_streak(), 0);
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(g.over_streak(), 1);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(g.over_streak(), 2);
        assert!(g.tripped());

        let mut g = Limit::new(zero, SCANS as i32).unwrap();
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(block(&mut g, [2048; 3]), 0);
        assert_eq!(g.over_streak(), 0);
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(g.over_streak(), 1);
        assert!(!g.tripped());
    }
    #[test]
    fn applied_reduction_rearms_warning_but_never_clears_a_trip() {
        let zero = 6144 * SCANS as i32;
        let mut g = Limit::new(zero, SCANS as i32).unwrap();
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(g.over_streak(), 1);
        assert!(g.acknowledge_reduction());
        assert_eq!(g.over_streak(), 0);
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(!g.tripped());
        assert!(g.acknowledge_reduction());

        // A warning remains actionable even if an under-limit block arrives
        // before foreground consumes it; the resulting reduction is safe.
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(block(&mut g, [2048; 3]), 0);
        assert!(g.acknowledge_reduction());

        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(g.tripped());
        assert!(!g.acknowledge_reduction());
    }
    #[test]
    fn signed_recirculation_cancels_without_absolute_value_bias() {
        let mut g = Limit::new(6144 * SCANS as i32, 1).unwrap();
        assert_eq!(block(&mut g, [3048, 1048, 2048]), 0);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2049; 3]), -3 * SCANS as i32);
        assert!(!g.tripped());
    }
    #[test]
    fn reference_equation_positive_return_lowers_output() {
        let zero = 6144 * SCANS as i32;
        let mut g = Limit::new(zero, SCANS as i32).unwrap();
        // Regeneration has the opposite signed direction and is NOT abs'd.
        assert_eq!(block(&mut g, [2050, 2048, 2048]), -2 * SCANS as i32);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2046, 2048, 2048]), 2 * SCANS as i32);
        assert!(g.tripped());
    }
    #[test]
    fn invalid_configuration_and_adc_rails_are_bounded_samples() {
        let maximum = (4095 * 3 * SCANS) as i32;
        for (zero, limit) in [
            (-1, 1),
            (maximum + 1, 1),
            (196608, 0),
            (196608, maximum + 1),
        ] {
            assert!(Limit::new(zero, limit).is_none());
        }
        let zero = 6144 * SCANS as i32;
        let mut high = Limit::new(zero, 1).unwrap();
        assert_eq!(block(&mut high, [2048, 4095, 2048]), -2047 * SCANS as i32);
        assert!(!high.tripped());
        let mut low = Limit::new(zero, 4096 * SCANS as i32).unwrap();
        assert_eq!(block(&mut low, [2048, 0, 2048]), 2048 * SCANS as i32);
        assert!(!low.tripped());
    }
    #[test]
    fn blocks_do_not_accumulate_previous_windows() {
        let zero = 6144 * SCANS as i32;
        let residual = 3 * SCANS as i32;
        let mut g = Limit::new(zero, residual - 1).unwrap();
        assert_eq!(block(&mut g, [2047; 3]), residual);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2048; 3]), 0);
        assert_eq!(block(&mut g, [2047; 3]), residual);
        assert!(!g.tripped());
        assert_eq!(block(&mut g, [2047; 3]), residual);
        assert!(g.tripped());
        let mut g = Limit::new(zero, residual).unwrap();
        for _ in 0..1000 {
            assert_eq!(block(&mut g, [2047; 3]), residual);
            assert!(!g.tripped());
        }
    }
    #[test]
    fn baseline_scaling_is_bounded_and_conservative() {
        for extra in 0..128u32 {
            let sums = [2048 * 128 + extra, 2048 * 128, 2048 * 128];
            let zero = zero_from_128(sums).unwrap() as u32;
            let numerator = sums.iter().sum::<u32>() * SCANS;
            assert!(zero * 128 >= numerator && zero * 128 - numerator < 128);
        }
        assert_eq!(
            zero_from_128([4095 * 128; 3]),
            Some((4095 * 3 * SCANS) as i32)
        );
        assert_eq!(zero_from_128([4095 * 128 + 1, 0, 0]), None);
    }
    #[test]
    fn cadence_covers_each_carrier_phase_once() {
        for period in [50usize, 100] {
            let mut seen = [false; 100];
            for scan in 0..period {
                let phase = scan * 201 % period;
                assert!(!seen[phase]);
                seen[phase] = true;
            }
            assert!(seen[..period].iter().all(|&s| s));
        }
        assert_eq!(
            SCANS,
            if cfg!(feature = "bench-current-fast-20") {
                20
            } else if cfg!(all(
                feature = "bench-startup-adc",
                not(feature = "bench-startup-20k")
            )) {
                100
            } else {
                50
            }
        );

        let mut seen = [false; 2666];
        for scan in 0..50usize {
            let phase = scan * 226 * 64 % 2666;
            assert!(!seen[phase]);
            seen[phase] = true;
        }
        let mut last = None;
        let mut first = 0;
        let mut maximum_gap = 0;
        for (phase, &hit) in seen.iter().enumerate() {
            if hit {
                if let Some(previous) = last {
                    maximum_gap = maximum_gap.max(phase - previous);
                } else {
                    first = phase;
                }
                last = Some(phase);
            }
        }
        maximum_gap = maximum_gap.max(2666 - last.unwrap() + first);
        assert!(maximum_gap <= 60);

        let mut phases = [0usize; 20];
        for (scan, phase) in phases.iter_mut().enumerate() {
            *phase = scan * 226 * 64 % 2666;
        }
        phases.sort_unstable();
        for pair in phases.windows(2) {
            assert_ne!(pair[0], pair[1]);
        }
        let maximum_gap = phases
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .chain(core::iter::once(2666 - phases[19] + phases[0]))
            .max()
            .unwrap();
        assert_eq!(maximum_gap, 278);
    }
}
