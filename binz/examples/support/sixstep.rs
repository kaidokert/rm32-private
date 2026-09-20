//! DRV/G071 register plans for rm32's 1-based six-step convention.
//! Geometry follows ../rm32_stm32/src/phase.rs::com_step verbatim.
//! A/B/C are TIM1 CH3/CH2/CH1; current map is separately qualified.

#[derive(Clone, Copy, Debug)]
pub struct Plan {
    pub source: usize,
    pub sink: usize,
    pub floating: usize,
    pub ccmr1: u32,
    pub ccmr2: u32,
    pub ccer: u32,
    pub ccr: [u32; 3], // physical CH1/2/3 order
}

pub fn plan(step: u8, duty_tenths: u32) -> Option<Plan> {
    plan_with_period(step, duty_tenths, 6400)
}
pub fn plan_with_period(step: u8, duty_tenths: u32, period: u32) -> Option<Plan> {
    if period == 0 || period > 65536 {
        return None;
    }
    let (source, sink) = match step {
        1 => (0, 1),
        2 => (2, 1),
        3 => (2, 0),
        4 => (1, 0),
        5 => (1, 2),
        6 => (0, 2),
        _ => return None,
    };
    let mut mode = [0x40; 3];
    let mut ccr = [0; 3];
    mode[2 - source] = 0x68;
    ccr[2 - source] = period * duty_tenths.min(100) / 1000;
    Some(Plan {
        source,
        sink,
        floating: 3 - source - sink,
        ccmr1: mode[0] | mode[1] << 8,
        ccmr2: mode[2],
        ccer: (5 << ((2 - source) * 4)) | (5 << ((2 - sink) * 4)),
        ccr,
    })
}

pub const fn step_for_values(values: [u8; 3]) -> u8 {
    let mut source = 0;
    let mut sink = 0;
    let mut i = 1;
    while i < 3 {
        if values[i] >= values[source] {
            source = i;
        }
        if values[i] < values[sink] {
            sink = i;
        }
        i += 1;
    }
    match (source, sink) {
        (0, 1) => 1,
        (2, 1) => 2,
        (2, 0) => 3,
        (1, 0) => 4,
        (1, 2) => 5,
        (0, 2) => 6,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compare_tracks_actual_period_and_never_doubles_twenty_khz_duty() {
        for period in [2667, 3200, 6400] {
            for step in 1..=6 {
                for duty in 0..=100 {
                    let p = plan_with_period(step, duty, period).unwrap();
                    assert_eq!(p.ccr.iter().sum::<u32>(), period * duty / 1000);
                    assert!(p.ccr.iter().all(|c| *c <= period / 10));
                }
            }
        }
        assert_eq!(
            plan_with_period(1, 61, 3200)
                .unwrap()
                .ccr
                .iter()
                .sum::<u32>(),
            195
        );
        assert!(plan_with_period(1, 61, 0).is_none());
        assert!(plan_with_period(1, 61, 65537).is_none());
    }
    #[test]
    fn reference_geometry_and_float_are_preserved() {
        for step in 1..=6 {
            let p = plan(step, 60).unwrap();
            assert_ne!(p.source, p.sink);
            assert_eq!(p.floating, [2, 0, 1, 2, 0, 1][step as usize - 1]);
            assert_eq!((p.ccer >> ((2 - p.floating) * 4)) & 15, 0);
            assert_eq!((p.ccer >> ((2 - p.sink) * 4)) & 15, 5);
            assert_eq!((p.ccer >> ((2 - p.source) * 4)) & 15, 5);
            assert_eq!(p.ccr.iter().sum::<u32>(), 384);
        }
        assert!(plan(0, 60).is_none());
        assert!(plan(7, 60).is_none());
    }
    #[test]
    fn no_unbounded_duty() {
        for step in 1..=6 {
            assert_eq!(plan(step, u32::MAX).unwrap().ccr.iter().sum::<u32>(), 640);
        }
    }
    #[test]
    fn extrema_match_sector_roles() {
        for step in 1..=6 {
            let p = plan(step, 60).unwrap();
            let mut values = [128; 3];
            values[p.source] = 255;
            values[p.sink] = 0;
            assert_eq!(step_for_values(values), step);
        }
    }
}
