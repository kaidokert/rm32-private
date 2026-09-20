use drv_observer_replay::detector::{Detector,Reason};
use std::{env,fs};
mod full_polling;

fn main() {
    let args:Vec<_>=env::args().collect();
    if args.get(1).map(String::as_str)==Some("--full-polling") {
        assert_eq!(args.len(),4,"--full-polling <half-us seed interval> <obs.csv>");
        full_polling::run(&args[3],args[2].parse().expect("seed interval"));
        return;
    }
    for path in env::args().skip(1) {
        let text=fs::read_to_string(&path).expect("observation CSV");
        let mut lines=text.lines();let header:Vec<_>=lines.next().expect("header").split(',').collect();
        let col=|name:&str|header.iter().position(|v|*v==name).expect(name);
        let (step,late,reject,version,miss,age)=(col("step"),col("late_on"),col("reject"),col("wire_version"),col("neutral"),col("sector_us"));
        let mut d=Detector::default();let mut reasons=[0usize;6];let mut events=[0usize;6];
        let flags=col("flags");let mut verified=0;
        println!("CAPTURE {path}");
        for line in lines {
            let row:Vec<_>=line.split(',').collect();
            let n=|i:usize|row[i].parse::<u16>().expect("numeric column");
            assert!(n(version)>=4,"late comparator required");
            let s=n(step) as u8;
            let valid=n(reject)&11==0 && !row[late].is_empty() && (n(version)<6 || n(miss)&4==0)
                && (n(version)<7 || n(flags)&4!=0);
            let raw=row[late]=="1";
            let result=d.update(s,if n(version)>=8 {!raw}else{raw},valid);
            if n(version)>=7 {
                let expected=(result.expected as u16)|((result.armed as u16)<<1)
                    |((result.event as u16)<<2)|((result.latched as u16)<<3);
                assert_eq!((n(flags)>>5)&15,expected,"firmware decision mismatch");
                assert_eq!((n(reject)>>4)&7,result.reason as u16,"firmware reason mismatch");
                verified+=1;
            }
            reasons[result.reason as usize]+=1;
            if result.reason==Reason::Candidate {
                events[s as usize-1]+=1;
                println!("CANDIDATE step={s} confirmation_age_us={} expected={} armed={} latched={}",n(age),result.expected,result.armed,result.latched);
            }
        }
        println!("SUMMARY reasons_invalid_baseline_opposite_accumulating_candidate_latched={reasons:?} candidates_by_step={events:?}");
        println!("REPLAY verified_live_decisions={verified}");
    }
}
