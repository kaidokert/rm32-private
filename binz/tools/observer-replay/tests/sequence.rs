//! Integrated logical-time tests. Synthetic levels, NOT a motor model or bench
//! replay. The fixture controls time; all control decisions use actual minz-core.
mod support;
#[path = "support/coast_trace.rs"]
mod coast_trace;
use support::*;
use portable_atomic::Ordering::Relaxed;
use minz_core::{am32_control as ctl, am32_isr as isr, am32_loop as lp};

struct Rig {
    h: MockHal, s: SchedStore, d: DriveStore, u: DutyStore, z: ZctStore,
}
impl Rig {
    fn new(ci: u32, polling: bool) -> Self {
        let r = Self { h: MockHal::new(), s: SchedStore::default(),
            d: DriveStore::default(), u: DutyStore::default(), z: ZctStore::new() };
        r.s.commutation_interval.store(ci, Relaxed);
        r.s.average_interval.store(ci, Relaxed);
        r.s.last_average_interval.store(ci, Relaxed);
        for v in &r.s.interval_hist { v.store(ci, Relaxed); }
        r.s.this_zc.store(ci as u16, Relaxed);
        r.s.last_zc.store(ci as u16, Relaxed);
        r.s.wait_time.store((ci / 4) as u16, Relaxed);
        r.d.current_step.store(1, Relaxed);
        r.d.rising.store(true, Relaxed);
        r.d.running.store(true, Relaxed);
        r.d.old_routine.store(polling, Relaxed);
        r.d.zero_crosses.store(20, Relaxed);
        r.u.input.store(300, Relaxed);
        r.u.adjusted_input.store(300, Relaxed);
        r.h.comp_enabled.set(!polling);
        r.bands();
        r
    }
    fn bands(&self) {
        let (s,d,u) = (self.s.sched(),self.d.drive(),self.u.duty());
        lp::min_bemf_schedule(&d);
        let ect=s.intervals().e_com_time();
        let avg=lp::store_average_interval(&s,ect);
        ctl::desync_check_band(&s,&d,&u,&self.h.observer(),avg);
        let (running,zc)=lp::filter_and_duty_max(&s,&d,&u,ect,avg);
        lp::bemf_timeout_resets(&d,&u,zc);
        ctl::bemf_timeout_rekick(&s,&d,&u,&self.z.zct(),&mut self.h.motor(),
            &self.h.observer(),running);
    }
    fn irq(&self) {
        if self.h.comp_enabled.get() && self.h.pending.get() {
            isr::comp_isr(&self.s.sched(),&self.d.drive(),&mut self.h.motor(),&self.h.observer());
        }
    }
    fn advance(&self,ticks:u32) {
        // Dispatch deadlines rather than teleporting over an armed COM event.
        let end=self.h.now.get()+u64::from(ticks);
        while let Some(deadline)=self.h.com_deadline.get() {
            if deadline>end { break; }
            assert!(deadline>=self.h.now.get());
            self.h.advance((deadline-self.h.now.get()) as u32);
            isr::tim1_up_tim16_isr(&self.s.sched(),&self.d.drive(),&self.z.zct(),
                &self.u.duty(),&mut self.h.motor(),&self.h.observer());
            self.bands();
        }
        // A modeled timer read inside the COM ISR can consume time beyond
        // the requested advance. Never rewind the clock or underflow.
        if end>self.h.now.get() {self.h.advance((end-self.h.now.get()) as u32);}
    }
    fn edge(&self) {
        self.h.comp_value.set(self.d.rising.load(Relaxed));
        self.h.pending.set(true);
        self.irq();
    }
    fn poll(&self) {
        isr::polling_bemf_check(&self.s.sched(),&self.d.drive(),&self.u.duty(),
            &self.z.zct(),&mut self.h.motor(),&self.h.observer(),self.d.running.load(Relaxed));
    }
}

