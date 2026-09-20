#[path="../examples/support/flying_acquire.rs"] mod flying_acquire;
use flying_acquire::*;
const EDGES:[(u8,bool);6]=[(2,true),(0,false),(1,true),(2,false),(0,true),(1,false)];

fn before_final(origin:u32,offset:usize)->RuntimeAcquire {
    let mut a=RuntimeAcquire::new(origin);
    for i in 0..12 {
        assert_eq!(a.preparation_step(),None);
        let (phase,level)=EDGES[(offset+i)%6];
        assert_eq!(a.edge(phase,level,origin.wrapping_add(100+i as u32*900)),Ok(None));
    }
    assert_eq!(a.intervals(),11);
    assert_eq!(a.preparation_step(),Some((offset+1) as u8));
    a
}

#[test] fn hint_never_substitutes_for_twelfth_real_edge() {
    for origin in [0,u32::MAX-7000] {for offset in 0..6 {
        let mut a=before_final(origin,offset);
        let onset=origin.wrapping_add(10900);
        assert_eq!(a.poll(onset),Ok(None));
        let (phase,level)=EDGES[offset];
        let seed=a.edge(phase,level,onset).unwrap().unwrap();
        assert_eq!(seed,Seed{step:(offset+1) as u8,edge_tick:onset,interval_ticks:900});
        assert_eq!(a.preparation_step(),None);
        // Completed seed remains one-shot: later traffic cannot freshen it.
        let (phase,level)=EDGES[(offset+1)%6];
        assert_eq!(a.edge(phase,level,onset.wrapping_add(900)),Ok(Some(seed)));
    }}
}

#[test] fn preparation_does_not_waive_order_interval_or_deadline() {
    for offset in 0..6 {
        for (gap,expected) in [(475,Fault::TooFast),(2001,Fault::TooSlow)] {
            let mut a=before_final(0,offset);let (p,l)=EDGES[offset];
            assert_eq!(a.edge(p,l,100+11*900+gap),Err(expected));
            assert_eq!(a.preparation_step(),None);
        }
        let mut a=before_final(0,offset);let (p,l)=EDGES[(offset+1)%6];
        assert_eq!(a.edge(p,l,10900),Err(Fault::WrongOrder));
        let mut a=before_final(0,offset);
        assert_eq!(a.poll(40001),Err(Fault::Expired));
        assert_eq!(a.preparation_step(),None);
        // Individually legal final interval can still violate full-cycle floor.
        let mut a=before_final(0,offset);let (p,l)=EDGES[offset];
        assert_eq!(a.edge(p,l,100+11*900+476),Err(Fault::CycleTooFast));
    }
}

#[test] fn preparation_must_not_black_out_persistence_sampling() {
    let mut f=EdgeFilter::new();
    assert_eq!(f.sample(false,0),Ok(None));
    assert_eq!(f.sample(true,100),Ok(None));
    assert_eq!(f.sample(true,139),Ok(None));
    assert_eq!(f.sample(true,140),Ok(Some((true,100))));
    let mut f=EdgeFilter::new();f.sample(false,0).unwrap();
    assert_eq!(f.sample(true,201),Err(()));
    assert_eq!(f.sample(true,202),Err(()));
}
