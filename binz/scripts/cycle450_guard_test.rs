#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
#[path="../examples/support/powered_guard.rs"] mod powered_guard;
use powered_guard::{RunGuard,Feedback,Fault};
fn feedback()->Feedback {Feedback{phase:[2048;3],bus_mv:11700,vref:1500}}
fn new()->RunGuard<2223,238> {RunGuard::with_limits(0,0,1,feedback(),1_000_000,500_000).unwrap()}
#[test] fn exact_cycle_boundary_latches() {
    for span in [2222,2223,2224,2493] {
        let mut g=new();
        for i in 0..6 {assert_eq!(g.accepted(500+i*370,((i+1)%6+1) as u8),None);}
        assert_eq!(g.accepted(500+span,2),if span<2223 {Some(Fault::CycleTiming)}else{None});
        if span<2223 {assert_eq!(g.accepted(3500,3),Some(Fault::CycleTiming));}
    }
}
#[test] fn independent_stops_remain() {
    for gap in [237,1001] {
        let mut g=new();assert_eq!(g.accepted(500,2),None);
        assert_eq!(g.accepted(500+gap,3),Some(Fault::Tracking));
    }
    assert_eq!(new().accepted(500,3),Some(Fault::Tracking));
    assert_eq!(new().poll(1,false,false),Some(Fault::Driver));
    assert_eq!(new().poll(1,true,true),Some(Fault::HostAbort));
    assert_eq!(new().poll(201,true,false),Some(Fault::TickGap));
    assert_eq!(new().feedback(1001,feedback()),Some(Fault::FeedbackStale));
    for phase in 0..3 {for raw in [847,3249] {
        let mut f=feedback();f.phase[phase]=raw;
        assert_eq!(new().feedback(1,f),Some(Fault::Current));
    }}
    let mut f=feedback();f.bus_mv=8399;
    assert_eq!(new().feedback(1,f),Some(Fault::Bus));
    assert_eq!(new().poll(500000,true,false),Some(Fault::SegmentDeadline));
    assert_eq!(new().poll(1000000,true,false),Some(Fault::CampaignDeadline));
}

#[test] fn event_monitor_is_not_a_replacement_for_cycle_speed_bound() {
    // Counterfactual policy audit ONLY: no live firmware changes.
    // An ordered 600eHz-like train meets 238..1000us event checks,
    // yet violates the current 450eHz single-cycle ceiling.
    let mut monitor=accepted_timing::Monitor::new(0,238,1000);
    let mut guard=new();
    for i in 0..13 {
        let now=278*(i+1);let step=((i+1)%6+1) as u8;
        assert_eq!(monitor.event(now,step),None);
        assert_eq!(guard.accepted(now,step),if i<6 {None}else{Some(Fault::CycleTiming)});
    }
}