#[test]
fn fresh_commutation_seed_does_not_open_blank_immediately() {
    let r=Rig::new(1666,false);
    let wait=minz_core::am32::wait_time(1666,minz_core::am32::advance_of(1666,lp::TEMP_ADVANCE));
    assert_eq!(wait,417);
    r.s.wait_time.store(wait as u16,Relaxed);
    r.advance(wait+1);
    r.advance(24); // 12us after the modeled fresh commutation
    r.edge();
    assert!(r.h.com_deadline.get().is_none());
    assert!(r.h.pending.get()); // unchanged reference camps post-crossing level
    r.advance(392); // elapsed834 >833 opens reference gate
    r.irq();
    assert!(r.h.com_deadline.get().is_some());
}

#[test]
fn measured_flying_seed_bootstraps_one_real_com_without_fake_accept() {
    use drv_observer_replay::flying_acquire::Seed;
    use minz_core::am32_hal::ComTimer;
    for step in 1..=6 {
        let ci=1687;
        let r=Rig::new(ci,false);
        r.d.current_step.store(step,Relaxed);r.d.rising.store(step&1!=0,Relaxed);
        r.d.zero_crosses.store(12,Relaxed);
        let wait=minz_core::am32::wait_time(ci,minz_core::am32::advance_of(ci,lp::TEMP_ADVANCE));
        r.s.wait_time.store(wait as u16,Relaxed);
        let seed=Seed{step:step as u8,edge_tick:0,interval_ticks:ci};
        r.advance(104); // captured acquisition completion age
        let (age,arr)=seed.handoff(104,wait).unwrap();
        assert_eq!(age,r.h.interval.get());
        r.h.comp_enabled.set(false);
        (&r.h).set_and_enable(arr);
        assert_eq!(r.h.com_deadline.get(),Some(u64::from(wait+1)));
        r.advance(arr as u32);
        assert_eq!(r.d.current_step.load(Relaxed),step);
        r.advance(1);
        assert_eq!(r.d.current_step.load(Relaxed),step%6+1);
        let voltage=drv_observer_replay::sixstep::plan((step%6+1) as u8,65).unwrap();
        assert_eq!(voltage.floating,[2,0,1,2,0,1][(step%6) as usize]);
        assert_eq!(r.s.average_interval.load(Relaxed),ci);
        assert!(r.h.comp_enabled.get());
        assert!(r.h.com_deadline.get().is_none());
        assert_eq!(r.h.events.borrow().iter().filter(|e|e.0==minz_core::blackbox::EV_ACC).count(),0);
        // A fresh real comparator event is required to schedule the next COM.
        r.advance(ci-(wait+1));r.edge();
        assert!(r.h.com_deadline.get().is_some());
        assert_eq!(r.h.events.borrow().iter().filter(|e|e.0==minz_core::blackbox::EV_ACC).count(),1);
        assert_eq!(r.d.desync_happened.load(Relaxed),0);
    }
}

