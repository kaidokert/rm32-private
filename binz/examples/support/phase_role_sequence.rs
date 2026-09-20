//! Ordered role-only commutation. Caller supplies exclusive, guarded authority.
//! This module does not grant it. Legacy prepared segments have ONE fixed duty.
//! The separate live-prepared entry requires coherent CCR/metadata publication
//! by its guarded caller; it cannot initialize or change a duty by itself.
use super::carrier_profile::Carrier;
use super::phase_gpio_plan::{self, Plan};

pub trait Registers {
    /// Clear MOE, then ALL gate latches on BOTH ports, before any role writes.
    fn blank(&mut self);
    /// All PWM modes, complementary enables, equal CCRs, initial UG; MOE stays off.
    fn prepare_equal(&mut self, compare: u32);
    fn modes(&mut self, plan: &Plan);
    fn sink(&mut self, plan: &Plan);
    fn mux(&mut self, floating: u8);
    fn enable(&mut self);
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Sector,
    Duty,
    DutyChanged,
}

/// Disabled preparation only: no role selection, sink, mux or enable operation.
/// Caller must validate geometry/ownership and serialize publication separately.
#[cfg(feature = "bench-recovery-duty-check")]
pub fn prepare_live_disabled<R: Registers>(io: &mut R, request: super::live_duty::Prepared) -> u32 {
    io.blank();
    io.prepare_equal(request.compare());
    request.duty()
}

/// Only for a coherently updated live segment. Never initializes CCRs or UG.
/// Caller holds the powered guard lock and has published matching metadata.
/// Ordinary apply_carrier retains its existing fixed-duty/startup limits.
pub fn apply_live_prepared<R: Registers>(
    io: &mut R,
    prepared: u32,
    step: u8,
    duty: u32,
) -> Result<u32, Refusal> {
    let plan = phase_gpio_plan::plan(step).ok_or(Refusal::Sector)?;
    if !super::duty_envelope::contains(duty) {
        return Err(Refusal::Duty);
    }
    if prepared != duty {
        return Err(Refusal::DutyChanged);
    }
    io.blank();
    io.modes(&plan);
    io.sink(&plan);
    io.mux(plan.floating);
    io.enable();
    Ok(duty)
}

pub fn apply<R: Registers>(io: &mut R, prepared: u32, step: u8, duty: u32) -> Result<u32, Refusal> {
    apply_carrier(io, prepared, step, duty, Carrier::Khz10)
}

/// Caller must establish the matching timer period while outputs are off,
/// BEFORE preparing any observation stream; never change it mid-segment.
pub fn apply_carrier<R: Registers>(
    io: &mut R,
    prepared: u32,
    step: u8,
    duty: u32,
    carrier: Carrier,
) -> Result<u32, Refusal> {
    let plan = phase_gpio_plan::plan(step).ok_or(Refusal::Sector)?;
    // Same duty refusal precedence, but no general division on an already
    // prepared segment. Its equal CCRs remain the cached segment value.
    let maximum = if cfg!(feature = "bench-startup-adc") {
        300
    } else {
        100
    };
    if duty == 0 || duty > maximum {
        return Err(Refusal::Duty);
    }
    if prepared != 0 && prepared != duty {
        return Err(Refusal::DutyChanged);
    }
    io.blank();
    if prepared == 0 {
        prepare_equal_for_carrier(io, carrier, duty);
    }
    io.modes(&plan);
    io.sink(&plan);
    io.mux(plan.floating);
    io.enable();
    Ok(duty)
}

