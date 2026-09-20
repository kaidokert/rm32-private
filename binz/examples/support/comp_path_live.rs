//! Optional dispatched-COMP-only aggregate counts. No per-comparator-read work.
//! COMP cannot nest itself; lower-priority COM cannot preempt it. Foreground
//! reset/dump happens outside active sensing. Guard IRQs do not access STATE.
use super::*;
struct State {
    active: bool,
    first: u32,
    bins: [u32; comp_paths::N],
}
static mut STATE: State = State {
    active: false,
    first: u32::MAX,
    bins: [0; comp_paths::N],
};
#[cfg(feature = "bench-comp-decisions")]
static mut TAIL: comp_decision_tail::Tail = comp_decision_tail::Tail::new();
#[cfg(feature = "bench-qualification-event")]
static mut EVENTS: qualification_event::History = qualification_event::History::new();
#[cfg(feature = "bench-qualification-event")]
static mut EVENT_EPOCH: u32 = 0;
pub fn reset() {
    unsafe {
        core::ptr::addr_of_mut!(STATE).write(State {
            active: false,
            first: u32::MAX,
            bins: [0; comp_paths::N],
        });
        #[cfg(feature = "bench-comp-decisions")]
        core::ptr::addr_of_mut!(TAIL).write(comp_decision_tail::Tail::new());
        #[cfg(feature = "bench-qualification-event")]
        {
            // Exhaustion stays invalid until reboot, never aliases an old epoch.
            EVENT_EPOCH = if EVENT_EPOCH == u32::MAX {
                u32::MAX
            } else {
                EVENT_EPOCH + 1
            };
            (&mut *core::ptr::addr_of_mut!(EVENTS)).reset(if EVENT_EPOCH == u32::MAX {
                0
            } else {
                EVENT_EPOCH
            });
        }
    }
}
pub struct Scope {
    average: u32,
    accepted: u32,
    #[cfg(feature = "bench-comp-decisions")]
    context: Option<(u32, u8)>,
    #[cfg(feature = "bench-qualification-event")]
    event_step: Option<u8>,
    #[cfg(any(
        feature = "bench-comp-decisions",
        feature = "bench-qualification-event"
    ))]
    completed: bool,
}
pub fn begin(average: u32, accepted: u32) -> Scope {
    unsafe {
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        s.active = true;
        s.first = u32::MAX;
        Scope {
            average,
            accepted,
            #[cfg(feature = "bench-comp-decisions")]
            context: None,
            #[cfg(feature = "bench-qualification-event")]
            event_step: None,
            #[cfg(any(
                feature = "bench-comp-decisions",
                feature = "bench-qualification-event"
            ))]
            completed: false,
        }
    }
}
#[inline]
pub fn first_count(value: u32) {
    unsafe {
        let s = &mut *core::ptr::addr_of_mut!(STATE);
        if s.active && s.first == u32::MAX {
            s.first = value;
        }
    }
}
impl Scope {
    #[cfg(feature = "bench-qualification-event")]
    pub fn with_event_context(mut self, powered: bool, step: u8) -> Self {
        self.event_step = if powered { Some(step) } else { None };
        self
    }
    #[cfg(feature = "bench-comp-decisions")]
    pub fn with_context(mut self, us: u32, step: u8) -> Self {
        self.context = Some((us, step));
        self
    }
    pub fn finish(mut self, accepted: u32, stopped: bool) {
        self.complete(accepted, stopped);
    }
    fn complete(&mut self, accepted: u32, stopped: bool) {
        unsafe {
            let s = &mut *core::ptr::addr_of_mut!(STATE);
            s.active = false;
            let first = if s.first == u32::MAX {
                None
            } else {
                Some(s.first)
            };
            let p = comp_paths::classify(
                first,
                self.average,
                accepted.wrapping_sub(self.accepted),
                stopped,
            );
            s.bins[p as usize] = s.bins[p as usize].saturating_add(1);
            #[cfg(any(
                feature = "bench-comp-decisions",
                feature = "bench-qualification-event"
            ))]
            {
                self.completed = true;
            }
            #[cfg(feature = "bench-qualification-event")]
            if let Some(step) = self.event_step {
                let history = &mut *core::ptr::addr_of_mut!(EVENTS);
                let open = matches!(
                    p,
                    comp_paths::Path::OpenNoAccept | comp_paths::Path::Accepted
                );
                if open && first.is_none_or(|value| value > u16::MAX as u32) {
                    history.invalidate();
                } else {
                    history.visit(
                        self.accepted,
                        step,
                        p,
                        if open { first.map(|v| v as u16) } else { None },
                    );
                }
            }
            #[cfg(feature = "bench-comp-decisions")]
            {
                self.completed = true;
                if let Some((us, step)) = self.context {
                    let _ = (&mut *core::ptr::addr_of_mut!(TAIL)).push(
                        us,
                        step,
                        first,
                        self.average,
                        accepted.wrapping_sub(self.accepted),
                        stopped,
                    );
                }
            }
        }
    }
}
#[cfg(any(
    feature = "bench-comp-decisions",
    feature = "bench-qualification-event"
))]
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.completed {
            self.complete(self.accepted, true);
        }
    }
}
pub fn dump<W: Write>(out: &mut W, _capture: bool, _final_accepted: u32) {
    unsafe {
        let s = &*core::ptr::addr_of!(STATE);
        let _ = writeln!(
            out,
            "COMPPATH scope=dispatched first_actual_count=1 no_gate={} closed={} open_no_accept={} accepted={} stopped_or_unknown={} active={}",
            s.bins[0], s.bins[1], s.bins[2], s.bins[3], s.bins[4], s.active as u8
        );
        #[cfg(feature = "bench-qualification-event")]
        if _capture {
            let h = &mut *core::ptr::addr_of_mut!(EVENTS);
            h.freeze();
            let _ = writeln!(
                out,
                "QUALEVENT epoch={} n={} total={} omitted={} invalid={} frozen={} final_accepted={} dispatched_only=1 wire=qe85-v1",
                h.epoch(),
                h.len(),
                h.total(),
                h.omitted(),
                h.invalid() as u8,
                h.frozen() as u8,
                _final_accepted
            );
            for index in 0..=h.len() {
                // Last frame is explicitly partial, even if empty or duplicating
                // the terminal stopped bucket. Never count it as another event.
                let partial = index == h.len();
                let row = if partial {
                    h.partial()
                } else {
                    h.row(index).unwrap()
                };
                let mut words = [0u16; 13];
                words[0] = h.epoch() as u16;
                words[1] = (h.epoch() >> 16) as u16;
                words[2] = row.accepted_before as u16;
                words[3] = (row.accepted_before >> 16) as u16;
                words[4..9].copy_from_slice(&row.counts);
                words[9] = row.first_open;
                words[10] = row.last_open;
                words[11] =
                    row.step as u16 | ((row.has_open as u16) << 8) | ((row.stopped as u16) << 9);
                words[12] = partial as u16;
                let _ = snapshot::record(out, "QE85", &words);
            }
        }
        #[cfg(feature = "bench-comp-decisions")]
        {
            let t = &mut *core::ptr::addr_of_mut!(TAIL);
            t.freeze(); // dump is post-stop; do not manufacture an extra decision
            let _ = writeln!(
                out,
                "COMPDECISION n={} total={} omitted={} invalid={} frozen={} active={} capture={} entry_not_edge=1 wire=d85-v1",
                t.len(),
                t.total(),
                t.omitted(),
                t.invalid() as u8,
                t.frozen() as u8,
                s.active as u8,
                _capture as u8
            );
            if _capture {
                for i in 0..t.len() {
                    if let Some(row) = t.row(i) {
                        let mut words = [0u16; 8];
                        for j in 0..4 {
                            words[2 * j] = row.0[j] as u16;
                            words[2 * j + 1] = (row.0[j] >> 16) as u16;
                        }
                        let _ = snapshot::record(out, "CD85", &words);
                    }
                }
            }
        }
    }
}

