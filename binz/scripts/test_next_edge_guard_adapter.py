"""Actual adapter body + real guard policy; hardware/time reads are explicit seams."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from scripts.test_recovery_budget import function

ROOT=Path(__file__).resolve().parents[1]


class NextGuardAdapter(unittest.TestCase):
    def test_owner_and_output_exclusion(self):
        body=function((ROOT/'examples/support/powered_timer.rs').read_text(),'follow_seed')
        harness='\n'.join(f'#[path="{(ROOT/"examples/support"/(name+".rs")).as_posix()}"] mod {name};'
                          for name in ['accepted_timing','powered_guard'])+r'''
use std::sync::atomic::{AtomicBool,AtomicU32,Ordering::Relaxed};
static OWN:AtomicBool=AtomicBool::new(false);
static OFF:AtomicBool=AtomicBool::new(true);
static PIN:AtomicBool=AtomicBool::new(true);
static MASK:AtomicBool=AtomicBool::new(false);
static REASON:AtomicU32=AtomicU32::new(0);
static COMMITS:AtomicU32=AtomicU32::new(0);
static mut GUARD:Option<powered_guard::RunGuard<2223,100,true>>=None;
fn owns()->bool {OWN.load(Relaxed)}
fn reason()->u32 {REASON.load(Relaxed)}
fn outputs_disabled()->bool {OFF.load(Relaxed)}
fn now()->u32 {assert!(MASK.load(Relaxed));100}
fn get_idr(port:u8,pin:u8)->bool {assert!(MASK.load(Relaxed));assert_eq!((port,pin),(1,14));PIN.load(Relaxed)}
fn trip(f:powered_guard::Fault) {
    assert!(!MASK.load(Relaxed));
    if reason()==0 {REASON.store(f as u32+1,Relaxed);}
    OWN.store(false,Relaxed);OFF.store(true,Relaxed);
}
mod cortex_m {pub mod interrupt {
    pub fn free<R>(f:impl FnOnce(&())->R)->R {
        assert!(!super::super::MASK.swap(true,super::super::Relaxed));
        let r=f(&());super::super::MASK.store(false,super::super::Relaxed);r
    }
}}
#[test] fn wrapper_cases() {
    for case in 0..10 {
        OWN.store(true,Relaxed);OFF.store(true,Relaxed);PIN.store(true,Relaxed);
        REASON.store(0,Relaxed);COMMITS.store(0,Relaxed);
        unsafe {GUARD=Some(powered_guard::RunGuard::with_limits(0,1000,1,
            powered_guard::Feedback{phase:[2048;3],vref:1500,bus_mv:11700},100000,20000).unwrap());}
        match case {
            1=>OWN.store(false,Relaxed),2=>REASON.store(99,Relaxed),
            3=>OFF.store(false,Relaxed),4=>COMMITS.store(1,Relaxed),
            5=>unsafe {GUARD=None;},6=>PIN.store(false,Relaxed),_=>{}
        }
        let mut ok=follow_seed(1,if case==8 {3}else{2},case==7);
        if case==9 {assert!(ok);ok=follow_seed(2,3,false);}
        assert_eq!(ok,case==0,"case {case}");
        assert!(outputs_disabled());assert!(!MASK.load(Relaxed));
        if case!=0 {assert!(!owns());assert_ne!(reason(),0);}
        if case==2 {assert_eq!(reason(),99);}
        assert_eq!(COMMITS.load(Relaxed),if case==4 {1}else{0});
    }
}
'''
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory);src=path/'test.rs';exe=path/'test.exe'
            src.write_text(harness+body)
            subprocess.run(['rustc','--edition=2021','--test','--cfg',
                            'feature="bench-reentry-next-edge"',str(src),'-o',str(exe)],check=True)
            subprocess.run([str(exe),'--test-threads=1'],check=True)


if __name__=='__main__':unittest.main()
