//! Opt-in sparse dispatch tail. No output authority; quiet until post-stop dump.
use super::*;
use portable_atomic::{AtomicU32, Ordering::Relaxed};
static GUARD: AtomicU32 = AtomicU32::new(0);
static EPOCH: AtomicU32 = AtomicU32::new(0);
pub fn epoch() -> u32 {
    EPOCH.load(Relaxed)
}
static mut TAIL: qualification_sparse::Tail = qualification_sparse::Tail::new(0);
pub fn guard_enter() {
    GUARD.store(GUARD.load(Relaxed).wrapping_add(1), Relaxed);
}
/// Caller masks interrupts and has quiesced the preceding observation producer.
pub fn reset() {
    let old = EPOCH.load(Relaxed);
    let next = old.checked_add(1).unwrap_or(0);
    // Keep the counter exhausted forever rather than wrapping0 back to1.
    EPOCH.store(old.saturating_add(1), Relaxed);
    unsafe {
        (&mut *core::ptr::addr_of_mut!(TAIL)).reset_epoch(next);
    }
}
pub struct Scope {
    entry: u16,
    accepted: u32,
    guard: u32,
    epoch: u32,
    step: u8,
    done: bool,
}
pub fn begin(entry: u16, accepted: u32, step: u8) -> Scope {
    Scope {
        entry,
        accepted,
        guard: GUARD.load(Relaxed),
        epoch: EPOCH.load(Relaxed),
        step,
        done: false,
    }
}
impl Scope {
    // Disabled check and live ISR must call this same emitted implementation.
    #[inline(never)]
    #[cfg_attr(
        feature = "bench-sparse-ram",
        unsafe(link_section = ".data.sparse_finish")
    )]
    pub fn finish(mut self, duration: u16, accepted: u32, stopped: bool) {
        if qualification_sparse::wanted(duration, stopped) {
            let guard = GUARD.load(Relaxed);
            let delta = accepted.wrapping_sub(self.accepted);
            unsafe {
                let t = &mut *core::ptr::addr_of_mut!(TAIL);
                if delta > 1 {
                    t.invalidate();
                } else {
                    t.push(
                        self.epoch,
                        qualification_sparse::Row {
                            accepted_before: self.accepted,
                            entry_tick: self.entry,
                            duration_us: duration,
                            step: self.step,
                            accepted: delta == 1,
                            guard_overlap: guard != self.guard,
                            stopped,
                        },
                    );
                }
            }
        }
        self.done = true;
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        if !self.done {
            unsafe {
                (&mut *core::ptr::addr_of_mut!(TAIL)).invalidate();
            }
        }
    }
}
pub fn dump<W: Write>(out: &mut W, capture: bool) {
    unsafe {
        let t = &mut *core::ptr::addr_of_mut!(TAIL);
        t.freeze_idle();
        let _ = writeln!(
            out,
            "QUALSPARSE epoch={} n={} omitted={} invalid={} frozen={} capture={} threshold_us={} dispatch_body=1 edge_time=0 normal_calls_counted=0",
            t.epoch(),
            t.len(),
            t.omitted(),
            t.invalid() as u8,
            t.frozen() as u8,
            capture as u8,
            qualification_sparse::THRESHOLD_US
        );
        if capture {
            for i in 0..t.len() {
                if let Some(r) = t.row(i) {
                    let _ = snapshot::record(
                        out,
                        "QS85",
                        &[
                            t.epoch() as u16,
                            (t.epoch() >> 16) as u16,
                            r.accepted_before as u16,
                            (r.accepted_before >> 16) as u16,
                            r.entry_tick,
                            r.duration_us,
                            r.step as u16,
                            r.accepted as u16,
                            r.guard_overlap as u16,
                            r.stopped as u16,
                        ],
                    );
                }
            }
        }
    }
}
/// Synthetic durations exercise actual instrumentation, never wait or drive.
#[inline(never)]
pub fn check<W: Write>(out: &mut W) {
    if !core_bench::bridge_disabled() || powered_timer::owns() || core_bench::active() {
        let _ = writeln!(out, "SPARSECHECK refused=1");
        return;
    }
    let mut baseline = 0;
    for _ in 0..16 {
        let start = t17();
        baseline = baseline.max(t17().wrapping_sub(start));
    }
    let _ = writeln!(
        out,
        "SPARSEBASE max_us={} samples=16 subtracted=0",
        baseline
    );
    for mode in 0..8 {
        let mut passed = 0;
        let mut max_us = 0;
        let preloaded = match mode {
            5 => 1,
            6 => 16,
            7 => 32,
            _ => 0,
        };
        // Opaque precomputed inputs prevent compiler speculation of synthetic
        // mode selection into the timed actual-Scope bracket.
        let duration = core::hint::black_box(if mode == 1 || mode == 2 || mode == 5 || mode == 6 {
            41
        } else {
            1
        });
        let accepted = core::hint::black_box(if mode == 1 || mode == 5 { 11 } else { 10 });
        let stopped = core::hint::black_box(mode == 3 || mode == 7);
        let overlap = core::hint::black_box(mode == 2 || mode == 6);
        let dropped = core::hint::black_box(mode == 4);
        for _ in 0..16 {
            cortex_m::interrupt::free(|_| reset());
            let prepared = unsafe {
                let t = &mut *core::ptr::addr_of_mut!(TAIL);
                let epoch = t.epoch();
                let mut ok = true;
                for i in 0..preloaded {
                    ok &= t.push(
                        epoch,
                        qualification_sparse::Row {
                            accepted_before: 9,
                            entry_tick: i as u16,
                            duration_us: 41,
                            step: 2,
                            accepted: false,
                            guard_overlap: false,
                            stopped: false,
                        },
                    );
                }
                ok && t.len() == preloaded.min(16)
                    && t.omitted() == preloaded.saturating_sub(16) as u32
            };
            let start = t17();
            let scope = begin(start, 10, 3);
            if overlap {
                guard_enter();
            }
            if dropped {
                drop(scope);
            } else {
                scope.finish(duration, accepted, stopped);
            }
            max_us = max_us.max(t17().wrapping_sub(start));
            let valid = unsafe {
                let t = &*core::ptr::addr_of!(TAIL);
                match mode {
                    0 => t.len() == 0 && !t.invalid() && !t.frozen(),
                    4 => t.invalid() && t.frozen(),
                    5..=7 => {
                        t.len() == (preloaded + 1).min(16)
                            && !t.invalid()
                            && t.omitted() == (preloaded + 1).saturating_sub(16) as u32
                            && t.frozen() == (mode == 7)
                            && t.row(t.len() - 1).is_some_and(|r| {
                                r.accepted_before == 10
                                    && r.step == 3
                                    && r.guard_overlap == (mode == 6)
                                    && r.accepted == (mode == 5)
                                    && r.stopped == (mode == 7)
                            })
                    }
                    _ => {
                        t.len() == 1
                            && !t.invalid()
                            && t.frozen() == (mode == 3)
                            && t.row(0).unwrap().guard_overlap == (mode == 2)
                            && t.row(0).unwrap().accepted == (mode == 1)
                    }
                }
            };
            if prepared && valid {
                passed += 1;
            }
        }
        let _ = writeln!(
            out,
            "SPARSECHECK mode={} passed={} total=16 max_us={} preloaded={} synthetic_duration=1 gate_authority=0",
            mode, passed, max_us, preloaded
        );
    }
    cortex_m::interrupt::free(|_| reset());
    let epoch = EPOCH.load(Relaxed);
    for i in 0..20 {
        let scope = begin(i as u16, i, 3);
        if i == 19 {
            guard_enter();
        }
        scope.finish(41, i + 1, false);
    }
    begin(20, 20, 4).finish(1, 20, true);
    let _ = writeln!(
        out,
        "SPARSEWIRE expected_epoch={} synthetic=1 expected_n=16 expected_omitted=5",
        epoch
    );
    dump(out, true);
    cortex_m::interrupt::free(|_| reset());
    let _ = writeln!(
        out,
        "SPARSECHECK END disabled={}",
        core_bench::bridge_disabled() as u8
    );
}
