#[path="../examples/support/flying_acquire.rs"] mod flying_acquire;
use flying_acquire::*;
#[test] fn new_profile_retains_live_age_boundary_and_edge_floor() {
    assert_eq!((SEED_MIN_TICKS,CYCLE_MIN_TICKS,INDIVIDUAL_MIN_TICKS),(834,5000,476));
    for ci in [834,895,952,1543] {
        let wait=minz_core::am32::wait_time(ci,minz_core::am32::advance_of(ci,16));
        for edge in [100,u32::MAX-100] {
            let s=Seed{step:1,edge_tick:edge,interval_ticks:ci};
            let age=wait-64;
            assert_eq!(s.handoff_with_min::<834>(edge.wrapping_add(age),wait),Some((age,64)));
            assert_eq!(s.handoff_with_min::<834>(edge.wrapping_add(age+1),wait),None);
        }
    }
    assert_eq!(Seed{step:1,edge_tick:0,interval_ticks:833}.handoff_with_min::<834>(0,500),None);
}
#[test] fn seed_requires_twelve_intervals_and_short_cycles_still_refuse() {
    const EDGES:[(u8,bool);6]=[(2,true),(0,false),(1,true),(2,false),(0,true),(1,false)];
    for spacing in [833,834,895] {
        let mut a=RuntimeAcquire::new(0);
        for i in 0..=12 {
            let (p,l)=EDGES[i%6];let r=a.edge(p,l,100+i as u32*spacing);
            if spacing==833 && i==6 {assert_eq!(r,Err(Fault::CycleTooFast));break;}
            if i<12 {assert_eq!(r,Ok(None));}else{assert_eq!(r.unwrap().unwrap().interval_ticks,spacing);}
        }
    }
    let mut a=RuntimeAcquire::new(0);
    assert_eq!(a.edge(2,true,100),Ok(None));
    assert_eq!(a.edge(0,false,575),Err(Fault::TooFast));
}
