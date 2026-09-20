#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
#[path="../examples/support/powered_guard.rs"] mod powered_guard;
use powered_guard::{RunGuard,Feedback,Fault};
fn feedback()->Feedback {Feedback{phase:[2048;3],bus_mv:11700,vref:1500}}
fn new()->RunGuard<2500,238> {RunGuard::with_limits(0,0,1,feedback(),1_000_000,500_000).unwrap()}
#[test] fn exact_cycle_boundary_and_latched_refusal() {
    for span in [2499,2500,2501,2778] {
        let mut g=new();
        for i in 0..6 {assert_eq!(g.accepted(500+i*417,((i+1)%6+1) as u8),None);}
        assert_eq!(g.accepted(500+span,2),if span<2500 {Some(Fault::CycleTiming)}else{None});
        if span<2500 {assert_eq!(g.accepted(3500,3),Some(Fault::CycleTiming));}
    }
}
#[test] fn other_guards_unchanged() {
    let mut g=new();assert_eq!(g.accepted(500,2),None);
    assert_eq!(g.accepted(737,3),Some(Fault::Tracking));
    assert_eq!(new().accepted(500,3),Some(Fault::Tracking));
    assert_eq!(new().poll(1,false,false),Some(Fault::Driver));
    assert_eq!(new().poll(1,true,true),Some(Fault::HostAbort));
    assert_eq!(new().feedback(1001,feedback()),Some(Fault::FeedbackStale));
    let mut f=feedback();f.phase[0]=3249;
    assert_eq!(new().feedback(1,f),Some(Fault::Current));
    f=feedback();f.bus_mv=8399;
    assert_eq!(new().feedback(1,f),Some(Fault::Bus));
    assert_eq!(new().poll(500000,true,false),Some(Fault::SegmentDeadline));
}
