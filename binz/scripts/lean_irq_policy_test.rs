#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
#[path="../examples/support/powered_guard.rs"] mod powered_guard;
#[path="../examples/support/scan_queue.rs"] mod scan_queue;
#[test] fn lean_policy_keeps_control_without_diagnostic_counters() {
    let mut monitor=accepted_timing::Monitor::new(0,100,1000);
    assert_eq!(monitor.event_policy::<true>(100,1),None);
    assert_eq!(monitor.event_policy::<true>(179,2),None);
    assert_eq!(monitor.fast_events(),(0,0));
    assert_eq!(monitor.poll(1180),Some(accepted_timing::Fault::Stale));
    let f=powered_guard::Feedback{phase:[2048;3],bus_mv:11700,vref:1500};
    let mut g=powered_guard::RunGuard::<2223,100,true>::with_limits(0,0,1,f,100000,50000).unwrap();
    for i in 0..24 {assert_eq!(g.accepted(100+i*79,((i+1)%6+1) as u8),None);}
    assert_eq!(g.fast_cycles(),(0,0));assert_eq!(g.fast_events(),(0,0));
    assert_eq!(g.accepted(2000,1),Some(powered_guard::Fault::Tracking));
    let mut q=scan_queue::Queue::new();
    for n in 0..8 {assert!(q.push(scan_queue::Frame{raw:[n;5],acquired:n as u32}));}
    assert_eq!(q.peak(),0);
    for n in 0..8 {assert_eq!(q.pop().unwrap().raw,[n;5]);}
    assert_eq!(q.pop(),None);
    for n in 0..8 {assert!(q.push(scan_queue::Frame{raw:[n;5],acquired:n as u32}));}
    assert!(!q.push(scan_queue::Frame{raw:[0;5],acquired:0}));assert_eq!(q.pop(),None);
}
