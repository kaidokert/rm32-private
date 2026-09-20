"""Actual follow-loop body with explicit simulated clock, COMP and owner seams.

No timing/WCET or physical mux-settling claim. Pure acquisition/filter code is real.
"""
from pathlib import Path
import subprocess
import tempfile
import unittest
from scripts.test_recovery_budget import function

ROOT = Path(__file__).resolve().parents[1]


class FollowLive(unittest.TestCase):
    def test_real_loop(self):
        source = (ROOT / 'examples/support/flying_bench.rs').read_text()
        declarations = source[source.index('struct FollowContext {'):source.index('/// Consume the exact')]
        body = function(source, 'follow_prepared') + function(
            source.replace('fn follow_sweep(', 'pub fn follow_sweep('), 'follow_sweep')
        body += function(source.replace('fn continue_filters(', 'pub fn continue_filters('), 'continue_filters')
        body = body.replace('core::ptr::write_volatile(COMP2_CSR,', 'write_comp(')
        body = body.replace('core::ptr::read_volatile(COMP2_CSR)', 'read_comp()')
        body = body.replace('let pending=c.next.prepare_candidate',
                            'let pending={unsafe {PREVALIDATIONS+=1;} c.next.prepare_candidate')
        body = body.replace('prepare_candidate(phase,&c.filters[phase]);',
                            'prepare_candidate(phase,&c.filters[phase])};')
        failure = function(source.replace('fn follow_failure(', 'pub fn follow_failure('), 'follow_failure')
        harness = 'use std::sync::atomic::{AtomicBool,AtomicU32,Ordering::Relaxed};\n'
        harness += f'#[path="{(ROOT / "examples/support/flying_acquire.rs").as_posix()}"] mod flying_acquire;\n'
        harness += declarations + body + failure + r'''
static mut TIME:u16=0;
static mut PREVALIDATIONS:u32=0;
static mut ORIGIN:u16=0;
static mut CSR:u32=0;
static mut OWN:bool=true;
static mut ENABLE:bool=true;
static mut CASE:u8=0;
static mut NEXT_PHASE:u32=0;
static mut NEXT_LEVEL:bool=false;
static mut LEVELS:[bool;3]=[false;3];
static mut FEEDBACK:u32=0;
static mut MASK:bool=false;
mod adc_stream {pub fn completed_scans()->u32 {
    // Every counter endpoint must share an IRQ-free observation with TIM17.
    unsafe {assert!(super::MASK);}0
}}
fn t17()->u16 {unsafe {let t=TIME;TIME=TIME.wrapping_add(1);t}}
fn clock_us()->u32 {unsafe {assert!(TIME.wrapping_sub(ORIGIN)<25000,"unbounded loop");TIME as u32}}
unsafe fn write_comp(value:u32) {CSR=value;}
unsafe fn read_comp()->u32 {
    let phase=((CSR>>4)&15).wrapping_sub(6);
    assert!(phase<3);
    let tick=TIME.wrapping_sub(ORIGIN) as u32*2;
    if CASE==9 && tick>=13000 {cancel_follow();}
    let change=if CASE==6 {(NEXT_PHASE+1)%3}else{NEXT_PHASE};
    let level=if tick>=13000 && phase==change && CASE!=7 {
        if CASE==6 {!LEVELS[phase as usize]}else{NEXT_LEVEL}
    }else{LEVELS[phase as usize]};
    if level {0}else{1<<30}
}
fn gates_off() {cancel_follow();unsafe {OWN=false;}}
fn set_pin(port:u8,pin:u8,on:bool) {assert_eq!((port,pin,on),(3,1,false));unsafe {ENABLE=false;}}
mod cortex_m {pub mod interrupt {
    pub fn free<R>(f:impl FnOnce(&())->R)->R {unsafe {
        assert!(!super::super::MASK);super::super::MASK=true;
        let r=f(&());super::super::MASK=false;r
    }}
}}
mod powered_timer {
    pub fn owns()->bool {unsafe {super::OWN}}
    pub fn ready()->bool {unsafe {super::ENABLE}}
    pub fn outputs_disabled()->bool {true}
    pub fn service_feedback(_:u32)->bool {unsafe {
        super::FEEDBACK+=1;
        if super::CASE==4 {super::gates_off();return false;}true
    }}
}
fn main() {unsafe {
    // A continuation must carry a real earlier candidate onset, not its return
    // timestamp. Gap refusal and owner checks also use the actual helper body.
    for origin in [0u16,60000] {
        CASE=0;ORIGIN=origin;OWN=true;ENABLE=true;TIME=origin.wrapping_add(6520);
        NEXT_PHASE=0;NEXT_LEVEL=false;LEVELS=[true,false,true];
        let mut filters=core::array::from_fn(|_|flying_acquire::EdgeFilter::new());
        for p in 0..3 {filters[p].sample(LEVELS[p],12980).unwrap();}
        filters[0].sample(false,13000).unwrap();
        let (phase,level,onset,confirmed,proof)=continue_filters(&mut filters,origin,0,0).unwrap().unwrap();
        assert_eq!((phase,level,onset),(0,false,13000));assert!(confirmed>=13040);
        let mut a=flying_acquire::RuntimeAcquire::new(0);
        for i in 0..=12u32 {
            let step=(i%6+1) as u8;let p=[2,0,1,2,0,1][(step-1) as usize];
            a.edge(p,step&1!=0,i*1000).unwrap();
        }
        let mut next=a.into_next_edge(confirmed+20).unwrap();
        let followed=if let Some(proof)=proof {
            next.prepare_edge(phase,level,onset).finish_persistent(phase,level,onset,confirmed,proof).unwrap()
        }else{next.edge(phase,level,onset,confirmed).unwrap()};
        assert_eq!(followed.seed,flying_acquire::Seed{step:2,edge_tick:13000,interval_ticks:1000});
        TIME=TIME.wrapping_add(200);
        assert_eq!(continue_filters(&mut filters,origin,0,0),Err(5));
        ENABLE=false;
        assert_eq!(continue_filters(&mut filters,origin,0,0),Err(4));
    }
    let mut cases=0;
    for step in 1..=6u8 {for origin in [0u16,60000] {for case in 0..=10u8 {
        CASE=case;ORIGIN=origin;TIME=origin.wrapping_add(6020);
        OWN=true;ENABLE=true;FEEDBACK=0;FOLLOW_REPORT=[0;8];
        let mut a=flying_acquire::RuntimeAcquire::new(0);
        let mut seed=None;
        for i in 0..=12u32 {
            let s=((step as u32-1+i)%6+1) as u8;
            let p=[2,0,1,2,0,1][(s-1) as usize];
            LEVELS[p as usize]=s&1!=0;
            seed=a.edge(p,s&1!=0,i*1000).unwrap();
        }
        let prior=seed.unwrap();
        let next=if step==6 {1}else{step+1};
        NEXT_PHASE=[2,0,1,2,0,1][(next-1) as usize];NEXT_LEVEL=next&1!=0;
        let mut filters=core::array::from_fn(|_|flying_acquire::EdgeFilter::new());
        for p in 0..3 {filters[p].sample(LEVELS[p],12040).unwrap();}
        FOLLOW=Some(FollowContext{next:a.into_next_edge(12040).unwrap(),filters,origin,saved:0,
            completed:None,phase:NEXT_PHASE as usize});
        FOLLOW_VALID.store(true,Relaxed);
        FOLLOW_REPORT=[0,prior.step as u32,prior.edge_tick,0,0,0,0,0];
        if case==1 {cancel_follow();}
        if case==2 {TIME=origin.wrapping_add(6200);}
        if case==5 {OWN=false;}
        let mut setup_ok=true;
        let mut cached=None;
        if cfg!(feature="bench-follow-direct") && case==0 {
            assert!(setup_follow(3));assert_eq!(FOLLOW_READS,if cfg!(feature="bench-follow-setup-phase") {1}else{3});
            assert_eq!(((CSR>>4)&15)-6,NEXT_PHASE);
        }
        if (case==0 || case==2) && !cfg!(feature="bench-follow-direct") {
            setup_ok=setup_follow(4);
            assert_eq!((&*core::ptr::addr_of!(FOLLOW)).as_ref().unwrap().phase,NEXT_PHASE as usize);
            if case==0 {
                assert!(setup_ok);assert_eq!(FOLLOW_READS,if cfg!(feature="bench-prepared-handoff") {1}else{4});
                assert_eq!(((CSR>>4)&15)-6,NEXT_PHASE);
                assert_eq!(FEEDBACK,0);
            } else {assert!(!setup_ok);}
        }
        if case>=8 {
            OWN=false; // setup has no powered guard/ADC owner yet
            loop {
                setup_ok=setup_follow(1);
                if !setup_ok {break;}
                cached=(&*core::ptr::addr_of!(FOLLOW)).as_ref().unwrap().completed;
                if cached.is_some() {break;}
            }
            assert_eq!(FEEDBACK,0); // comparator-only checkpoints
            if setup_ok {
                OWN=true;
                if case==10 {TIME=TIME.wrapping_add(300);}
            }
        }
        let result=if setup_ok {follow_prepared(prior,origin,1500,&mut ||case==3)}else{None};
        assert_eq!(result.is_some(),matches!(case,0|8|10),"step {step} case {case}");
        assert!(!FOLLOW_VALID.load(Relaxed));assert!(!MASK);
        if matches!(case,0|8|10) {
            let seed=result.unwrap();assert_eq!(seed.step,next);assert_eq!(seed.interval_ticks,1000);
            assert!(seed.edge_tick>=13000 && seed.edge_tick<13200);
            assert_eq!(FOLLOW_REPORT[4],seed.edge_tick);
            let minimum=if cfg!(feature="bench-follow-persistence") {0}else{40};
            assert!((minimum..=240).contains(&(FOLLOW_REPORT[5]-seed.edge_tick)));
            assert!(OWN && ENABLE);
            if cfg!(feature="bench-dma-guard") && case>=8 {assert_eq!(FEEDBACK,0);}
            else {assert!(FEEDBACK>0);}
            if case>=8 {assert_eq!(cached,Some(seed));}
            if case==10 {
                assert!(seed.handoff_with_min::<834>(TIME.wrapping_sub(origin) as u32*2,250).is_none(),
                        "remaining setup must not refresh a cached physical onset");
            }
        } else {
            assert!(!OWN && !ENABLE);
            assert_eq!(FOLLOW_REPORT[0],match case {1=>2,2=>5,3=>3,4|5|9=>4,
                6=>if cfg!(feature="bench-follow-expected-phase") {7}else{6},7=>7,_=>0});
            if case==2 {assert!(FOLLOW_REPORT[7]>200);}
        }
        cases+=1;
    }}}
    if cfg!(feature="bench-follow-prevalidate") && !cfg!(feature="bench-follow-persistence") {assert!(PREVALIDATIONS>0);}
    else {assert_eq!(PREVALIDATIONS,0);}
    println!("FOLLOW_LIVE simulated cases={cases} PASS (not hardware timing)");
}}
'''
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            src, exe = path / 'follow.rs', path / 'follow.exe'
            src.write_text(harness)
            for mode, (expected, dma_guard, prepared, prevalidate, direct, setup_phase) in enumerate(((False,False,False,False,False,False),(True,False,False,False,False,False),(True,True,False,False,False,False),(True,True,True,False,False,False),(True,True,True,True,False,False),(True,True,True,True,True,False),(True,True,True,True,True,True),(True,True,True,True,True,True))):
                subprocess.run(['rustc', '--edition=2021', '--cfg', 'feature="bench-reentry-next-edge"',
                                '--cfg', 'feature="bench-reentry-next-edge-live"',
                                '--cfg', 'feature="bench-range350"', '--cfg', 'feature="bench-seed400"',
                                *(['--cfg','feature="bench-follow-expected-phase"'] if expected else []),
                                *(['--cfg','feature="bench-dma-guard"'] if dma_guard else []),
                                *(['--cfg','feature="bench-prepared-handoff"'] if prepared else []),
                                *(['--cfg','feature="bench-follow-prevalidate"'] if prevalidate else []),
                                *(['--cfg','feature="bench-follow-direct"'] if direct else []),
                                *(['--cfg','feature="bench-follow-setup-phase"'] if setup_phase else []),
                                *(['--cfg','feature="bench-follow-persistence"'] if mode==7 else []),
                                str(src), '-o', str(exe)], check=True)
                subprocess.run([str(exe)], check=True)


if __name__ == '__main__':
    unittest.main()
