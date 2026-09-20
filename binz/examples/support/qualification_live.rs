//! Optional completed-dispatch-body summaries. Not physical edge timestamps.
use super::*;
use portable_atomic::{AtomicU32, Ordering::Relaxed};
static GUARD_SEQUENCE: AtomicU32 = AtomicU32::new(0);
static mut HISTORY: qualification_window::History = qualification_window::History::new();
pub fn guard_enter() {
    GUARD_SEQUENCE.store(GUARD_SEQUENCE.load(Relaxed).wrapping_add(1), Relaxed);
}
pub fn reset() {
    unsafe {
        core::ptr::addr_of_mut!(HISTORY).write(qualification_window::History::new());
    }
}
pub struct Scope {
    start: u16,
    guard: u32,
    accepted: u32,
    step: u8,
    done: bool,
}
pub fn begin(accepted: u32, step: u8) -> Scope {
    Scope {
        start: t17(),
        guard: GUARD_SEQUENCE.load(Relaxed),
        accepted,
        step,
        done: false,
    }
}
impl Scope {
    pub fn finish(mut self, accepted: u32, stopped: bool) {
        let end = t17();
        let guard = GUARD_SEQUENCE.load(Relaxed);
        unsafe {
            (&mut *core::ptr::addr_of_mut!(HISTORY)).complete(
                end.wrapping_sub(self.start),
                self.guard,
                guard,
                self.step,
                accepted.wrapping_sub(self.accepted),
                stopped,
            );
        }
        self.done = true;
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        // Unexpected earlyreturn cannot masquerade as a fully observed call.
        if !self.done {
            unsafe {
                (&mut *core::ptr::addr_of_mut!(HISTORY)).complete(0, 0, 0, 0, 2, true);
            }
        }
    }
}
pub fn dump<W: Write>(out: &mut W, capture: bool) {
    unsafe {
        let h = &mut *core::ptr::addr_of_mut!(HISTORY);
        h.freeze_idle();
        let p = h.pending();
        let _ = writeln!(
            out,
            "QUALWINDOW n={} omitted={} invalid={} frozen={} capture={} dispatch_body=1 edge_time=0",
            h.len(),
            h.omitted(),
            h.invalid() as u8,
            h.frozen() as u8,
            capture as u8
        );
        if capture {
            for i in 0..h.len() {
                if let Some(r) = h.row(i) {
                    let _ = snapshot::record(
                        out,
                        "QW85",
                        &[
                            r.accepted_total as u16,
                            (r.accepted_total >> 16) as u16,
                            r.step as u16,
                            r.accepted as u16,
                            r.stopped as u16,
                            r.window.calls,
                            r.window.guard_overlap_calls,
                            r.window.max_call_us,
                            r.window.saturated as u16,
                        ],
                    );
                }
            }
            let _ = snapshot::record(
                out,
                "QP85",
                &[
                    p.calls,
                    p.guard_overlap_calls,
                    p.max_call_us,
                    p.saturated as u16,
                ],
            );
        }
    }
}

/// Disabled-only actual Scope exercise; not a whole-ISR WCET measurement.
pub fn check<W: Write>(out: &mut W) {
    if !core_bench::bridge_disabled() || powered_timer::owns() || core_bench::active() {
        let _ = writeln!(out, "QUALCHECK refused=1");
        return;
    }
    for mode in 0..5 {
        let mut passed = 0;
        let mut max_us = 0;
        for _ in 0..16 {
            reset();
            let start = t17();
            let scope = begin(10, 3);
            if mode == 1 {
                guard_enter();
            }
            if mode == 4 {
                drop(scope);
            } else {
                scope.finish(if mode == 0 || mode == 3 { 11 } else { 10 }, mode >= 2);
            }
            max_us = max_us.max(t17().wrapping_sub(start));
            let valid = unsafe {
                let h = &*core::ptr::addr_of!(HISTORY);
                if mode == 4 {
                    h.invalid() && h.frozen()
                } else if mode == 1 {
                    h.len() == 0 && h.pending().guard_overlap_calls == 1
                } else {
                    h.len() == 1
                        && !h.invalid()
                        && h.frozen() == (mode >= 2)
                        && h.row(0).unwrap().accepted == (mode == 0 || mode == 3)
                        && h.row(0).unwrap().window.calls == 1
                }
            };
            if valid {
                passed += 1;
            }
        }
        let _ = writeln!(
            out,
            "QUALCHECK mode={} passed={} total=16 max_us={} scope_only=1 gate_authority=0",
            mode, passed, max_us
        );
    }
    reset();
    let _ = writeln!(
        out,
        "QUALCHECK END disabled={}",
        core_bench::bridge_disabled() as u8
    );
}
