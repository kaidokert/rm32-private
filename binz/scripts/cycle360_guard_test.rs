#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
#[path="../examples/support/powered_guard.rs"] mod powered_guard;
use powered_guard::{RunGuard,Feedback,Fault};
fn feedback()->Feedback {Feedback{phase:[2048;3],bus_mv:11700,vref:1500}}

#[test] fn running_cycle_boundary_only() {
    for spacing in [462,463,475,476,477] {
        let mut old=RunGuard::<2858,238>::with_limits(0,0,1,feedback(),1_000_000,500_000).unwrap();
        let mut next=RunGuard::<2778,238>::with_limits(0,0,1,feedback(),1_000_000,500_000).unwrap();
        for i in 1..=7 {
            let step=(i%6+1) as u8;
            assert_eq!(old.accepted(i*spacing,step),if i==7 && spacing*6<2858 {Some(Fault::CycleTiming)}else{None});
            assert_eq!(next.accepted(i*spacing,step),if i==7 && spacing*6<2778 {Some(Fault::CycleTiming)}else{None});
        }
    }
}
#[test] fn electrical_and_tracking_limits_retained() {
    let new=||RunGuard::<2778,238>::with_limits(0,0,1,feedback(),1_000_000,500_000).unwrap();
    let mut fast=new();
    assert_eq!(fast.accepted(500,2),None);
    assert_eq!(fast.accepted(737,3),Some(Fault::Tracking));
    assert_eq!(new().accepted(500,3),Some(Fault::Tracking));
    let mut g=new();
    for now in (100..=1000).step_by(100) {
        assert_eq!(g.feedback(now,feedback()),None);
        assert_eq!(g.poll(now,true,false),None);
    }
    assert_eq!(g.feedback(1001,feedback()),None);
    assert_eq!(g.poll(1001,true,false),Some(Fault::Tracking));
    let mut f=feedback(); f.phase[0]=3249;
    assert_eq!(new().feedback(100,f),Some(Fault::Current));
    f=feedback();f.bus_mv=8399;
    assert_eq!(new().feedback(100,f),Some(Fault::Bus));
    assert_eq!(new().poll(1,false,false),Some(Fault::Driver));
}