#[test]
fn actual_sine_lut_sector_order_matches_gate_and_acquisition_conventions() {
    use drv_observer_replay::{sixstep,flying_acquire::Acquire};
    // Read the actual firmware LUT, not a second handwritten waveform table.
    let source=include_str!("../../../examples/support/sine_table.rs");
    let table=source.split("const SINE_LUT: [u8; 256] = [").nth(1).unwrap().split("]; ").next().unwrap();
    let table=table.split("];").next().unwrap();
    let values:Vec<u8>=table.split(',').filter_map(|v| {
        let s=v.trim();if s.is_empty() {None}else{Some(s.parse().unwrap())}
    }).collect();
    assert_eq!(values.len(),256);
    let mut last=0;
    let mut transitions=Vec::new();
    for i in 0..=256 {
        let phases=[values[i%256],values[(i+85)%256],values[(i+170)%256]];
        let step=sixstep::step_for_values(phases);
        let plan=sixstep::plan(step,65).unwrap();
        assert_eq!(phases[plan.source],*phases.iter().max().unwrap());
        assert_eq!(phases[plan.sink],*phases.iter().min().unwrap());
        if last!=0 && last!=step {assert_eq!(step,last%6+1);transitions.push(step);}
        last=step;
    }
    assert_eq!(transitions,vec![6,1,2,3,4,5]);
    // Ideal commanded phase crossings: check geometry/polarity only. This is
    // NOT a rotor lag measurement or justification for powered handoff timing.
    let mut levels=[false;3];let mut crossings=Vec::new();
    for i in 0..=256 {
        for p in 0..3 {
            let level=values[(i+p*85)%256]>=128;
            if i!=0 && level!=levels[p] {
                let mut a=Acquire::new(0);
                a.edge(p as u8,level,0).unwrap();
                // Observe 12 more ideal ordered edges to expose the seed step.
                let edges=[(2,true),(0,false),(1,true),(2,false),(0,true),(1,false)];
                let offset=edges.iter().position(|&e|e==(p as u8,level)).unwrap();
                let mut seed=None;
                for n in 1..=12 {
                    let (phase,rising)=edges[(offset+n)%6];
                    seed=a.edge(phase,rising,n as u32*1667).unwrap();
                }
                let step=seed.unwrap().step;
                let phases=[values[i%256],values[(i+85)%256],values[(i+170)%256]];
                assert_eq!(sixstep::step_for_values(phases),step);
                assert_eq!(sixstep::plan(step,65).unwrap().floating,p);
                crossings.push(step);
            }
            levels[p]=level;
        }
    }
    assert_eq!(crossings,vec![6,1,2,3,4,5]);
}

#[test]
fn powered_capture_short_interval_is_legal_in_full_reference_sequence() {
    use minz_core::am32_hal::ComTimer;
    // E121 accepted interval register values, not synthesized motor voltages.
    // This tests reference timing legality, NOT whether the edges were BEMF.
    let r=Rig::new(1702,false);
    r.d.current_step.store(5,Relaxed);r.d.rising.store(true,Relaxed);
    r.d.zero_crosses.store(12,Relaxed);
    let wait=minz_core::am32::wait_time(1702,minz_core::am32::advance_of(1702,lp::TEMP_ADVANCE));
    r.h.comp_enabled.set(false);(&r.h).set_and_enable(wait as u16);
    for (i,interval) in [1672,1846,1447,1825,1425,1815,1216].into_iter().enumerate() {
        r.advance(interval-r.h.interval.get());r.edge();r.bands();
        assert_eq!(r.h.events.borrow().iter().filter(|e|e.0==minz_core::blackbox::EV_ACC).count(),i+1);
        assert_eq!(r.d.desync_happened.load(Relaxed),0);
        assert!(!r.d.old_routine.load(Relaxed));
    }
    assert!((1333..=2000).contains(&r.s.average_interval.load(Relaxed)));
}

#[test]
fn masked_queued_irq_must_not_rearm_pending_com() {
    use drv_observer_replay::irq_dispatch::{qualify,Decision};
    for guarded in [false,true] {
        let r=Rig::new(1667,false);
        r.advance(1667);r.edge();
        let deadline=r.h.com_deadline.get().unwrap();
        // Delayed lower-priority COM plus a queued shared-vector invocation.
        // Deliberately advance hardware time without servicing that deadline.
        r.h.advance(1000);r.h.pending.set(true);
        assert!(!r.h.comp_enabled.get());
        if !guarded || qualify(!r.h.comp_enabled.get(),r.h.comp_enabled.get(),r.h.pending.get())==Decision::Dispatch {
            isr::comp_isr(&r.s.sched(),&r.d.drive(),&mut r.h.motor(),&r.h.observer());
        }
        let accepts=r.h.events.borrow().iter().filter(|e|e.0==minz_core::blackbox::EV_ACC).count();
        assert_eq!(accepts,if guarded {1}else{2});
        assert_eq!(r.h.com_deadline.get()==Some(deadline),guarded);
        assert_eq!(r.d.current_step.load(Relaxed),1);
    }
}

