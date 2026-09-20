#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
#[path="../examples/support/powered_guard.rs"] mod powered_guard;
use powered_guard::{RunGuard,Feedback,Fault};
const MIN:u32=if cfg!(feature="bench-event100") {100}else{238};
type G=RunGuard<2223,MIN,true>;
fn sample()->Feedback {Feedback{phase:[2048;3],bus_mv:11700,vref:1500}}
fn new()->G {G::with_limits(0,0,1,sample(),1_000_000,500_000).unwrap()}
#[test] fn short_cycles_report_without_losing_order_or_timestamps() {
    for origin in [0,u32::MAX-1000] {
        let mut g=G::with_limits(origin,0,1,sample(),1_000_000,500_000).unwrap();
        for i in 1..=24 {
            assert_eq!(g.accepted(origin.wrapping_add(i*300),(i%6+1) as u8),None);
        }
        assert_eq!(g.fast_cycles(),(18,1800));
        assert_eq!(g.accepted(origin.wrapping_add(7500),3),Some(Fault::Tracking));
        assert_eq!(g.fast_cycles(),(18,1800));
    }
}
#[test] fn all_independent_stops_remain() {
    for gap in [1001] {
        let mut g=new();assert_eq!(g.accepted(500,2),None);
        assert_eq!(g.accepted(500+gap,3),Some(Fault::Tracking));
    }
    assert_eq!(new().poll(1,false,false),Some(Fault::Driver));
    assert_eq!(new().poll(1,true,true),Some(Fault::HostAbort));
    assert_eq!(new().poll(201,true,false),Some(Fault::TickGap));
    assert_eq!(new().feedback(1001,sample()),Some(Fault::FeedbackStale));
    for phase in 0..3 {for raw in [847,3249] {
        let mut f=sample();f.phase[phase]=raw;
        assert_eq!(new().feedback(1,f),Some(Fault::Current));
    }}
    for vref in [0,4095] {
        let mut f=sample();f.vref=vref;
        assert_eq!(new().feedback(1,f),Some(Fault::Bus));
    }
    let mut f=sample();f.bus_mv=8399;
    assert_eq!(new().feedback(1,f),Some(Fault::Bus));
    assert_eq!(new().poll(500000,true,false),Some(Fault::SegmentDeadline));
    assert_eq!(new().poll(1000000,true,false),Some(Fault::CampaignDeadline));
}
#[test] fn exact_event_floor_and_displaced_pair() {
    let mut g=new();g.accepted(500,2);
    assert_eq!(g.accepted(500+MIN,3),None);
    let mut g=new();g.accepted(500,2);g.accepted(850,3);
    assert_eq!(g.accepted(1050,4),None);
    assert_eq!(g.fast_events(),if MIN==100 {(0,0)}else{(1,200)});
}
#[test] fn short_events_report_and_default_still_kills() {
    for gap in [0,MIN-1,MIN] {
        let mut g=new();g.accepted(500,2);
        assert_eq!(g.accepted(500+gap,3),None);
        assert_eq!(g.fast_events(),if gap<MIN {(1,gap)}else{(0,0)});
        assert_eq!(g.accepted(501+gap,3),Some(Fault::Tracking));
        let mut strict=RunGuard::<2223,MIN>::with_limits(0,0,1,sample(),1000000,500000).unwrap();
        strict.accepted(500,2);
        assert_eq!(strict.accepted(500+gap,3),if gap<MIN {Some(Fault::Tracking)}else{None});
    }
}
#[test] fn installation_resets_report_but_not_admission_checks() {
    let mut slot=Some(new());
    for i in 1..=12 {slot.as_mut().unwrap().accepted(i*300,(i%6+1) as u8);}
    assert_eq!(slot.as_ref().unwrap().fast_cycles(),(6,1800));
    G::install(&mut slot,G::admit(10,200,3,sample(),100000,50000,25).unwrap());
    assert_eq!(slot.as_ref().unwrap().fast_cycles(),(0,0));
    assert_eq!(slot.as_ref().unwrap().feedback_timestamp(),10u32.wrapping_sub(25));
    assert_eq!(G::admit(0,0,1,sample(),100000,50000,1001).err(),Some(Fault::FeedbackStale));
}