/// Caller must establish disabled, inactive sensing. Exercises actual Scope,
/// including Drop, not the reference comparator or an ISR scheduling budget.
#[cfg(feature = "bench-comp-decisions")]
pub fn check<W: Write>(out: &mut W) {
    for mode in 0..5 {
        let mut passed = 0;
        let mut maximum = 0;
        for _ in 0..16 {
            reset();
            let began = t17();
            let scope = begin(core::hint::black_box(1000), 10).with_context(100, 2);
            first_count(core::hint::black_box(if mode == 0 { 500 } else { 501 }));
            // Only the first actual count belongs to the gate decision.
            first_count(999);
            if mode == 4 {
                drop(scope);
            } else {
                scope.finish(if mode == 2 { 11 } else { 10 }, mode == 3);
            }
            maximum = maximum.max(t17().wrapping_sub(began) as u32);
            let valid = unsafe {
                let s = &*core::ptr::addr_of!(STATE);
                let t = &*core::ptr::addr_of!(TAIL);
                let path = if mode >= 3 { 4 } else { mode + 1 };
                let row = t.row(0).unwrap();
                !s.active
                    && t.total() == 1
                    && !t.invalid()
                    && t.frozen() == (mode >= 3)
                    && ((row.0[3] >> 8) & 255) == path
                    && (row.0[2] & 65535) == if mode == 0 { 500 } else { 501 }
                    && s.bins[path as usize] == 1
            };
            if valid {
                passed += 1;
            }
        }
        let _ = writeln!(
            out,
            "DECISIONCHECK mode={} passed={} total=16 max_us={} scope_only=1 gate_authority=0",
            mode, passed, maximum
        );
    }
    reset();
}

