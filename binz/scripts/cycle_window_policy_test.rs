#[path="cycle_window_policy.rs"] mod policy;
#[path="../examples/support/accepted_timing.rs"] mod accepted_timing;
use policy::Window;

fn run(origin:u32, spacing:u32, delayed:Option<(u32,u32)>)->Option<u32> {
    let mut w=Window::<2778>::new();
    let mut m=accepted_timing::Monitor::new(origin,238,1000);
    for i in 0..80u32 {
        let delay=delayed.filter(|&(index,_)|index==i).map_or(0,|(_,d)|d);
        let now=origin.wrapping_add(i*spacing+delay);
        let step=(i%6+1) as u8;
        assert_eq!(m.event(now,step),None);
        if w.accepted(now,step).is_some() {return Some(i);}
    }
    None
}

#[test] fn warmup_and_exact_boundary_across_wrap() {
    for origin in [0,u32::MAX-2000,u32::MAX-16000] {
        assert_eq!(run(origin,462,None),Some(6));
        assert_eq!(run(origin,463,None),None);
        assert_eq!(run(origin,475,Some((25,100))),None);
        assert_eq!(run(origin,475,Some((25,150))),Some(37));
    }
}

#[test] fn silence_order_and_fast_events_still_require_monitor() {
    for step in [0,1,3,7] {
        let mut m=accepted_timing::Monitor::new(0,238,1000);
        assert_eq!(m.event(0,1),None);
        assert_eq!(m.event(475,step),Some(accepted_timing::Fault::SectorOrder));
    }
    let mut m=accepted_timing::Monitor::new(u32::MAX-500,238,1000);
    assert_eq!(m.poll(500),Some(accepted_timing::Fault::Stale));
    assert_eq!(m.event(501,1),Some(accepted_timing::Fault::Stale));
    let mut m=accepted_timing::Monitor::new(0,238,1000);
    m.event(0,1);
    assert_eq!(m.event(237,2),Some(accepted_timing::Fault::TooFast));
}

#[test] fn faults_latch_and_report_actual_window() {
    let mut w=Window::<2778>::new();
    for i in 0..6 {assert_eq!(w.accepted(i*462,(i+1) as u8),None);}
    let f=w.accepted(2772,1).unwrap();
    assert_eq!((f.span_us,f.minimum_us,f.slow),(2772,2778,false));
    assert_eq!(w.accepted(9999,2),Some(f));
    let mut slow=Window::<2778>::new();
    slow.accepted(0,1);
    assert!(slow.accepted(6001,1).unwrap().slow);
}

#[test] fn exhaustive_constant_acceleration_delay_bound() {
    let mut maximum=0;
    let mut witness=(0,0,0,0);
    for old in 463..=1000u32 {for new in 238..=462u32 {
        let mut w=Window::<2778>::new();
        let mut times=[0u32;60]; let mut first_single=None; let mut first_pair=None;
        for i in 0..60usize {
            if i>0 {times[i]=times[i-1]+if i<=30 {old}else{new};}
            if i>=6 && first_single.is_none() && times[i]-times[i-6]<2778 {first_single=Some(i);}
            if w.accepted(times[i],(i%6+1) as u8).is_some() {first_pair=Some(i);break;}
        }
        let a=first_single.unwrap(); let b=first_pair.unwrap();
        assert!(b>=a && b-a<=6);
        let delay=times[b]-times[a];
        assert!(delay<2778);
        if delay>maximum {maximum=delay;witness=(old,new,a,b);}
    }}
    println!("constant-step acceleration sweep: max extra delay {maximum}us, witness {witness:?}");
}

#[test] fn two_cycle_average_does_not_bound_each_cycle() {
    // Counterexample, not a physically plausible acceleration claim:
    // six238us gaps then six688us gaps total exactly5556us every12 events.
    let mut w=Window::<2778>::new();
    let mut m=accepted_timing::Monitor::new(0,238,1000);
    let mut times=[0u32;150]; let mut shortest=u32::MAX;
    for i in 0..times.len() {
        if i>0 {times[i]=times[i-1]+if i<=30 {1000}
            else if (i-31)%12<6 {238}else{688};}
        let step=(i%6+1) as u8;
        assert_eq!(m.event(times[i],step),None);
        assert_eq!(w.accepted(times[i],step),None);
        if i>=36 {shortest=shortest.min(times[i]-times[i-6]);}
    }
    assert_eq!(shortest,1428);
    println!("average-only counterexample:1428us single cycle admitted with valid order/gaps");
}
