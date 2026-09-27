//! TIM1 register plan for one six-step sector.
//!
//! **This is a refactor of binz's `examples/support/sixstep.rs`, not a new
//! design.** The concept is the reference's and is preserved exactly:
//!
//! * phases A/B/C are TIM1 CH3/CH2/CH1, so a phase index `p` occupies
//!   register slot `2 - p`;
//! * the **source** channel runs PWM mode 1 (`OCxM = 0b110`); all three
//!   channels carry the **same** compare, as the reference's role plan keeps
//!   them (E063 -- the binz sixstep plan zeroed the other two, and a preloaded
//!   zero delayed every new source by up to a carrier period);
//! * the **sink** channel is set to *force inactive* (`OCxM = 0b100`), which
//!   drives its output low and therefore its complement high -- the low-side
//!   FET is held on without depending on a compare value;
//! * the **floating** channel is force-inactive with `CCER` nibble `0b0001`
//!   (`CCxE` only): with `OSSR = 1` that makes OCx = OCxREF = 0 and OCxN its
//!   off-state level (`CCxNP` = 0), so **both gate inputs are actively driven
//!   low** and the half-bridge is off (E068). It used to clear the nibble,
//!   which RM0444's output-control table makes *Hi-Z* -- the DRV8304 inputs
//!   then rest on their weak internal pull-downs beside 48 kHz switching. The
//!   reference drives "all other pins ... actively ... including both
//!   floating-phase inputs low" (`binz/examples/support/phase_gpio_plan.rs`);
//! * `CCER` carries nibble `0b0101` (`CCxE | CCxNE`) for source and sink.
//!
//! What was deliberately *not* carried over: the reference clamps duty at 100
//! tenths (10.0%), which is its qualified envelope. Here the clamp is a
//! parameter so a 15% target can be expressed without editing the plan, and
//! the reference's value remains available as [`REFERENCE_DUTY_CAP`].
//!
//! Why a register *plan* rather than direct writes: it is a pure function, so
//! the sector geometry and the float invariant are host-testable without
//! hardware, which is how the reference validated them too.

use crate::commutation::{Phase, Step, sector};

#[cfg(test)]
mod transition_tests;
#[cfg(test)]
mod latch_transaction_tests;

/// Duty ceiling the reference plan enforces, in tenths of a percent.
pub const REFERENCE_DUTY_CAP: u16 = 100;

/// Output-compare mode for the chopped phase: PWM mode 1 with preload.
const MODE_PWM1: u32 = 0x68;
/// Output-compare mode for the held and floating phases: force inactive.
const MODE_FORCE_INACTIVE: u32 = 0x40;
/// `CCER` nibble enabling an output and its complement.
const CCER_PAIR: u32 = 0b0101;
/// `CCER` nibble for the floating phase: `CCxE` only. With `OSSR = 1` and the
/// channel force-inactive, both OCx and OCxN are driven low (see module docs).
const CCER_FLOAT: u32 = 0b0001;

/// Register values for one sector.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Plan {
    /// Phase indices, 0 = A, 1 = B, 2 = C.
    pub source: usize,
    pub sink: usize,
    pub floating: usize,
    pub ccmr1: u32,
    pub ccmr2: u32,
    pub ccer: u32,
    /// Compare values in physical CH1/CH2/CH3 order.
    pub ccr: [u32; 3],
}

/// Register slot for a phase index: A/B/C map to CH3/CH2/CH1.
#[inline]
const fn slot(phase_ix: usize) -> usize {
    2 - phase_ix
}

#[inline]
const fn phase_ix(p: Phase) -> usize {
    p as usize
}

