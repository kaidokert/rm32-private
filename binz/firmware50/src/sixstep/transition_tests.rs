//! Settled digital output requests, not a timing/analog simulation.
use super::*;

fn outputs(p: &Plan, pwm_high: bool) -> [(bool, bool); 3] {
    core::array::from_fn(|ch| {
        let mode = if ch == 2 { p.ccmr2 } else { p.ccmr1 >> (ch * 8) } & 0x70;
        let reference = mode == 0x60 && pwm_high;
        let e = (p.ccer >> (ch * 4)) & 5;
        assert!(e == 1 || e == 5, "model covers the actual E1/NE0-or1 plans");
        (reference, e == 5 && !reference)
    })
}

#[test]
fn immediate_role_writes_have_states_outside_old_and_new_sectors() {
    let mut intermediate_lows = 0;
    let mut double_highs = 0;
    for reverse in [false, true] {
        for n in 1..=6 {
            let next = if reverse { if n == 1 { 6 } else { n - 1 } }
                else if n == 6 { 1 } else { n + 1 };
            let old = plan(Step::new_clamped(n), 550, 1333, 800).unwrap();
            let new = plan(Step::new_clamped(next), 550, 1333, 800).unwrap();
            for level in [false, true] {
                let before = outputs(&old, level);
                let after = outputs(&new, level);
                let mut interim = old;
                for write in 0..3 {
                    match write {
                        0 => interim.ccmr1 = new.ccmr1,
                        1 => interim.ccmr2 = new.ccmr2,
                        _ => interim.ccer = new.ccer,
                    }
                    let pins = outputs(&interim, level);
                    for ch in 0..3 {
                        if pins[ch].1 && !before[ch].1 && !after[ch].1 {
                            intermediate_lows += 1;
                        }
                    }
                    if pins.iter().filter(|p| p.0).count() > 1 { double_highs += 1; }
                    assert!(pins.iter().all(|p| !(p.0 && p.1)), "no same-leg shoot-through asserted");
                }
                assert_eq!(outputs(&interim, level), after);
            }
        }
    }
    std::println!("settled-state observations: intermediate_lows={intermediate_lows}, double_highs={double_highs}");
    assert!(intermediate_lows > 0);
    assert!(double_highs > 0);
}

#[test]
fn single_role_latch_has_no_torn_sector_even_if_staging_is_interrupted() {
    for n in 1..=6 {
        for next in 1..=6 {
            let old = plan(Step::new_clamped(n), 550, 1333, 800).unwrap();
            let new = plan(Step::new_clamped(next), 550, 1333, 800).unwrap();
            for level in [false, true] {
                let mut active = old;
                let mut preload = old;
                preload.ccmr1 = new.ccmr1;
                assert_eq!(outputs(&active, level), outputs(&old, level));
                preload.ccmr2 = new.ccmr2;
                assert_eq!(outputs(&active, level), outputs(&old, level));
                preload.ccer = new.ccer;
                assert_eq!(outputs(&active, level), outputs(&old, level));
                active = preload; // COMG; no UG, no counter restart.
                assert_eq!(outputs(&active, level), outputs(&new, level));
            }
        }
    }
}

// E416 feasibility only: no production caller or binary selects this plan.
fn diode_plan(mut p: Plan) -> Plan {
    p.ccer &= !(4 << (slot(p.source) * 4));
    p
}

#[test]
fn diode_plan_changes_only_source_complement_enable() {
    for n in 1..=6 {
        for duty in [0, 100, 500, 800] {
            let p = plan(Step::new_clamped(n), duty, 1333, 800).unwrap();
            let d = diode_plan(p);
            assert_eq!(p.ccer ^ d.ccer, 4 << (slot(p.source) * 4));
            assert_eq!((p.ccmr1, p.ccmr2, p.ccr), (d.ccmr1, d.ccmr2, d.ccr));
            for level in [false, true] {
                let pins = outputs(&d, level);
                assert_eq!(pins[slot(p.source)], (level, false));
                assert_eq!(pins[slot(p.sink)], (false, true));
                assert_eq!(pins[slot(p.floating)], (false, false));
            }
        }
    }
}

#[test]
fn diode_mode_does_not_make_immediate_role_writes_atomic() {
    let mut unintended_low_observations = 0;
    let mut two_high_observations = 0;
    for reverse in [false, true] {
        for n in 1..=6 {
            let next = if reverse { if n == 1 { 6 } else { n - 1 } }
                else if n == 6 { 1 } else { n + 1 };
            let old = diode_plan(plan(Step::new_clamped(n), 550, 1333, 800).unwrap());
            let new = diode_plan(plan(Step::new_clamped(next), 550, 1333, 800).unwrap());
            for level in [false, true] {
                let before = outputs(&old, level);
                let after = outputs(&new, level);
                let mut interim = old;
                for write in 0..3 {
                    match write {
                        0 => interim.ccmr1 = new.ccmr1,
                        1 => interim.ccmr2 = new.ccmr2,
                        _ => interim.ccer = new.ccer,
                    }
                    let pins = outputs(&interim, level);
                    for ch in 0..3 {
                        if pins[ch].1 && !before[ch].1 && !after[ch].1 {
                            unintended_low_observations += 1;
                        }
                    }
                    if pins.iter().filter(|p| p.0).count() > 1 {
                        two_high_observations += 1;
                    }
                }
                assert_eq!(outputs(&interim, level), after);
            }
        }
    }
    std::println!("diode settled-state observations: unintended_lows={unintended_low_observations}, two_highs={two_high_observations}");
    assert!(two_high_observations > 0, "source-low-off does not fix role ordering");
}
