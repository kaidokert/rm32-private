//! Acceptance-time flag classification; both flags is deliberately ambiguous.
//! A software-only accepted event is a lower bound on actual level rescue.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Origin {
    PhysicalOnly = 0,
    SoftwareOnly = 1,
    Both = 2,
    Neither = 3,
}

#[inline(always)]
pub const fn classify(software_flag: bool, physical_exti: bool) -> Origin {
    match (software_flag, physical_exti) {
        (false, true) => Origin::PhysicalOnly,
        (true, false) => Origin::SoftwareOnly,
        (true, true) => Origin::Both,
        (false, false) => Origin::Neither,
    }
}

#[inline(always)]
pub const fn late(measured_half_us: u16, prior_average_half_us: u32) -> bool {
    prior_average_half_us >= 64
        && measured_half_us as u32 > prior_average_half_us + (prior_average_half_us >> 2)
}

/// IT87 wire packing; event-limit values are bounded at 1000 us by policy.
#[inline(always)]
pub const fn pack_limit_origin(event_limit_us: u16, origin: Origin) -> u16 {
    event_limit_us | ((origin as u16) << 14)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_flag_combinations_are_distinct_and_bounded() {
        assert_eq!(classify(false, true), Origin::PhysicalOnly);
        assert_eq!(classify(true, false), Origin::SoftwareOnly);
        assert_eq!(classify(true, true), Origin::Both);
        assert_eq!(classify(false, false), Origin::Neither);
        for software in [false, true] {
            for physical in [false, true] {
                assert!((classify(software, physical) as usize) < 4);
            }
        }
    }

    #[test]
    fn late_threshold_is_strict_and_division_free() {
        assert!(!late(250, 200));
        assert!(late(251, 200));
        assert!(!late(500, 0));
        assert!(!late(80, 63));
    }

    #[test]
    fn origin_fits_above_bounded_event_limit() {
        for origin in [
            Origin::PhysicalOnly,
            Origin::SoftwareOnly,
            Origin::Both,
            Origin::Neither,
        ] {
            let packed = pack_limit_origin(1000, origin);
            assert_eq!(packed & 0x3fff, 1000);
            assert_eq!((packed >> 14) as u8, origin as u8);
        }
    }
}