#[test]
fn post_level_after_every_com_can_accelerate_without_freshness_fault() {
    // Adversarial stimulus motivated by Entries075/079/080: every newly
    // selected phase is already at the expected level. This is NOT a rotor
    // model or a claim that this artifact is the measured signal's cause.
    // Unlike edge(), no independent periodic rotor crossing drives this test.
    use drv_observer_replay::event_watch::Watch;
    let r=Rig::new(1666,false);
    let wait=minz_core::am32::wait_time(1666,minz_core::am32::advance_of(1666,lp::TEMP_ADVANCE));
    r.s.wait_time.store(wait as u16,Relaxed);
    r.advance(wait+1); // same fresh-COM seed as the physical prefix
    let mut watch=Watch::new((r.h.now.get()/2) as u32,0,2500);
    let mut intervals=Vec::new();
    let mut first_desync=None;
    for count in 1..=12 {
        r.edge(); // expected post-level pending immediately after each COM
        assert!(r.h.com_deadline.get().is_none(),"must still pay blanking");
        let gate=r.s.average_interval.load(Relaxed)/2+1;
        let elapsed=r.h.interval.get();
        r.advance(gate.saturating_sub(elapsed));
        r.irq();
        let deadline=r.h.com_deadline.get().expect("post-level accepted at gate");
        assert_eq!(watch.poll((r.h.now.get()/2) as u32,count),None);
        r.advance((deadline-r.h.now.get()) as u32);
        intervals.push(r.s.average_interval.load(Relaxed));
        if first_desync.is_none() && r.d.desync_happened.load(Relaxed)!=0 {
            first_desync=Some(count);
        }
    }
    let roles=r.h.timed_roles.borrow();
    let first=roles[1].0-roles[0].0;
    let last=roles[11].0-roles[10].0;
    println!("post-level synthetic: average_intervals={intervals:?} first_com_ticks={first} last_com_ticks={last} first_desync_at_com={first_desync:?}");
    assert_eq!(roles.len(),12);
    assert_eq!(first_desync,Some(12),"reference eventually detects the collapse");
    assert!(!r.d.running.load(Relaxed));
    assert!(r.d.old_routine.load(Relaxed));
    assert!(last<first*3/4,"accepted-event train accelerates without rotor input");
    assert!(intervals[11]<1666*3/4);
    // Test-only external envelope (~167..250eHz), never derived from the
    // collapsing controller estimate. This does not expand live bench limits.
    use drv_observer_replay::accepted_timing::{Monitor,Fault};
    let mut timing=Monitor::new(0,1333,2000);
    let accepted:Vec<_>=r.h.timed_events.borrow().iter().copied()
        .filter(|e|e.1==minz_core::blackbox::EV_ACC).collect();
    assert_eq!(accepted.len(),12);
    assert_eq!(timing.event(accepted[0].0 as u32,accepted[0].2+1),None);
    assert_eq!(timing.event(accepted[1].0 as u32,accepted[1].2+1),Some(Fault::TooFast));
    // Fresh events are necessary but cannot certify rotor lock. No firmware
    // guard is weakened or gate authority granted by this regression.
}

