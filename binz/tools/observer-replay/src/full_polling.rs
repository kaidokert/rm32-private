//! Recorded-level, sample-cadence reference replay. Never invent missing levels
//! or keep using a recorded floating phase after the model changes drive step.
#[path = "../tests/support/mod.rs"]
mod support;
use support::*;
use portable_atomic::Ordering::Relaxed;
use minz_core::{am32_control as ctl, am32_isr as isr, am32_loop as lp};

pub fn run(path: &str, ci: u32) {
    assert!(ci >= 2000 && ci < 32768, "polling seed interval in half-us ticks");
    let text=std::fs::read_to_string(path).expect("capture CSV");
    let mut lines=text.lines();
    let header:Vec<_>=lines.next().expect("header").split(',').collect();
    let col=|s:&str|header.iter().position(|x|*x==s).expect(s);
    let (time,age,step,float,level,flags,version,miss,reject)=
        (col("us"),col("sector_us"),col("step"),col("float"),col("late_on"),
         col("flags"),col("wire_version"),col("neutral"),col("reject"));
    let (h,s,d,u,z)=(MockHal::new(),SchedStore::default(),DriveStore::default(),
                    DutyStore::default(),ZctStore::new());
    s.commutation_interval.store(ci,Relaxed);
    s.average_interval.store(ci,Relaxed);s.last_average_interval.store(ci,Relaxed);
    s.this_zc.store(ci as u16,Relaxed);s.last_zc.store(ci as u16,Relaxed);
    s.wait_time.store((ci/4) as u16,Relaxed);
    for v in &s.interval_hist {v.store(ci,Relaxed);}
    d.running.store(true,Relaxed);d.old_routine.store(true,Relaxed);
    d.zero_crosses.store(20,Relaxed);
    u.input.store(300,Relaxed);u.adjusted_input.store(300,Relaxed);
    let mut initialized=false;let mut polls=0;let mut invalid=0;
    let mut stop="capture_end";
    println!("FULL_POLLING capture={path} seed_ci={ci} seed_zc=sector_start_minus_half_ci sample_cadence_only=1");
    for line in lines {
        let row:Vec<_>=line.split(',').collect();
        let n=|i:usize|row[i].parse::<u32>().expect("numeric CSV");
        assert!(n(version)>=7,"v7 timing/fault metadata required");
        let t=u64::from(n(time))*2;let physical=n(step) as u16;
        assert!((1..=6).contains(&physical));
        if !initialized {
            h.now.set(t);h.interval.set((n(age)*2+ci/2)&65535);
            d.current_step.store(physical,Relaxed);d.rising.store(physical&1!=0,Relaxed);
            initialized=true;
        }
        if t<h.now.get() {continue;} // Blocking reference wait consumed this row.
        h.advance((t-h.now.get()) as u32);
        let model=d.current_step.load(Relaxed);
        let expected_float=match model {1|4=>2,2|5=>0,_=>1};
        if model!=physical || n(float)!=expected_float {
            println!("DIVERGENCE us={} model_step={model} recorded_step={physical} recorded_float={}",n(time),n(float));
            stop="physical_step_or_mux_divergence";break;
        }
        if !d.old_routine.load(Relaxed) {stop="interrupt_mode_needs_edge_capture";break;}
        // Do not fabricate counter inputs from missing/invalid ON samples.
        if row[level].is_empty() || n(flags)&4==0 || n(miss)&4!=0 || n(reject)&11!=0 {
            invalid+=1;continue;
        }
        let (ss,dd,uu)=(s.sched(),d.drive(),u.duty());
        lp::min_bemf_schedule(&dd);
        let raw=row[level]=="1";
        h.comp_value.set(if n(version)>=8 {!raw}else{raw});
        h.interval_step.set(1); // Let the reference's blocking timer wait advance.
        isr::polling_bemf_check(&ss,&dd,&uu,&z.zct(),&mut h.motor(),&h.observer(),d.running.load(Relaxed));
        h.interval_step.set(0);polls+=1;
        let ect=ss.intervals().e_com_time();
        let avg=lp::store_average_interval(&ss,ect);
        ctl::desync_check_band(&ss,&dd,&uu,&h.observer(),avg);
        let (running,zc)=lp::filter_and_duty_max(&ss,&dd,&uu,ect,avg);
        lp::bemf_timeout_resets(&dd,&uu,zc);
        ctl::bemf_timeout_rekick(&ss,&dd,&uu,&z.zct(),&mut h.motor(),&h.observer(),running);
    }
    for &(tick,step) in h.timed_roles.borrow().iter() {
        println!("REFERENCE_COM us={} step={step}",tick as f64/2.0);
    }
    println!("RESULT polls={polls} invalid={invalid} com={} average={} desync={} recovery={} stop={stop} lock_proven=0",
        h.roles.borrow().len(),s.average_interval.load(Relaxed),d.desync_happened.load(Relaxed),d.bemf_timeout_happened.load(Relaxed));
}
