#[path="../examples/support/flying_acquire.rs"] mod flying_acquire;
#[path="../examples/support/driven_seed.rs"] mod driven_seed;
#[path="../examples/support/irq_dispatch.rs"] mod irq_dispatch;
use driven_seed::{Qualification,Edge};
fn e(epoch:u16,at:u32,interval:u16)->Edge {
    Edge{epoch,step:((epoch-1)%6+1) as u8,before:at,after:at+24,interval}
}
#[test] fn e553_replay_requires_twelve_new_intervals() {
    let rows:[[u32;5];15]=[
        [0,3,896,908,1374],[1,4,1453,1465,1114],[2,5,2524,2537,2142],
        [3,6,3242,3254,1435],[4,1,4086,4099,1689],[5,2,4786,4798,1399],
        [6,3,5637,5649,1701],[7,4,6223,6236,1173],[8,5,7093,7105,1739],
        [9,6,7782,7794,1377],[10,1,8575,8588,1587],[11,2,9277,9290,1404],
        [12,3,10118,10130,1681],[13,4,11027,11040,1819],[14,5,11785,11798,1516]];
    let mut old=Qualification::with_reanchor(0);
    let mut candidate=Qualification::with_timing_reanchor(0);
    for (i,r) in rows.iter().enumerate() {
        let edge=Edge{epoch:r[0] as u16,step:r[1] as u8,before:r[2]*2,after:r[3]*2,interval:r[4] as u16};
        old.accept(edge);
        assert_eq!(candidate.accept(edge).is_some(),i==14);
        if i==2 {assert_eq!(candidate.intervals(),0);}
    }
    assert_eq!(old.fault(),3);assert_eq!(old.ready(),None);
    assert_eq!(candidate.reanchors(),(1,2));assert_eq!(candidate.discarded_fault(),3);
    let seed=candidate.ready().unwrap();
    assert_eq!((seed.interval_ticks,seed.edge_tick),(1543,23570));
    assert_eq!(candidate.intervals(),12);assert_eq!(candidate.cycles()[0],7);
}
#[test] fn no_second_restart_or_clock_refresh() {
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,100,0));q.accept(e(2,2300,2200));
    assert_eq!(q.discarded_fault(),3);
    q.accept(e(3,4500,2200));assert_eq!(q.fault(),3);
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,30000,0));q.accept(e(2,32200,2200));
    for i in 3..=14 {q.accept(e(i,32200+(i as u32-2)*1600,1600));}
    assert!(q.ready().is_none());assert_ne!(q.fault(),0);
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,100,0));q.accept(e(3,3300,3200));
    q.accept(e(4,5500,2200));assert_eq!(q.fault(),3);
}
#[test] fn corrupt_interval_duplicate_and_fast_event_still_refuse() {
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,100,0));q.accept(e(2,2300,1500));assert_eq!(q.fault(),4);
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,100,0));q.accept(e(1,2300,2200));assert_eq!(q.fault(),2);
    let mut q=Qualification::with_timing_reanchor(0);
    q.accept(e(1,100,0));q.accept(e(2,200,100));assert_eq!(q.fault(),5);
}
