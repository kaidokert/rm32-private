//! Pure DRV/G071 pin-role plans for a future free-running-carrier experiment.
//! NO peripheral writes or gate authority. Logical A/B/C are PA10/PB1,
//! PA9/PB0, PA8/PA7, NOT the frozen minz board's phase naming.
//!
//! Integration contract: mask interrupts; check guard/ownership; clear MOE;
//! clear ALL gate GPIO latches on BOTH ports before setting the new low leg.
//! Only then apply these MODER fields and BSRR values. Keep all three TIM1
//! CCRs equal and active before first enable; do not clear/latch them at each
//! commutation. Global safing must still clear GPIO lows as well as MOE/CCRs.
//! None of that integration is provided or hardware-qualified by this module.

pub const A_GATES: u32 = (1 << 7) | (1 << 8) | (1 << 9) | (1 << 10);
pub const B_GATES: u32 = 3;
pub const A_MODER_MASK: u32 = (3 << 14) | (3 << 16) | (3 << 18) | (3 << 20);
pub const B_MODER_MASK: u32 = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub source: u8,
    pub sink: u8,
    pub floating: u8,
    pub a_moder: u32,
    pub b_moder: u32,
    pub a_bsrr: u32,
    pub b_bsrr: u32,
}

pub fn plan(step: u8) -> Option<Plan> {
    let (source, sink) = match step {
        1 => (0, 1),
        2 => (2, 1),
        3 => (2, 0),
        4 => (1, 0),
        5 => (1, 2),
        6 => (0, 2),
        _ => return None,
    };
    let mut a_moder = 0;
    let mut b_moder = 0;
    let mut a_set = 0;
    let mut b_set = 0;
    for phase in 0..3 {
        // Only the source uses complementary timer outputs; all other pins
        // are actively driven GPIO, including both floating-phase inputs low.
        let mode = if phase == source { 2 } else { 1 };
        a_moder |= mode << (2 * (10 - phase));
        if phase == 2 {
            a_moder |= mode << 14;
        } else {
            b_moder |= mode << (2 * (1 - phase));
        }
        if phase == sink {
            if phase == 2 {
                a_set |= 1 << 7;
            } else {
                b_set |= 1 << (1 - phase);
            }
        }
    }
    Some(Plan {
        source,
        sink,
        floating: 3 - source - sink,
        a_moder,
        b_moder,
        a_bsrr: a_set | ((A_GATES & !a_set) << 16),
        b_bsrr: b_set | ((B_GATES & !b_set) << 16),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_existing_logical_geometry_without_copying_minz_pins() {
        for step in 1..=6 {
            let p = plan(step).unwrap();
            let old = super::super::sixstep::plan(step, 55).unwrap();
            assert_eq!(
                (p.source as usize, p.sink as usize, p.floating as usize),
                (old.source, old.sink, old.floating)
            );
            for phase in 0..3 {
                let high = (p.a_moder >> (2 * (10 - phase))) & 3;
                let low = if phase == 2 {
                    (p.a_moder >> 14) & 3
                } else {
                    (p.b_moder >> (2 * (1 - phase))) & 3
                };
                assert_eq!((high, low), if phase == p.source { (2, 2) } else { (1, 1) });
                let level = if phase == 2 {
                    (p.a_bsrr >> 7) & 1
                } else {
                    (p.b_bsrr >> (1 - phase)) & 1
                };
                assert_eq!(level, u32::from(phase == p.sink));
            }
        }
    }
    #[test]
    fn explicit_step_one_and_c_low_mapping() {
        let p = plan(1).unwrap(); // A PWM onPA10/PB1, B low onPB0.
        assert_eq!((p.a_moder >> 20) & 3, 2);
        assert_eq!((p.b_moder >> 2) & 3, 2);
        assert_eq!(p.b_bsrr & 65535, 1);
        assert_eq!(p.a_bsrr & 65535, 0);
        let p = plan(5).unwrap(); // B PWM, C low onPA7, A floating.
        assert_eq!((p.a_moder >> 18) & 3, 2);
        assert_eq!(p.b_moder & 3, 2);
        assert_eq!(p.a_bsrr & 65535, 1 << 7);
        assert_eq!(p.b_bsrr & 65535, 0);
    }
    #[test]
    fn unrelated_pins_untouched_and_no_conflicting_bsrr_writes() {
        for step in 1..=6 {
            let p = plan(step).unwrap();
            assert_eq!(p.a_moder & !A_MODER_MASK, 0);
            assert_eq!(p.b_moder & !B_MODER_MASK, 0);
            for (word, mask) in [(p.a_bsrr, A_GATES), (p.b_bsrr, B_GATES)] {
                assert_eq!((word & 65535) & (word >> 16), 0);
                assert_eq!((word & 65535) | (word >> 16), mask);
            }
        }
    }
    #[test]
    fn every_role_preserves_comparator_analog_inputs() {
        let a_sense = (3 << 4) | (3 << 6); // PA2 phase C, PA3 neutral.
        let b_sense = (3 << 6) | (3 << 14); // PB3 phase A, PB7 phase B.
        assert_eq!(A_MODER_MASK & a_sense, 0);
        assert_eq!(B_MODER_MASK & b_sense, 0);
        for step in 1..=6 {
            let p = plan(step).unwrap();
            let a = (a_sense & !A_MODER_MASK) | p.a_moder;
            let b = (b_sense & !B_MODER_MASK) | p.b_moder;
            assert_eq!(a & a_sense, a_sense);
            assert_eq!(b & b_sense, b_sense);
        }
        // The global disabled AF restore uses these same masks.
        assert_eq!(
            ((a_sense & !A_MODER_MASK) | (2 << 14) | (2 << 16) | (2 << 18) | (2 << 20)) & a_sense,
            a_sense
        );
        assert_eq!(((b_sense & !B_MODER_MASK) | 10) & b_sense, b_sense);
    }
    #[test]
    fn invalid_sector_never_gets_a_plan() {
        for step in [0, 7, 255] {
            assert!(plan(step).is_none());
        }
    }
}
