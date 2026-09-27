//! Level revisit: one software-pended retry of a crossing whose edge was missed.
//!
//! **Why this exists.** The COMP line is edge-triggered, and the half-cycle
//! gate refuses every edge that arrives too early. On a sector whose one real
//! transition lands inside that blanking window, the edge is refused and the
//! comparator then simply *stays* at the post-crossing level -- no second edge
//! of the armed polarity ever comes, so the sector can never be accepted. On
//! this rig's comparator, which sits high (E049), that is exactly the falling
//! sectors: a rising sector always gets another rising edge from PWM ripple, a
//! falling sector gets one real falling edge and then silence. firmware50
//! measured this as the invariant "rising-armed sectors accept ~100%,
//! falling-armed ~10%, whichever sectors those are" (E046, E051, E052, E054)
//! and E055 ruled out servicing latency as its cause.
//!
//! The qualified image carries the remedy, compiled in:
//! `bench-running-level-revisit` is in the frozen oracle's feature closure
//! (`binz/captures/reference/reverse_48k_com_top_high_20260919/README.md`).
//! Its foreground polls, and when the gate is open, the line is live, nothing
//! is pending and the comparator *already reads the post-crossing level*, it
//! pends `ADC_COMP` in software so the ordinary ISR decision -- gate,
//! persistence, estimator -- runs as if the edge had just arrived.
//! One retry per sector.
//!
//! This is a transcription of `binz/examples/support/level_revisit.rs::admit`,
//! with the reference's half-µs units converted to this crate's µs:
//!
//! | reference term | reference value | here |
//! |---|---|---|
//! | `average_half_us >= 64` | 32 µs | `average_us >= 32` |
//! | `allow_high_speed` | `true` under `bench-running-level-revisit` | always true (the low-speed `>= 1000` half-µs floor is the *other* feature, `bench-bemf-level-revisit`, which the oracle does not carry) |
//! | `interval > average >> 1` | half-cycle gate | `elapsed_us > average_us >> 1` |
//!
//! The reference's `owner`, `active`, `coast_reference` and `software_masked`
//! are its own bench-mode flags; here they collapse into one `closed_loop`
//! input (the ISR owns the estimator and the line is ours).

/// Everything the admission decision depends on, sampled together.
#[derive(Copy, Clone, Debug)]
pub struct Inputs {
    /// The ISR owns the estimator and no accepted crossing awaits commutation.
    pub closed_loop: bool,
    /// `EXTI.IMR1` has the COMP line enabled.
    pub line_live: bool,
    /// An edge is already latched; the hardware will deliver it, no retry.
    pub pending: bool,
    /// Current interval estimate, µs.
    pub average_us: u32,
    /// Time since the last accepted crossing, µs.
    pub elapsed_us: u32,
    /// The live comparator already reads the level this sector's crossing
    /// leaves behind.
    pub post_level: bool,
    /// This sector has already had its one retry.
    pub already_retried: bool,
}

/// Shortest estimate for which a retry is admitted, µs (reference: 64 half-µs).
pub const AVERAGE_MIN_US: u32 = 32;

/// Revalidate the caller's cached polarity inside the same mask as admission.
/// This is not a generation identifier: steps repeat after six commutations.
#[inline]
#[must_use]
pub const fn sector_ready(requested: u8, detector: u32, com_phase: u32) -> bool {
    requested >= 1 && requested <= 6 && requested as u32 == detector && com_phase == 0
}

/// Admit one software retry of a missed crossing.
#[inline]
#[must_use]
pub const fn admit(v: Inputs) -> bool {
    v.closed_loop
        && v.line_live
        && !v.pending
        && v.average_us >= AVERAGE_MIN_US
        && v.elapsed_us > (v.average_us >> 1)
        && v.post_level
        && !v.already_retried
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one case that must admit: a falling sector past its gate, whose
    /// edge was refused early and whose comparator now just sits low.
    fn ready() -> Inputs {
        Inputs {
            closed_loop: true,
            line_live: true,
            pending: false,
            average_us: 1_000,
            elapsed_us: 600,
            post_level: true,
            already_retried: false,
        }
    }

    #[test]
    fn a_missed_crossing_past_the_gate_is_retried() {
        assert!(admit(ready()));
    }

    #[test]
    fn every_input_vetoes_on_its_own() {
        let cases: [fn(&mut Inputs); 7] = [
            |v| v.closed_loop = false,
            |v| v.line_live = false,
            |v| v.pending = true,
            |v| v.average_us = AVERAGE_MIN_US - 1,
            |v| v.elapsed_us = v.average_us >> 1,
            |v| v.post_level = false,
            |v| v.already_retried = true,
        ];
        for (i, f) in cases.iter().enumerate() {
            let mut v = ready();
            f(&mut v);
            assert!(!admit(v), "veto {i} did not veto");
        }
    }

    /// The gate is strict, matching the reference's `>` and the estimator's
    /// own half-cycle gate: exactly half an interval is still too early.
    #[test]
    fn the_gate_is_strictly_past_half() {
        let mut v = ready();
        v.elapsed_us = 500;
        assert!(!admit(v));
        v.elapsed_us = 501;
        assert!(admit(v));
    }

    /// The units conversion from the reference, asserted.
    #[test]
    fn floor_is_the_references_64_half_microseconds() {
        assert_eq!(AVERAGE_MIN_US, 64 / 2);
    }

    /// No speed ceiling: the running variant admits at the 15% target's
    /// 237 µs sector as well as at handoff speeds.
    #[test]
    fn admits_at_target_speed() {
        let mut v = ready();
        v.average_us = 237;
        v.elapsed_us = 200;
        assert!(admit(v));
    }

    #[test]
    fn commutation_between_outer_check_and_masked_admission_cannot_pend() {
        for cached in 1..=6u8 {
            let next = if cached == 6 { 1 } else { cached + 1 };
            // Outer idle/step observations were ready. Recheck after COM.
            let pending = sector_ready(cached, u32::from(next), 0) && admit(ready());
            assert!(!pending);
            for phase in 1..=3 {
                assert!(!sector_ready(cached, u32::from(cached), phase));
            }
            assert!(sector_ready(cached, u32::from(cached), 0) && admit(ready()));
        }
        assert!(!sector_ready(0, 0, 0));
        assert!(!sector_ready(7, 7, 0));
    }

    #[test]
    fn hardware_recheck_and_pend_share_the_mask() {
        let source = include_str!("../bin/board.rs");
        let body = source.split("fn revisit(&mut self, step: Step)").nth(1).unwrap()
            .split("fn publish_plans").next().unwrap();
        let masked = body.find("cortex_m::interrupt::free").unwrap();
        let check = body.find("revisit::sector_ready").unwrap();
        let sample = body.find("let now_raw").unwrap();
        let pend = body.find("hw::comp::pend()").unwrap();
        assert!(masked < check && check < sample && sample < pend);
        assert!(body[check..sample].contains("return false"));
    }
}