#[test]
fn interrupt_train_200ehz_schedules_one_com_per_accept_then_missing_edge_stops() {
    let r=Rig::new(1667,false);
    for _ in 0..120 {
        // External synthetic crossing period, independent of estimated ci.
        r.advance(1667);
        r.edge();
    }
    r.advance(1667);
    let n=r.h.roles.borrow().len();
    assert_eq!(n,120);
    assert!((1665..=1669).contains(&r.s.average_interval.load(Relaxed)));
    assert_eq!(r.d.desync_happened.load(Relaxed),0);
    let roles=r.h.timed_roles.borrow();
    for pair in roles.windows(2) {
        assert!((1665..=1669).contains(&(pair[1].0-pair[0].0)));
        assert_eq!(pair[1].1,pair[0].1%6+1);
    }
    drop(roles);
    use drv_observer_replay::accepted_timing::{Monitor,Fault};
    let mut timing=Monitor::new(0,1333,2000);
    let accepted:Vec<_>=r.h.timed_events.borrow().iter().copied()
        .filter(|e|e.1==minz_core::blackbox::EV_ACC).collect();
    assert_eq!(accepted.len(),120);
    for &(tick,_,sector,_) in &accepted {
        assert_eq!(timing.event(tick as u32,sector+1),None);
    }
    let last_tick=accepted.last().unwrap().0 as u32;
    assert_eq!(timing.poll(last_tick+2000),None);
    assert_eq!(timing.poll(last_tick+2001),Some(Fault::Stale));
    r.advance(10_000);
    r.bands();
    assert_eq!(r.h.roles.borrow().len(),n,"no edge means no free-running COM");
    assert_eq!(r.d.desync_happened.load(Relaxed),0,"absence of desync is NOT lock");
    r.advance(35_001);
    r.bands();
    assert_eq!(r.d.bemf_timeout_happened.load(Relaxed),1);
    assert!(r.d.old_routine.load(Relaxed));
    assert_eq!(r.h.roles.borrow().len(),n+1,"reference timeout re-kicks");
}

#[test]
fn reference_rekick_occurs_after_shadow_power_guard_has_latched_tracking() {
    // Policy replay beside the timed reference, not a motor model or proof of
    // hardware safing. Actual peripheral post-stop veto has a separate bench test.
    use drv_observer_replay::powered_guard::{Feedback,Guard,Fault};
    let r=Rig::new(1667,false);
    for _ in 0..120 {r.advance(1667);r.edge();}
    r.advance(1667); // drain the last accepted one-shot COM
    let accepted:Vec<_>=r.h.timed_events.borrow().iter().copied()
        .filter(|e|e.1==minz_core::blackbox::EV_ACC).collect();
    assert_eq!(accepted.len(),120);
    let first_step=accepted[0].2+1;
    let seed=if first_step==1 {6}else{first_step-1};
    let sample=Feedback{phase:[2048;3],bus_mv:11800,vref:1500};
    let mut guard=Guard::with_limits(0,0,seed,sample,1_000_000,1_000_000).unwrap();
    let last_us=(accepted.last().unwrap().0/2) as u32;
    let mut index=0;
    let mut stopped=None;
    for now in (100..=last_us+1200).step_by(100) {
        assert_eq!(guard.feedback(now,sample),None);
        while index<accepted.len() && accepted[index].0/2<=u64::from(now) {
            let (tick,_,sector,_)=accepted[index];
            assert_eq!(guard.accepted((tick/2) as u32,sector+1),None);
            index+=1;
        }
        if let Some(fault)=guard.poll(now,true,false) {
            assert_eq!(fault,Fault::Tracking);stopped=Some(now);break;
        }
    }
    let stopped=stopped.expect("independent tracking stop despite fresh ADC");
    assert_eq!(index,120);
    assert!((1001..=1100).contains(&(stopped-last_us)));
    assert_eq!(r.d.bemf_timeout_happened.load(Relaxed),0);
    let before=r.h.roles.borrow().len();
    r.advance(45_001);r.bands();
    assert_eq!(r.d.bemf_timeout_happened.load(Relaxed),1);
    assert_eq!(r.h.roles.borrow().len(),before+1,"unguarded reference proposes re-kick");
    let late=(r.h.now.get()/2) as u32;
    assert!(late>stopped);
    assert_eq!(guard.feedback(late,sample),Some(Fault::Tracking));
    assert_eq!(guard.accepted(late,1),Some(Fault::Tracking));
    assert_eq!(guard.poll(late,true,false),Some(Fault::Tracking));
    assert!(!guard.healthy(),"reference progress cannot resurrect expired authority");
}

