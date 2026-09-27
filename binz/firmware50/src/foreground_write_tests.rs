//! CPU interrupt ordering model; not a hardware response-time proof.
use super::arm_allowed;

#[derive(Default)]
struct State { stopped: bool, active: bool, output: bool }
impl State {
    fn stop(&mut self) { self.stopped = true; self.active = false; self.output = false; }
    fn write(&mut self) {
        if arm_allowed(self.stopped, self.active) { self.output = true; }
    }
}

#[test]
fn stop_dominates_before_and_after_write() {
    for before in [false, true] {
        let mut s = State { active: true, ..State::default() };
        if before { s.stop(); }
        s.write();
        if !before { assert!(s.output); s.stop(); }
        assert!(!s.output);
    }
    let mut s = State::default();
    s.write();
    assert!(!s.output);
    s.stop();
    s.stopped = false;
    s.active = true;
    s.write();
    assert!(s.output, "explicit fresh arm permits subsequent writes");
}

#[test]
fn stale_check_negative_control_overwrites_stop() {
    let mut s = State { active: true, ..State::default() };
    let allowed = arm_allowed(s.stopped, s.active);
    s.stop();
    if allowed { s.output = true; }
    assert!(s.output);
}

#[test]
fn wrappers_bind_to_guarded_write() {
    let board = include_str!("../bin/board.rs");
    assert!(board.contains("powered_write(hw::pwm::moe_on);"));
    assert!(board.contains("powered_write(|| hw::pwm::apply_plan(plan));"));
    assert!(board.contains("powered_write(|| roots::set_compares_wired(logical));"));
    assert!(board.contains("if logical == [0; 3]"));
    let body = board.split("fn powered_write").nth(1).unwrap()
        .split("impl Hal for Board").next().unwrap();
    assert!(body.contains("cortex_m::interrupt::free(|_| {"));
    assert!(body.contains("roots::guard_latched(), S.guard().active.load(Ordering::Relaxed)"));
    // Source binding complements inspection of the actual optimized mask span.
}
