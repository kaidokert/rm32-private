mod support;
use support::*;
use drv_observer_replay::filtered_irq_source::Source;
use minz_core::am32_isr;
use portable_atomic::Ordering::Relaxed;

fn setup()->(SchedStore,DriveStore,MockHal) {
    let s=SchedStore::default();let d=DriveStore::default();let h=MockHal::new();
    s.average_interval.store(1200,Relaxed);s.wait_time.store(300,Relaxed);
    d.rising.store(true,Relaxed);d.current_step.store(1,Relaxed);d.filter_level.store(12,Relaxed);
    h.comp_value.set(true);(s,d,h)
}
// Translate only source pending/mask operations; execute actual minz-core ISR.
fn visit(src:&mut Source,s:&SchedStore,d:&DriveStore,h:&MockHal) {
    if !src.dispatch() {return;}
    h.pending.set(src.pending());h.comp_enabled.set(src.enabled());
    am32_isr::comp_isr(&s.sched(),&d.drive(),&mut h.motor(),&h.observer());
    if !h.pending.get() {src.clear();}
    if !h.comp_enabled.get() {src.mask();}
}
#[test] fn post_crossing_pending_survives_strict_gate_then_arms_once() {
    let (s,d,h)=setup();let mut src=Source::new();let ticket=src.phase();src.enable(ticket);src.capture(ticket);
    h.interval.set(600);visit(&mut src,&s,&d,&h);
    assert!(src.pending());assert!(h.com_arrs.borrow().is_empty());
    h.interval.set(601);visit(&mut src,&s,&d,&h);
    assert!(!src.pending());assert!(!src.enabled());assert_eq!(*h.com_arrs.borrow(),vec![301]);
    assert_eq!(h.interval.get(),0);assert_eq!(s.this_zc.load(Relaxed),601);
    visit(&mut src,&s,&d,&h);assert_eq!(h.com_arrs.borrow().len(),1);
}
#[test] fn pre_crossing_and_persistence_flip_clear_without_commutation() {
    for open in [false,true] {
        let (s,d,h)=setup();let mut src=Source::new();let ticket=src.phase();src.enable(ticket);src.capture(ticket);
        h.interval.set(if open {700}else{600});
        if open {h.comp_seq.replace(vec![true,true,false]);} else {h.comp_value.set(false);}
        visit(&mut src,&s,&d,&h);
        assert!(!src.pending());assert!(src.enabled());assert!(h.com_arrs.borrow().is_empty());
    }
}
#[test] fn masked_capture_stale_epoch_and_no_edge_have_no_authority() {
    let (s,d,h)=setup();let mut src=Source::new();let old=src.phase();src.capture(old);
    h.interval.set(700);visit(&mut src,&s,&d,&h);assert!(h.com_arrs.borrow().is_empty());
    src.stop();assert!(!src.enable(old));assert!(!src.capture(old));
    let fresh=src.phase();assert!(!src.capture(old));src.enable(fresh);
    visit(&mut src,&s,&d,&h);assert!(h.com_arrs.borrow().is_empty());
    src.capture(fresh);src.capture(fresh);assert_eq!(src.overcaptures,1);
    visit(&mut src,&s,&d,&h);assert_eq!(h.com_arrs.borrow().len(),1);
}