#[test]
fn polling_accept_waits_then_changes_to_interrupt_and_timer_commutates() {
    let r=Rig::new(1800,true);
    r.h.interval.set(1500);
    // One tick per timer read gives the blocking polling wait elapsed time.
    r.h.interval_step.set(1);
    r.h.comp_value.set(true);
    for _ in 0..4 { r.poll(); }
    assert_eq!(r.h.roles.borrow().len(),1);
    assert!(r.h.now.get()>=400,"polling acceptance must pay its wait");
    assert!(!r.d.old_routine.load(Relaxed));
    assert!(r.h.comp_enabled.get());
    assert!(r.h.com_deadline.get().is_none(),"polling ARR write does not arm COM");
    r.h.interval_step.set(0);
    r.advance(1400);
    r.edge();
    assert!(r.h.com_deadline.get().is_some());
    assert_eq!(r.h.roles.borrow().len(),1,"accept is not immediate commutation");
    r.advance(1000);
    assert_eq!(r.h.roles.borrow().len(),2);
}

#[test]
fn reference_startup_seed_converges_through_polling_before_irq() {
    let r=Rig::new(1666,true);
    r.d.running.store(false,Relaxed);
    r.d.zero_crosses.store(0,Relaxed);
    ctl::start_motor(&r.s.sched(),&r.d.drive(),&mut r.h.motor(),&r.h.observer());
    assert_eq!(r.s.commutation_interval.load(Relaxed),10_000);
    assert_eq!(r.h.interval.get(),5000);
    assert!(r.d.old_routine.load(Relaxed));
    assert!(!r.h.comp_enabled.get());
    // Inject explicitly synthetic sector intervals. This tests reference
    // state/changeover, NOT coast acquisition against a rotor model.
    r.h.interval_step.set(1);
    for _ in 0..20 {
        if !r.d.old_routine.load(Relaxed) {break;}
        r.h.interval.set(1666);
        r.h.comp_value.set(r.d.rising.load(Relaxed));
        r.d.bemf_counter.store(0,Relaxed);
        r.d.zcfound.store(false,Relaxed);
        r.bands();
        let before=r.h.roles.borrow().len();
        for _ in 0..64 {
            r.poll();
            if r.h.roles.borrow().len()>before {break;}
        }
        assert_eq!(r.h.roles.borrow().len(),before+1);
    }
    assert!(!r.d.old_routine.load(Relaxed));
    assert!(r.h.comp_enabled.get());
    assert!(r.d.zero_crosses.load(Relaxed)>5);
    assert_eq!(r.d.zcfr_guard_hits.load(Relaxed),0);
    assert!(r.h.com_deadline.get().is_none());
}

#[test]
fn recorded_mixed_history_reproduces_average_and_desync_comparisons() {
    // coast_history_200_01 H85, first18 EV_REF records: step,interval,
    // pre-main-band average,previous average,pre-increment zero_crosses.
    // Replay ONLY interval history + real desync band, not unrecorded edges.
    let rows=[(2,1666,1666,1666,0),(3,8908,1666,1666,0),
        (4,6855,2873,1666,1),(5,5316,3738,1666,2),(6,4186,4346,1666,3),
        (1,3564,4766,1666,4),(2,3047,5083,1666,5),(3,2685,5313,1666,6),
        (4,2463,4276,1666,7),(5,2272,3544,1666,8),(6,2128,3036,1666,9),
        (1,1995,2693,1666,10),(2,1995,2432,2432,11),(3,1794,2257,2432,12),
        (4,1696,2108,2432,13),(5,1707,1980,2432,14),(6,1722,1886,2432,15),
        (1,1722,1818,2432,16)];
    let r=Rig::new(1666,true);
    r.d.zero_crosses.store(0,Relaxed);
    for (i,(step,ci,avg,previous,zc)) in rows.into_iter().enumerate() {
        assert_eq!(r.s.average_interval.load(Relaxed),avg,"row{i}");
        assert_eq!(r.s.last_average_interval.load(Relaxed),previous,"row{i}");
        r.s.sched().intervals().push(step-1,ci);
        r.s.commutation_interval.store(ci,Relaxed);
        r.d.current_step.store(step as u16,Relaxed);
        if step==1 {r.d.desync_check.store(true,Relaxed);}
        r.d.zero_crosses.store(zc+u32::from(i!=0),Relaxed);
        let ss=r.s.sched();
        let new_avg=lp::store_average_interval(&ss,ss.intervals().e_com_time());
        ctl::desync_check_band(&ss,&r.d.drive(),&r.u.duty(),&r.h.observer(),new_avg);
        assert_eq!(r.d.desync_happened.load(Relaxed),0);
    }
    assert_eq!(r.s.last_average_interval.load(Relaxed),1773);
}

