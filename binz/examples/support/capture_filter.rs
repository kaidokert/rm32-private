//! TIM2 TI2 filter configuration only. Never writes registers or grants authority.
//! Source: G071 PAC IC2F/CKD and RM0444 TIM2_TISEL route, see HARDWARE_SENSE_FILTER.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub ckd: u32,
    pub ccmr1: u32,
    pub tisel: u32,
    pub sample_cycles: u32,
    pub samples: u32,
}
pub fn plan(ckd: u8, filter: u8) -> Option<Plan> {
    let dts = match ckd {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => return None,
    };
    let (div, n) = match filter {
        0 => (dts, 0),
        1 => (1, 2),
        2 => (1, 4),
        3 => (1, 8),
        4 => (2 * dts, 6),
        5 => (2 * dts, 8),
        6 => (4 * dts, 6),
        7 => (4 * dts, 8),
        8 => (8 * dts, 6),
        9 => (8 * dts, 8),
        10 => (16 * dts, 5),
        11 => (16 * dts, 6),
        12 => (16 * dts, 8),
        13 => (32 * dts, 5),
        14 => (32 * dts, 6),
        15 => (32 * dts, 8),
        _ => return None,
    };
    Some(Plan {
        ckd: (ckd as u32) << 8,
        ccmr1: (1 << 8) | ((filter as u32) << 12),
        tisel: 1 << 8,
        sample_cycles: div,
        samples: n,
    })
}
impl Plan {
    /// Ideal sample-phase bracket, EXCLUDES input synchronizer and interrupt latency.
    /// For filter0 no deglitch guarantee is claimed.
    pub fn ideal_cycles(self) -> Option<(u32, u32)> {
        if self.samples == 0 {
            None
        } else {
            Some((
                (self.samples - 1) * self.sample_cycles,
                self.samples * self.sample_cycles,
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_filter_duration_vs_qualified_pwm_window() {
        // ARR2665,62/1000 duty:165 compare ticks;139 after26 timer deadtime.
        // Register arithmetic only, NOT a measurement of comparator pulse width.
        let compare = 2666 * 62 / 1000;
        let high = compare - 26;
        assert_eq!((compare, high), (165, 139));
        assert!(plan(2, 12).unwrap().ideal_cycles().unwrap().0 > compare);
        assert_eq!(plan(2, 5).unwrap().ideal_cycles(), Some((56, 64)));
        assert!(plan(2, 5).unwrap().ideal_cycles().unwrap().1 < high);
    }
    #[test]
    fn maximum_filter_uses_timer_kernel_not_counter_prescaler() {
        let p = plan(2, 15).unwrap();
        assert_eq!((p.ckd, p.ccmr1, p.tisel), (0x200, 0xf100, 0x100));
        assert_eq!(p.ideal_cycles(), Some((896, 1024))); //14..16us at64MHz
        assert_eq!(plan(0, 15).unwrap().ideal_cycles(), Some((224, 256)));
        // No PSC field: 2MHz interval count remains independent of fDTS.
    }
    #[test]
    fn no_filter_and_kernel_clock_modes() {
        assert_eq!(plan(2, 0).unwrap().ideal_cycles(), None);
        for code in 1..=3 {
            assert_eq!(
                plan(0, code).unwrap().sample_cycles,
                plan(2, code).unwrap().sample_cycles
            );
        }
        assert!(plan(3, 15).is_none());
        assert!(plan(2, 16).is_none());
    }
    #[test]
    fn all_register_values_only_touch_capture_fields() {
        for ckd in 0..=2 {
            for filter in 0..=15 {
                let p = plan(ckd, filter).unwrap();
                assert_eq!(p.ccmr1 & !0xf300, 0);
                assert_eq!(p.ccmr1 & 0x300, 0x100);
                assert_eq!(p.ckd & !0x300, 0);
                assert_eq!(p.tisel, 0x100);
            }
        }
    }
}
