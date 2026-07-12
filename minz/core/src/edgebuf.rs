//! EDGE_BUF rev-pair bookkeeping — the half-flip state machine the
//! TIM7 stepper runs at sector boundaries. The buffers themselves
//! (4096-bin halves, per-sector counters, boundary ticks) stay in
//! the firmware; this owns the transitions: which half is active,
//! when a pair completes, where a boundary tick belongs.
//!
//! Concurrency contract: every function here MUST run inside the
//! caller's critical section (the firmware wraps the whole flip +
//! boundary block in `free`) — the COMP ISR reads the
//! (ACTIVE_HALF, HALF_START_TICK, REV_PHASE) tuple together and a
//! torn snapshot would file edges into the wrong half/slot.

use portable_atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

pub struct EdgeHalves<'a> {
    /// 0/1: which rev of the current pair is in progress.
    pub rev_phase: &'a AtomicU8,
    /// 0/1: which half of EDGE_BUF is being written.
    pub active_half: &'a AtomicU8,
    /// 10 µs tick at which the active pair started.
    pub half_start_tick: &'a AtomicU32,
    /// `E` key: freeze — suppress flips so the frozen half survives
    /// re-dumps (motor keeps running).
    pub freeze: &'a AtomicBool,
}

/// What the firmware must do after a flip: clear the new active
/// half's bins + per-sector counters, finalize slot 0 of the frozen
/// half to `old_half_start`, and initialize slot 0 of the new half
/// to `now` (together these keep slot 0 valid for every completed
/// pair without depending on a sector-0 transition being recorded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlipPlan {
    pub old_active: usize,
    pub new_active: usize,
    pub old_half_start: u32,
}

/// Called at every rev wrap (commutation sector 5→0). XORs the rev
/// phase unconditionally; on the second rev of a pair (and not
/// frozen) rotates the halves — the atomics are updated here, the
/// buffer work is returned as a [`FlipPlan`].
pub fn on_rev_wrap(eh: &EdgeHalves<'_>, now_10us: u32) -> Option<FlipPlan> {
    let prev_phase = eh.rev_phase.fetch_xor(1, Ordering::Relaxed);
    if prev_phase == 1 && !eh.freeze.load(Ordering::Relaxed) {
        let old_active = eh.active_half.load(Ordering::Relaxed) as usize;
        let new_active = old_active ^ 1;
        let old_half_start = eh.half_start_tick.load(Ordering::Relaxed);
        eh.half_start_tick.store(now_10us, Ordering::Relaxed);
        eh.active_half.store(new_active as u8, Ordering::Relaxed);
        Some(FlipPlan {
            old_active,
            new_active,
            old_half_start,
        })
    } else {
        None
    }
}

/// Boundary-tick slot for a sector entry: `rev_phase*6 + sector`,
/// `None` out of range. For non-flip rev wraps that's slot 6 of the
/// old active half (rev 1 sec 0); for flip wraps slot 0 of the new
/// half; for mid-rev changes `phase*6+sector` of the current half.
pub fn boundary_slot(rev_phase: u8, sector: u8) -> Option<usize> {
    let slot = rev_phase as usize * 6 + sector as usize;
    (slot < 12).then_some(slot)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rig {
        rev_phase: AtomicU8,
        active_half: AtomicU8,
        half_start_tick: AtomicU32,
        freeze: AtomicBool,
    }

    impl Rig {
        fn new() -> Self {
            Self {
                rev_phase: AtomicU8::new(0),
                active_half: AtomicU8::new(0),
                half_start_tick: AtomicU32::new(100),
                freeze: AtomicBool::new(false),
            }
        }

        fn eh(&self) -> EdgeHalves<'_> {
            EdgeHalves {
                rev_phase: &self.rev_phase,
                active_half: &self.active_half,
                half_start_tick: &self.half_start_tick,
                freeze: &self.freeze,
            }
        }
    }

    #[test]
    fn flips_every_second_wrap_and_rotates_halves() {
        let r = Rig::new();
        // Wrap 1: first rev of the pair done — no flip, phase 0→1.
        assert_eq!(on_rev_wrap(&r.eh(), 500), None);
        assert_eq!(r.rev_phase.load(Ordering::Relaxed), 1);
        // Wrap 2: pair complete — flip 0→1, start tick updated.
        let plan = on_rev_wrap(&r.eh(), 900).unwrap();
        assert_eq!(
            plan,
            FlipPlan {
                old_active: 0,
                new_active: 1,
                old_half_start: 100
            }
        );
        assert_eq!(r.active_half.load(Ordering::Relaxed), 1);
        assert_eq!(r.half_start_tick.load(Ordering::Relaxed), 900);
        assert_eq!(r.rev_phase.load(Ordering::Relaxed), 0);
        // Next pair flips back 1→0.
        assert_eq!(on_rev_wrap(&r.eh(), 1300), None);
        let plan = on_rev_wrap(&r.eh(), 1700).unwrap();
        assert_eq!(plan.old_active, 1);
        assert_eq!(plan.new_active, 0);
        assert_eq!(plan.old_half_start, 900);
    }

    #[test]
    fn freeze_suppresses_the_flip_but_phase_keeps_counting() {
        // `E`-key semantics: the frozen half must stay intact across
        // any number of wraps; on thaw the pair cadence resumes from
        // the phase parity (matches the shipped behavior).
        let r = Rig::new();
        on_rev_wrap(&r.eh(), 500);
        r.freeze.store(true, Ordering::Relaxed);
        assert_eq!(on_rev_wrap(&r.eh(), 900), None, "flip suppressed");
        assert_eq!(r.active_half.load(Ordering::Relaxed), 0, "half untouched");
        assert_eq!(r.half_start_tick.load(Ordering::Relaxed), 100);
        on_rev_wrap(&r.eh(), 1300);
        r.freeze.store(false, Ordering::Relaxed);
        assert!(on_rev_wrap(&r.eh(), 1700).is_some(), "thaw resumes");
    }

    #[test]
    fn boundary_slots() {
        assert_eq!(boundary_slot(0, 0), Some(0));
        assert_eq!(boundary_slot(0, 5), Some(5));
        assert_eq!(boundary_slot(1, 0), Some(6)); // non-flip rev wrap
        assert_eq!(boundary_slot(1, 5), Some(11));
        assert_eq!(boundary_slot(2, 0), None); // corrupt phase: refuse
    }
}