#[test]
fn recorded_startup_history_triggers_reference_desync_without_fast_irq_train() {
    // coast_history_seed2_200_01: actual EV_REF interval history. Replay
    // averaging and the observed desync operands, not unrecorded CPU timing.
    let r=Rig::new(1666,true);
    let rows=[(3,1666),(4,9533),(5,7574),(6,6105),(1,5003),(2,4177),
        (3,3457),(4,3042),(5,2681),(6,2460),(1,2244),(2,2132),
        (3,2023),(4,1942),(5,1942),(6,1765),(1,1699)];
    for (i,(step,ci)) in rows.into_iter().enumerate() {
        let ss=r.s.sched();ss.intervals().push(step-1,ci);
        let avg=lp::store_average_interval(&ss,ss.intervals().e_com_time());
        if i==10 {assert_eq!(avg,3010);}
        if i==16 {assert_eq!(avg,1917);}
    }
    // Captured EV_DSY operands, before previous-average update. History
    // proves both values exist; it does not locate the precise preemption
    // where foreground retained3010 while polling advanced another sector.
    r.s.last_average_interval.store(3010,Relaxed);
    r.s.commutation_interval.store(1717,Relaxed);
    r.d.zero_crosses.store(16,Relaxed);
    r.d.current_step.store(1,Relaxed);
    r.d.old_routine.store(false,Relaxed);
    r.d.desync_check.store(true,Relaxed);
    ctl::desync_check_band(&r.s.sched(),&r.d.drive(),&r.u.duty(),&r.h.observer(),1917);
    assert_eq!(r.d.desync_happened.load(Relaxed),1);
    assert!(!r.d.running.load(Relaxed));
    assert!(r.d.old_routine.load(Relaxed));
    assert_eq!(r.s.last_average_interval.load(Relaxed),1917);
}

#[test]
fn persistence_flip_rejects_without_arming_then_clean_event_recovers() {
    let r=Rig::new(1667,false);
    r.advance(1667);
    r.h.comp_seq.borrow_mut().extend([true,true,false]);
    r.edge();
    assert!(r.h.com_deadline.get().is_none());
    assert!(r.h.comp_enabled.get());
    assert!(!r.h.pending.get());
    r.edge();
    assert!(r.h.com_deadline.get().is_some());
    r.advance(500);
    assert_eq!(r.h.roles.borrow().len(),1);
}

#[test]
fn blank_camps_post_level_but_clears_pre_level() {
    let r=Rig::new(1667,false);
    r.edge();
    assert!(r.h.pending.get());
    assert!(r.h.com_deadline.get().is_none());
    r.h.comp_value.set(false);
    r.irq();
    assert!(!r.h.pending.get());
    r.edge();
    r.advance(834);
    r.irq();
    assert!(!r.h.pending.get());
    assert!(r.h.com_deadline.get().is_some());
}

#[test]
fn interrupt_train_250ehz_and_timer_wrap_remain_ordered() {
    let r=Rig::new(1333,false);
    // Absolute simulation time crosses u16 wrap while the interval timer
    // is reset on each accepted crossing, as on the MCU.
    for _ in 0..120 { r.advance(1333); r.edge(); }
    r.advance(500);
    assert_eq!(r.h.roles.borrow().len(),120);
    assert!(r.h.now.get()>65_536);
    assert!((1331..=1335).contains(&r.s.average_interval.load(Relaxed)));
    assert_eq!(r.d.desync_happened.load(Relaxed),0);
}

