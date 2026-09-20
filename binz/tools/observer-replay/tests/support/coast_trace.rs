//! Measured all-phase bridge-disabled samples through a virtual controller.
//! Sample-and-hold sensitivity study, NOT edge-resolution hardware replay.
use super::*;
use std::path::PathBuf;

fn measured_samples() -> Vec<(u64,usize,bool)> {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../captures");
    let raw=std::fs::read_to_string(root.join("bemf_acceptlog_200_01.txt")).unwrap();
    let csv=std::fs::read_to_string(root.join("bemf_acceptlog_200_01_coast.csv")).unwrap();
    let mut lines=csv.lines();
    let header:Vec<_>=lines.next().unwrap().split(',').collect();
    let col=|name|header.iter().position(|&s|s==name).unwrap();
    let rows:Vec<Vec<_>>=lines.map(|line|line.split(',').collect()).collect();
    let mut samples=Vec::new();
    for line in raw.lines().filter(|line|line.starts_with("COASTCOMP ")) {
        let value=|key:&str|->usize {
            line.split_whitespace().find_map(|s|s.strip_prefix(key))
                .unwrap().parse().unwrap()
        };
        let index=value("row=");
        let row=&rows[index];
        let origin=row[col("elapsed_us")].parse::<u64>().unwrap();
        for (phase,(name,offset)) in [("comp_a","a_us="),("comp_b","b_us="),("comp_c","c_us=")].into_iter().enumerate() {
            let raw_level=match row[col(name)] {"True"=>true,"False"=>false,_=>panic!("boolean")};
            samples.push(((origin+value(offset) as u64)*2,phase,raw_level));
        }
    }
    samples.sort_by_key(|s|s.0);
    samples
}

#[test]
fn coast_three_phase_sample_hold_reports_all_seed_outcomes() {
    let samples=measured_samples();
    assert_eq!(samples.len(),96,"only32 rows have measured comparator offsets");
    assert_eq!(samples[0],((503+191)*2,0,true));
    assert!(samples.windows(2).all(|w|w[1].0>w[0].0));
    for seed in 1..=6 {
        let r=Rig::new(1666,false);
        r.d.current_step.store(seed,Relaxed);
        r.d.rising.store(seed&1!=0,Relaxed);
        let mut levels=[None;3];
        let mut initialized=false;
        let mut stop="sample_end";
        for &(tick,phase,raw) in &samples {
            if !initialized {
                levels[phase]=Some(!raw); // physical raw->reference polarity
                if levels.iter().any(Option::is_none) {continue;}
                r.h.now.set(tick);
                r.h.interval.set(418); // explicit assumed prior accepted input
                let floating=[2,0,1,2,0,1][seed as usize-1];
                r.h.comp_value.set(levels[floating].unwrap());
                initialized=true;
                continue; // initial level is not a fabricated edge
            }
            r.advance((tick-r.h.now.get()) as u32); // COM at actual model deadline
            if !r.d.running.load(Relaxed) {stop="reference_stopped";break;}
            if r.d.old_routine.load(Relaxed) {stop="polling_changeover";break;}
            levels[phase]=Some(!raw);
            let step=r.d.current_step.load(Relaxed);
            let floating=[2,0,1,2,0,1][step as usize-1];
            let level=levels[floating].unwrap();
            let previous=r.h.comp_value.replace(level);
            if level!=previous && level==r.d.rising.load(Relaxed) {
                r.h.pending.set(true);
            }
            // Service at sample cadence only, with held comparator levels.
            // Mux changes between samples and sub-sample edges are unresolved.
            r.irq();r.bands();
        }
        let accepted=r.h.timed_events.borrow().iter()
            .filter(|e|e.1==minz_core::blackbox::EV_ACC).count();
        println!("COAST_HOLD seed={seed} accepts={accepted} com={} avg={} desync={} stop={stop} sample_hold=1 lock_proven=0",
            r.h.roles.borrow().len(),r.s.average_interval.load(Relaxed),r.d.desync_happened.load(Relaxed));
        // Structural invariants only: do not bless one favorable seed as lock.
        assert!(r.h.roles.borrow().len()<=accepted);
        assert!(r.h.timed_roles.borrow().windows(2).all(|w|w[1].1==w[0].1%6+1));
    }
}

#[test]
fn measured_sample_hold_startup_sensitivity() {
    let samples=measured_samples();
    let begin=samples[2].0; // allthree initialphase samples available
    let end=samples.last().unwrap().0;
    for seed in 1..=6 {
        let r=Rig::new(1666,true);
        *r.h.phase_samples.borrow_mut()=samples.iter().map(|&(t,p,v)|(t,p,!v)).collect();
        r.h.advance(begin as u32);
        r.d.current_step.store(seed,Relaxed);
        r.d.running.store(false,Relaxed);r.d.zero_crosses.store(0,Relaxed);
        ctl::start_motor(&r.s.sched(),&r.d.drive(),&mut r.h.motor(),&r.h.observer());
        r.h.interval_step.set(1); // 0.5us perread; modeled CPU cost,not measured
        let mut next_poll=r.h.now.get()+100;
        let mut first_irq=0;
        while r.h.now.get()<end && r.d.running.load(Relaxed) {
            r.advance(2);
            if r.h.now.get()>=next_poll {
                r.poll();
                next_poll=(r.h.now.get()/100+1)*100;
            }
            if !r.d.old_routine.load(Relaxed) && first_irq==0 {first_irq=r.h.now.get()-begin;}
            r.irq();r.bands();
        }
        println!("COAST_START_HOLD seed={seed} first_irq_us={} com={} average={} desync={} running={} sample_hold=1 lock_proven=0",
            first_irq/2,r.h.roles.borrow().len(),r.s.average_interval.load(Relaxed),
            r.d.desync_happened.load(Relaxed),r.d.running.load(Relaxed));
        assert_eq!(r.d.zcfr_guard_hits.load(Relaxed),0);
        assert!(r.h.timed_roles.borrow().windows(2).all(|w|w[1].1==w[0].1%6+1));
    }
}
