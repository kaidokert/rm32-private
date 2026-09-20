//! Synthetic contract tests; NOT replayed hardware or proof of BEMF lock.
mod support;
use support::*;
use drv_observer_replay::driven_irq_core::{self, Acceptance, Result};
use portable_atomic::Ordering::Relaxed;

fn setup(step:u16)->(SchedStore,DriveStore,MockHal) {
    let s=SchedStore::default(); let d=DriveStore::default(); let h=MockHal::new();
    s.average_interval.store(1666,Relaxed); s.wait_time.store(416,Relaxed);
    d.current_step.store(step,Relaxed); d.rising.store(step%2==1,Relaxed);
    d.filter_level.store(12,Relaxed);
    h.interval.set(1000); h.pending.set(true); h.comp_enabled.set(true);
    h.comp_value.set(step%2==1);
    (s,d,h)
}
fn visit(s:&SchedStore,d:&DriveStore,h:&MockHal)->Result {
    driven_irq_core::visit(&s.sched(),&d.drive(),h,h,h).2
}
#[test] fn immediate_persistence_accepts_all_sectors_without_coarse_baseline() {
    for step in 1..=6 {
        let (s,d,h)=setup(step);
        // Exactly 12 immediate expected reads; an extra read would see opposite.
        *h.comp_seq.borrow_mut()=vec![step%2==1;12];
        h.comp_value.set(step%2!=1);
        assert_eq!(visit(&s,&d,&h),Result::Accepted(Acceptance{
            sector:step as u8-1,interval:1000,requested_arr:417}));
        assert!(h.comp_seq.borrow().is_empty());
        assert!(!h.pending.get()); assert!(!h.comp_enabled.get());
        assert_eq!(h.interval.get(),0);
        // These physical seams are never supplied to the adapter.
        assert!(h.com_arrs.borrow().is_empty()); assert_eq!(h.com_deadline.get(),None);
        assert!(h.roles.borrow().is_empty()); assert!(h.duties.borrow().is_empty());
    }
}
#[test] fn one_wrong_read_at_every_filter_position_refuses() {
    for index in 0..12 {
        let (s,d,h)=setup(1);let mut reads=vec![true;12];reads[index]=false;
        *h.comp_seq.borrow_mut()=reads;
        assert_eq!(visit(&s,&d,&h),Result::NoAcceptance);
        assert!(!h.pending.get());assert!(h.comp_enabled.get());
        assert_eq!(h.interval.get(),1000);assert_eq!(h.com_deadline.get(),None);
    }
}
#[test] fn gate_is_strict_and_preserves_production_pending_camp() {
    let (s,d,h)=setup(1);h.interval.set(833);
    assert_eq!(visit(&s,&d,&h),Result::NoAcceptance);
    assert!(h.pending.get()); // Must be bounded by caller's real storm guard.
    h.interval.set(834);
    assert!(matches!(visit(&s,&d,&h),Result::Accepted(_)));
}
#[test] fn gate_closed_opposite_clears_but_no_pending_never_accepts() {
    let (s,d,h)=setup(1);h.interval.set(833);h.comp_value.set(false);
    assert_eq!(visit(&s,&d,&h),Result::NoAcceptance);assert!(!h.pending.get());
    h.interval.set(2000);h.comp_value.set(true);
    assert_eq!(visit(&s,&d,&h),Result::NoAcceptance);
    assert_eq!(h.interval.get(),2000);
}
#[test] fn invalid_sector_filter_and_uninitialized_gate_touch_no_hardware() {
    for (step,filter,average) in [(0,12,1666),(7,12,1666),(1,0,1666),(1,13,1666),(1,12,0)] {
        let (s,d,h)=setup(step);d.filter_level.store(filter,Relaxed);
        s.average_interval.store(average,Relaxed);
        assert_eq!(visit(&s,&d,&h),Result::InvalidState);
        assert!(h.calls.borrow().is_empty());assert!(h.pending.get());
    }
}
#[test] fn accepted_sample_is_live_timer_not_command_interval_and_no_repeat() {
    let (s,d,h)=setup(1);h.interval.set(1587);
    s.this_zc.store(1601,Relaxed);
    assert_eq!(visit(&s,&d,&h),Result::Accepted(Acceptance{
        sector:0,interval:1587,requested_arr:417}));
    assert_eq!(s.last_zc.load(Relaxed),1601);assert_eq!(s.this_zc.load(Relaxed),1587);
    assert_eq!(visit(&s,&d,&h),Result::NoAcceptance);
    h.advance(10000);
    assert_eq!(h.com_deadline.get(),None);assert!(h.roles.borrow().is_empty());
}