#[test]
fn fallback_hysteresis_and_desync_are_distinct_paths() {
    let r=Rig::new(2500,false);
    r.advance(2500); r.edge(); r.advance(700);
    assert!(!r.d.old_routine.load(Relaxed),"2500 is not above fallback threshold");
    r.s.average_interval.store(2501,Relaxed);
    r.advance(1800); r.edge(); r.advance(700);
    assert!(r.d.old_routine.load(Relaxed));
    assert!(!r.h.comp_enabled.get());

    let r=Rig::new(1667,false);
    r.s.last_average_interval.store(3000,Relaxed);
    r.d.desync_check.store(true,Relaxed);
    r.bands();
    assert_eq!(r.d.desync_happened.load(Relaxed),1);
    assert!(!r.d.running.load(Relaxed));
    assert!(r.d.old_routine.load(Relaxed));
    assert_eq!(r.d.zero_crosses.load(Relaxed),0);
}

#[test]
fn safety_kill_cancels_pending_commutation_and_latches() {
    let r=Rig::new(1667,false);
    r.advance(1667); r.edge();
    assert!(r.h.com_deadline.get().is_some());
    ctl::safety_kill(&r.d.drive(),&r.u.duty(),&mut r.h.motor(),&r.h.observer(),9);
    r.advance(50_000); r.bands();
    assert!(r.h.roles.borrow().is_empty());
    assert!(r.u.killed.load(Relaxed));
    assert!(!r.h.comp_enabled.get());
    assert!(r.h.called("all_off"));
    assert!(r.h.frozen.get());
    // Core's all_off is only a HAL request. Actual G071 MOE/gates/ENABLE
    // safing must still be independently measured on hardware.
}

#[test]
fn full_tim6_polling_and_main_bands_execute_changeover() {
    let r=Rig::new(1800,true);
    let bench=BenchStore::default();
    r.h.inj.set((0,0,0,1600)); // healthy synthetic ADC; not DRV calibration
    r.u.uart_duty_input.store(300,Relaxed);
    r.u.tim1_arr.store(3332,Relaxed);
    r.h.interval.set(1300);
    r.h.interval_step.set(1);
    r.h.comp_value.set(true);
    for _ in 0..4 {
        ctl::set_input(&r.s.sched(),&r.d.drive(),&r.u.duty(),
            &mut r.h.motor(),&r.h.observer());
        r.bands();
        r.h.advance(100); // nominal 20 kHz (half-us time units)
        isr::tim6_dacunder_isr(&r.s.sched(),&r.d.drive(),&r.u.duty(),
            &bench.bench(),&r.z.zct(),&mut r.h.motor(),&r.h.observer());
    }
    assert_eq!(r.d.tenkhz_counter.load(Relaxed),4);
    assert_eq!(r.h.roles.borrow().len(),1);
    assert!(!r.d.old_routine.load(Relaxed));
    assert!(!r.h.duties.borrow().is_empty());
    assert!(!r.u.killed.load(Relaxed));
}

#[test]
fn seeded_preload_delays_first_com_like_g071_measurement() {
    use minz_core::am32_hal::ComTimer;
    let r=Rig::new(1667,false);
    r.h.com_preload.set(true);
    r.h.com_active_arr.set(1999);
    (&r.h).set_and_enable(399);
    r.advance(400);
    assert!(r.h.roles.borrow().is_empty(),"requested 200 us has not fired");
    r.advance(1600);
    assert_eq!(r.h.timed_roles.borrow()[0].0,2000,"seeded active ARR gives 1000 us");
    assert_eq!(r.h.com_active_arr.get(),399,"overflow transfers preload");
}
