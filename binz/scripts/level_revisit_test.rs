#[path = "../examples/support/level_revisit.rs"]
mod level_revisit;

use level_revisit::{Inputs, admit};

fn valid() -> Inputs {
    Inputs {
        owner: true,
        active: true,
        coast_reference: true,
        software_masked: false,
        hardware_enabled: true,
        average_half_us: 216,
        interval_half_us: 109,
        pending: false,
        post_level: true,
        same_step_retried: false,
        allow_high_speed: true,
    }
}

#[test]
fn admits_one_high_speed_post_level_retry_after_gate() {
    assert!(admit(valid()));
}

#[test]
fn preserves_low_speed_only_policy_without_new_feature() {
    let mut v = valid();
    v.allow_high_speed = false;
    assert!(!admit(v));
    v.average_half_us = 1000;
    v.interval_half_us = 501;
    assert!(admit(v));
}

#[test]
fn gate_is_strict_and_average_must_be_initialized() {
    let mut v = valid();
    v.interval_half_us = 108;
    assert!(!admit(v));
    v.interval_half_us = 109;
    v.average_half_us = 63;
    assert!(!admit(v));
}

#[test]
fn every_authority_and_signal_veto_is_fail_closed() {
    for index in 0..8 {
        let mut v = valid();
        match index {
            0 => v.owner = false,
            1 => v.active = false,
            2 => v.coast_reference = false,
            3 => v.software_masked = true,
            4 => v.hardware_enabled = false,
            5 => v.pending = true,
            6 => v.post_level = false,
            7 => v.same_step_retried = true,
            _ => unreachable!(),
        }
        assert!(!admit(v), "veto {index}");
    }
}
