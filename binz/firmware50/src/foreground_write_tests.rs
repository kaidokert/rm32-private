//! CPU-guard interleaving model, not peripheral timing or NMI certification.
use super::arm_allowed;

#[derive(Default)]
struct State { latched: bool, active: bool, moe: bool, compares: u32 }
impl State {
    fn stop(&mut self) { self.latched = true; self.active = false; self.moe = false; self.compares = 0; }
    fn write(&mut self) { self.moe = true; self.compares = 133; }
    fn off(&self) -> bool { !self.moe && self.compares == 0 }
}

#[test]
fn atomic_foreground_write_respects_stop_on_either_side() {
    for stop_before in [false, true] {
        let mut s = State { active: true, ..State::default() };
        if stop_before { s.stop(); }
        if arm_allowed(s.latched, s.active) { s.write(); }
        if !stop_before { assert!(s.moe && s.compares == 133); }
        if !stop_before { s.stop(); }
        assert!(s.off());
    }
    let mut inactive = State::default();
    if arm_allowed(inactive.latched, inactive.active) { inactive.write(); }
    assert!(inactive.off());
}

#[test]
fn stale_precheck_negative_control_reproduces_shutdown_overwrite() {
    let mut s = State { active: true, ..State::default() };
    let admitted = arm_allowed(s.latched, s.active);
    s.stop();
    if admitted { s.write(); }
    assert!(!s.off(), "the unprotected implementation must fail this model");
}

#[test]
fn target_wrappers_bind_model_to_admission_inside_exclusion() {
    let board = include_str!("../bin/board.rs");
    let helper = board.split("fn powered_write(").nth(1).unwrap().split("impl Hal for Board").next().unwrap();
    let masked = helper.find("cortex_m::interrupt::free").unwrap();
    let decision = helper.find("firmware50::oneshot::arm_allowed").unwrap();
    let write = helper.find("write();").unwrap();
    assert!(masked < decision && decision < write);
    assert!(helper.contains("cortex_m::interrupt::free(|_| {\n        if firmware50::oneshot::arm_allowed(\n            roots::guard_latched(), S.guard().active.load(Ordering::Relaxed),\n        ) {\n            write();\n        }\n    });"));
    assert!(helper.contains("roots::guard_latched(), S.guard().active.load(Ordering::Relaxed)"));
    assert!(board.contains("powered_write(|| hw::pwm::apply_plan(plan));"));
    assert!(board.contains("powered_write(hw::pwm::moe_on);"));
    assert!(board.contains("powered_write(|| roots::set_compares_wired(logical));"));
    assert!(board.contains("if logical == [0; 3]"));
    // Structural binding is not proof of compiled critical-section semantics.
}

#[test]
fn startup_zero_then_rearm_can_write_after_previous_stop() {
    let mut s = State { active: true, ..State::default() };
    s.stop();
    assert_eq!(crate::sine::compares(crate::duty::STARTUP_TICKS - 1, 0, 0), [0; 3]);
    // Mirrors guard_arm's reset, not a target interrupt simulation.
    s.latched = false;
    s.active = true;
    if arm_allowed(s.latched, s.active) { s.write(); }
    assert!(s.moe && s.compares == 133);
    s.stop();
    assert!(s.off());
}