// Keep division together with the once-per-segment hardware write. This
// side-effecting call cannot be speculated onto prepared commutations merely
// because compare() is pure. Inspect the linked ISR as well as this source.
#[inline(never)]
fn prepare_equal_for_carrier<R: Registers>(io: &mut R, carrier: Carrier, duty: u32) {
    let compare = if cfg!(feature = "bench-startup-adc") {
        carrier.ticks() * duty / 1000
    } else {
        carrier.compare(duty).expect("validated segment duty")
    };
    io.prepare_equal(compare);
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;
    #[derive(Default)]
    struct Mock {
        events: Vec<u32>,
    }
    impl Registers for Mock {
        fn blank(&mut self) {
            self.events.push(1);
        }
        fn prepare_equal(&mut self, c: u32) {
            self.events.push(1000 + c);
        }
        fn modes(&mut self, p: &Plan) {
            self.events.push(10 + p.source as u32);
        }
        fn sink(&mut self, p: &Plan) {
            self.events.push(20 + p.sink as u32);
        }
        fn mux(&mut self, p: u8) {
            self.events.push(30 + p as u32);
        }
        fn enable(&mut self) {
            self.events.push(40);
        }
    }
    #[test]
    fn live_prepared_all_duties_and_sectors_never_reload_compares() {
        for duty in super::super::duty_envelope::MIN..=super::super::duty_envelope::MAX {
            for step in 1..=6 {
                let mut io = Mock::default();
                assert_eq!(apply_live_prepared(&mut io, duty, step, duty), Ok(duty));
                let p = phase_gpio_plan::plan(step).unwrap();
                assert_eq!(
                    io.events,
                    [
                        1,
                        10 + p.source as u32,
                        20 + p.sink as u32,
                        30 + p.floating as u32,
                        40
                    ]
                );
            }
        }
    }
    #[cfg(feature = "bench-recovery-duty-check")]
    #[test]
    fn disabled_preparation_never_selects_or_energizes_a_phase() {
        for ticks in [6400, 3200, 2666] {
            for duty in super::super::duty_envelope::MIN..=super::super::duty_envelope::MAX {
                let request = super::super::live_duty::Prepared::new(ticks, duty).unwrap();
                let mut io = Mock::default();
                assert_eq!(prepare_live_disabled(&mut io, request), duty);
                assert_eq!(io.events, [1, 1000 + ticks * duty / 1000]);
            }
        }
    }
    #[test]
    fn live_prepared_refuses_stale_unprepared_or_invalid_without_writes() {
        for (prepared, step, duty, why) in [
            (0, 1, 70, Refusal::DutyChanged),
            (70, 1, 80, Refusal::DutyChanged),
            (80, 1, 70, Refusal::DutyChanged),
            (0, 1, 0, Refusal::Duty),
            (39, 1, 39, Refusal::Duty),
            (
                super::super::duty_envelope::MAX + 1,
                1,
                super::super::duty_envelope::MAX + 1,
                Refusal::Duty,
            ),
            (u32::MAX, 1, u32::MAX, Refusal::Duty),
            (300, 0, 300, Refusal::Sector),
            (300, 7, 300, Refusal::Sector),
        ] {
            let mut io = Mock::default();
            assert_eq!(apply_live_prepared(&mut io, prepared, step, duty), Err(why));
            assert!(io.events.is_empty());
        }
        #[cfg(not(feature = "bench-startup-adc"))]
        {
            let mut io = Mock::default();
            // Neither legacy entry point inherits the live range.
            assert_eq!(apply(&mut io, 300, 1, 300), Err(Refusal::Duty));
            for carrier in [Carrier::Khz10, Carrier::Khz20, Carrier::Khz24, Carrier::Khz32, Carrier::Khz40, Carrier::Khz48] {
                assert_eq!(
                    apply_carrier(&mut io, 300, 1, 300, carrier),
                    Err(Refusal::Duty)
                );
            }
            assert!(io.events.is_empty());
        }
    }
    #[test]
    fn initial_load_precedes_roles_and_enable() {
        let mut io = Mock::default();
        assert_eq!(apply(&mut io, 0, 1, 54), Ok(54));
        assert_eq!(io.events, [1, 1345, 10, 21, 32, 40]);
    }
    #[test]
    fn all_36_role_transitions_do_not_reload_or_reset_carrier() {
        for before in 1..=6 {
            for after in 1..=6 {
                let mut io = Mock::default();
                let duty = apply(&mut io, 0, before, 55).unwrap();
                io.events.clear();
                assert_eq!(apply(&mut io, duty, after, 55), Ok(55));
                let p = phase_gpio_plan::plan(after).unwrap();
                assert_eq!(
                    io.events,
                    [
                        1,
                        10 + p.source as u32,
                        20 + p.sink as u32,
                        30 + p.floating as u32,
                        40
                    ]
                );
            }
        }
    }
    #[test]
    fn invalid_input_never_writes_and_caller_must_safe_on_refusal() {
        for (prepared, step, duty, why) in [
            (0, 0, 54, Refusal::Sector),
            (0, 7, 54, Refusal::Sector),
            (0, 1, 0, Refusal::Duty),
            (
                0,
                1,
                if cfg!(feature = "bench-startup-adc") {
                    301
                } else {
                    101
                },
                Refusal::Duty,
            ),
            (0, 1, u32::MAX, Refusal::Duty),
            (54, 1, 55, Refusal::DutyChanged),
        ] {
            let mut io = Mock::default();
            assert_eq!(apply(&mut io, prepared, step, duty), Err(why));
            assert!(io.events.is_empty());
        }
    }
    #[test]
    fn cleared_preparation_requires_new_equal_load() {
        let mut io = Mock::default();
        apply(&mut io, 0, 6, 100).unwrap();
        assert_eq!(io.events[1], 1640);
        io.events.clear();
        apply(&mut io, 0, 2, 40).unwrap();
        assert_eq!(io.events[1], 1256);
    }
    #[test]
    fn candidate_carrier_only_changes_initial_compare_not_role_order() {
        for step in 1..=6 {
            let mut io = Mock::default();
            apply_carrier(&mut io, 0, step, 54, Carrier::Khz24).unwrap();
            assert_eq!(io.events[1], 1143);
            io.events.clear();
            apply_carrier(&mut io, 54, step, 54, Carrier::Khz24).unwrap();
            assert_eq!(io.events.len(), 5);
            assert_eq!(io.events[0], 1);
            assert_eq!(io.events[4], 40);
        }
    }
    #[test]
    fn every_supported_duty_carrier_prepares_once_with_exact_compare() {
        let maximum = if cfg!(feature = "bench-startup-adc") {
            300
        } else {
            100
        };
            for carrier in [Carrier::Khz10, Carrier::Khz20, Carrier::Khz24, Carrier::Khz32, Carrier::Khz40, Carrier::Khz48] {
            for duty in 1..=maximum {
                let mut io = Mock::default();
                assert_eq!(apply_carrier(&mut io, 0, 1, duty, carrier), Ok(duty));
                assert_eq!(io.events[1], 1000 + carrier.ticks() * duty / 1000);
                io.events.clear();
                for step in 1..=6 {
                    assert_eq!(apply_carrier(&mut io, duty, step, duty, carrier), Ok(duty));
                }
                assert_eq!(io.events.len(), 30);
                assert!(io.events.iter().all(|&e| e < 1000));
                io.events.clear();
                assert_eq!(
                    apply_carrier(&mut io, duty, 1, maximum + 1, carrier),
                    Err(Refusal::Duty)
                );
                assert!(io.events.is_empty());
            }
        }
    }
}