/// Build the plan for `step` at `duty_tenths`, on a timer period of
/// `period` ticks, clamping duty at `duty_cap`.
///
/// Returns `None` for a period the timer cannot represent.
#[must_use]
pub fn plan(step: Step, duty_tenths: u16, period: u32, duty_cap: u16) -> Option<Plan> {
    if period == 0 || period > 65536 {
        return None;
    }
    let s = sector(step);
    let source = phase_ix(s.source);
    let sink = phase_ix(s.sink);
    let floating = phase_ix(s.floating);

    // Every channel starts forced inactive; the source is then switched to
    // PWM. Starting from "inactive" rather than "PWM" is what makes the sink
    // and the floating phase safe by default.
    let mut mode = [MODE_FORCE_INACTIVE; 3];
    mode[slot(source)] = MODE_PWM1;
    let duty = if duty_tenths > duty_cap { duty_cap } else { duty_tenths };
    // **All three compares equal (E063).** The CCRs are preloaded (OCxPE), so
    // a compare written at commutation only takes effect at the next timer
    // update. With the source's compare written fresh and the others zeroed,
    // the channel that just became source had a stale zero loaded and did not
    // chop for up to a whole carrier period -- a gap in which the floating
    // phase transients, and the driven observer accepted those transients at
    // positions set by the carrier phase (E060). The qualified image keeps
    // "all three TIM1 CCRs equal and active ... do not clear/latch them at each
    // commutation" (`binz/examples/support/phase_gpio_plan.rs:5-9`,
    // `bench-pwm-roles`) and switches only roles. Here the role is the mode:
    // the sink is force-inactive whatever its compare, and the floating phase
    // has both enables cleared, so an equal compare on either is inert.
    let c = period * duty as u32 / 1000;
    let ccr = [c; 3];

    Some(Plan {
        source,
        sink,
        floating,
        ccmr1: mode[0] | (mode[1] << 8),
        ccmr2: mode[2],
        ccer: (CCER_PAIR << (slot(source) * 4))
            | (CCER_PAIR << (slot(sink) * 4))
            | (CCER_FLOAT << (slot(floating) * 4)),
        ccr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: u32 = 6400;

    fn steps() -> impl Iterator<Item = Step> {
        (1u8..=6).map(Step::new_clamped)
    }

    /// The reference's own geometry assertion, ported: float slot disabled,
    /// source and sink enabled, and the floating phase follows C,A,B,C,A,B.
    #[test]
    fn reference_geometry_and_float_are_preserved() {
        let expect_float = [2usize, 0, 1, 2, 0, 1];
        for (i, step) in steps().enumerate() {
            let p = plan(step, 60, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            assert_ne!(p.source, p.sink, "step {}", i + 1);
            assert_eq!(p.floating, expect_float[i], "step {}", i + 1);
            // Float: CCxE only -- OCx forced low, OCxN at its off-state low.
            assert_eq!((p.ccer >> (slot(p.floating) * 4)) & 15, CCER_FLOAT, "float driven low");
            assert_eq!((p.ccer >> (slot(p.sink) * 4)) & 15, CCER_PAIR);
            assert_eq!((p.ccer >> (slot(p.source) * 4)) & 15, CCER_PAIR);
            assert_eq!(p.ccr, [384; 3]);
        }
    }

    /// All three compares are equal in every sector, so the channel that
    /// becomes source already holds its compare when its mode switches -- the
    /// reference's equal-CCR invariant (E063). Only the source chops: the sink
    /// is force-inactive and the floating phase is disabled.
    #[test]
    fn all_three_compares_are_equal_and_only_the_source_chops() {
        for step in steps() {
            let p = plan(step, 100, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            assert_eq!(p.ccr[0], p.ccr[1]);
            assert_eq!(p.ccr[1], p.ccr[2]);
            assert!(p.ccr[slot(p.source)] > 0);
            let modes = [p.ccmr1 & 0xFF, (p.ccmr1 >> 8) & 0xFF, p.ccmr2 & 0xFF];
            assert_eq!(modes.iter().filter(|m| **m == MODE_PWM1).count(), 1);
            assert_eq!((p.ccer >> (slot(p.floating) * 4)) & 15, CCER_FLOAT);
        }
    }

    /// The floating half-bridge is off with both inputs driven: its channel is
    /// force-inactive (OCx low) and only `CCxE` is set, so with `OSSR = 1` the
    /// complement is held at its off-state level rather than released to Hi-Z.
    /// Neither the high side (OCx) nor the low side (OCxN) can conduct.
    #[test]
    fn the_floating_phase_is_driven_off_not_released() {
        for step in steps() {
            let p = plan(step, 100, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            let modes = [p.ccmr1 & 0xFF, (p.ccmr1 >> 8) & 0xFF, p.ccmr2 & 0xFF];
            assert_eq!(modes[slot(p.floating)], MODE_FORCE_INACTIVE);
            let nib = (p.ccer >> (slot(p.floating) * 4)) & 15;
            assert_eq!(nib & 0b0001, 0b0001, "CCxE set: OCx driven (to its forced-low ref)");
            assert_eq!(
                nib & 0b0100,
                0,
                "CCxNE clear: OCxN held at off-state, never complementary"
            );
            assert_eq!(nib & 0b1010, 0, "no polarity inversion");
        }
    }

    /// Consecutive sectors carry the same compare, so a commutation never
    /// depends on a preload taking effect.
    #[test]
    fn a_commutation_changes_no_compare() {
        for step in steps() {
            let a = plan(step, 62, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            let b = plan(step.next(), 62, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            assert_eq!(a.ccr, b.ccr);
        }
    }

    /// The source runs PWM; sink and floating are forced inactive.
    #[test]
    fn modes_match_the_role_of_each_phase() {
        for step in steps() {
            let p = plan(step, 60, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            let modes = [p.ccmr1 & 0xFF, (p.ccmr1 >> 8) & 0xFF, p.ccmr2 & 0xFF];
            assert_eq!(modes[slot(p.source)], MODE_PWM1);
            assert_eq!(modes[slot(p.sink)], MODE_FORCE_INACTIVE);
            assert_eq!(modes[slot(p.floating)], MODE_FORCE_INACTIVE);
        }
    }

    #[test]
    fn compare_tracks_the_actual_period() {
        for period in [2667u32, 3200, 6400] {
            for step in steps() {
                for duty in 0u16..=100 {
                    let p = plan(step, duty, period, REFERENCE_DUTY_CAP).unwrap();
                    assert_eq!(p.ccr, [period * duty as u32 / 1000; 3]);
                    assert!(p.ccr.iter().all(|c| *c <= period / 10));
                }
            }
        }
    }

    #[test]
    fn duty_is_clamped_at_the_cap_and_never_unbounded() {
        for step in steps() {
            // The reference cap.
            let p = plan(step, u16::MAX, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            assert_eq!(p.ccr, [640; 3]);
            // A raised cap for the 15% target, stated explicitly.
            let p = plan(step, u16::MAX, PERIOD, 150).unwrap();
            assert_eq!(p.ccr, [960; 3]);
        }
    }

    #[test]
    fn a_compare_never_exceeds_the_period() {
        for step in steps() {
            for duty in [0u16, 100, 150, 300, u16::MAX] {
                let p = plan(step, duty, PERIOD, 300).unwrap();
                assert!(p.ccr.iter().all(|c| *c <= PERIOD));
            }
        }
    }

    #[test]
    fn an_unrepresentable_period_is_refused() {
        assert!(plan(Step::new_clamped(1), 60, 0, REFERENCE_DUTY_CAP).is_none());
        assert!(plan(Step::new_clamped(1), 60, 65_537, REFERENCE_DUTY_CAP).is_none());
        assert!(plan(Step::new_clamped(1), 60, 65_536, REFERENCE_DUTY_CAP).is_some());
    }

    /// Walking the six sectors energises each phase as source exactly twice
    /// and floats each exactly twice -- the property that makes it a balanced
    /// electrical revolution rather than six unrelated states.
    #[test]
    fn six_sectors_are_a_balanced_revolution() {
        let mut as_source = [0u8; 3];
        let mut as_sink = [0u8; 3];
        let mut as_float = [0u8; 3];
        for step in steps() {
            let p = plan(step, 60, PERIOD, REFERENCE_DUTY_CAP).unwrap();
            as_source[p.source] += 1;
            as_sink[p.sink] += 1;
            as_float[p.floating] += 1;
        }
        assert_eq!(as_source, [2, 2, 2]);
        assert_eq!(as_sink, [2, 2, 2]);
        assert_eq!(as_float, [2, 2, 2]);
    }
}