/// Idle-only synthetic Scope cost, not reference filtering or powered WCET.
#[cfg(feature = "bench-qualification-event")]
#[inline(never)]
pub fn event_check<W: Write>(out: &mut W) {
    if core_bench::active()
        || powered_timer::owns()
        || driven_run::owns()
        || !core_bench::bridge_disabled()
    {
        let _ = writeln!(out, "QEVENTCHECK refused=1");
        return;
    }
    for preload in [0u32, 16, 32] {
        for mode in 0..6 {
            let mut passed = 0;
            let mut maximum = 0;
            for _ in 0..16 {
                reset();
                unsafe {
                    for n in 0..preload {
                        (&mut *core::ptr::addr_of_mut!(EVENTS)).visit(
                            n,
                            (n % 6 + 1) as u8,
                            comp_paths::Path::Accepted,
                            Some(900),
                        );
                    }
                }
                let step = core::hint::black_box((preload % 6 + 1) as u8);
                let average = core::hint::black_box(1000);
                let count = core::hint::black_box(if mode == 0 { 500 } else { 501 });
                let after = core::hint::black_box(preload + if mode == 2 { 1 } else { 0 });
                let began = t17();
                let scope = begin(average, preload).with_event_context(true, step);
                if mode != 5 {
                    first_count(count);
                    first_count(999);
                }
                if mode == 4 {
                    drop(scope);
                } else {
                    scope.finish(after, mode == 3);
                }
                maximum = maximum.max(t17().wrapping_sub(began) as u32);
                let valid = unsafe {
                    let h = &*core::ptr::addr_of!(EVENTS);
                    let terminal = (2..=4).contains(&mode);
                    let expected_total = preload + terminal as u32;
                    let r = if terminal {
                        h.row(h.len() - 1).unwrap()
                    } else {
                        h.partial()
                    };
                    let path = match mode {
                        0 => 1,
                        1 => 2,
                        2 => 3,
                        3 | 4 => 4,
                        _ => 0,
                    };
                    let mut counts = [0; 5];
                    counts[path] = 1;
                    !(*core::ptr::addr_of!(STATE)).active
                        && !h.invalid()
                        && h.total() == expected_total
                        && h.omitted() == expected_total.saturating_sub(16)
                        && h.frozen() == (mode == 3 || mode == 4)
                        && r.accepted_before == preload
                        && r.step == step
                        && r.counts == counts
                        && r.has_open == (mode == 1 || mode == 2)
                        && (!r.has_open || (r.first_open == 501 && r.last_open == 501))
                };
                if valid {
                    passed += 1;
                }
            }
            let _ = writeln!(
                out,
                "QEVENTCHECK preload={} mode={} passed={} total=16 max_us={} scope_only=1 gate_authority=0",
                preload, mode, passed, maximum
            );
        }
    }
    reset();
    let _ = writeln!(
        out,
        "QEVENTCHECK END disabled={}",
        core_bench::bridge_disabled() as u8
    );
}
