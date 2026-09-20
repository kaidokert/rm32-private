//! Counterexamples to two tempting timestamp adapters. Uses actual minz-core.
//! These prove non-equivalence, not that captured-event control is impossible.
mod support;
use support::*;
use minz_core::am32_isr;
use portable_atomic::Ordering::Relaxed;

fn setup(interval:u32,levels:Vec<bool>)->(SchedStore,DriveStore,MockHal) {
    let s=SchedStore::default();let d=DriveStore::default();let h=MockHal::new();
    s.average_interval.store(1200,Relaxed);s.wait_time.store(300,Relaxed);
    d.rising.store(true,Relaxed);d.current_step.store(1,Relaxed);
    d.filter_level.store(12,Relaxed);
    h.pending.set(true);h.comp_enabled.set(true);h.interval.set(interval);
    h.comp_value.set(true);h.comp_seq.replace(levels);(s,d,h)
}
fn run(s:&SchedStore,d:&DriveStore,h:&MockHal) {
    am32_isr::comp_isr(&s.sched(),&d.drive(),&mut h.motor(),&h.observer());
}

#[test]
fn captured_count_cannot_replace_live_count_at_strict_gate() {
    // Edge arrived at590; dispatch at601. Gate opens strictly AFTER600.
    let (s,d,h)=setup(601,vec![true;12]);run(&s,&d,&h);
    assert_eq!(*h.com_arrs.borrow(),vec![301]);
    let (s,d,h)=setup(590,vec![true;12]);
    for _ in 0..4 {run(&s,&d,&h);}
    assert!(h.pending.get());assert!(h.com_arrs.borrow().is_empty());
    // Returning a frozen capture value cannot let retained pending age open.
}

#[test]
fn holding_captured_level_can_accept_a_real_persistence_failure() {
    // E338 shows initially-correct reads that reverse later in the loop.
    let mut actual=vec![true;11];actual.push(false);
    let (s,d,h)=setup(900,actual);run(&s,&d,&h);
    assert!(!h.pending.get());assert!(h.com_arrs.borrow().is_empty());
    let (s,d,h)=setup(900,vec![true;12]);run(&s,&d,&h);
    assert_eq!(*h.com_arrs.borrow(),vec![301]);
    // A qualified earlier edge is not twelve fresh observations of now.
}

#[test]
fn timestamp_substitution_changes_estimator_even_when_both_gates_open() {
    let (live_s,d,h)=setup(940,vec![true;12]);run(&live_s,&d,&h);
    let (capture_s,d,h)=setup(900,vec![true;12]);run(&capture_s,&d,&h);
    assert_eq!(live_s.this_zc.load(Relaxed),940);
    assert_eq!(capture_s.this_zc.load(Relaxed),900);
    assert_eq!(*h.com_arrs.borrow(),vec![301]);
    // Merely changing the reported interval does not compensate a COM timer
    // armed relative to dispatch. Event-time scheduling needs a coherent design.
}
