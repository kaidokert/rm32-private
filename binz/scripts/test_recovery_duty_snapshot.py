"""Actual resume wrapper with host seams; no hardware/timing qualification."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]


class RecoveryDutySnapshot(unittest.TestCase):
    def test_actual_resume_wrapper(self):
        source=(ROOT/'examples/support/core_bench.rs').read_text()
        body='fn resume_once'+source.split('fn resume_once',1)[1].split('/// Perform timer/counter',1)[0]
        harness=r'''
use std::sync::atomic::{AtomicU32,Ordering::Relaxed};
static REENTRY_RESERVE:AtomicU32=AtomicU32::new(0);
static POWER_DUTY:AtomicU32=AtomicU32::new(0);
static POWER_WINDOW_US:AtomicU32=AtomicU32::new(0);
static DROP_AT:AtomicU32=AtomicU32::new(2000);
static PREPARES:AtomicU32=AtomicU32::new(0);
static ARMED:AtomicU32=AtomicU32::new(0);
static STAGED:AtomicU32=AtomicU32::new(0);
static mut REENTRY_REPORT:[u32;6]=[0;6];
struct Archive;
impl Archive {fn get(&self)->Option<()> {Some(())}}
static FIRST_SEGMENT:Archive=Archive;
mod duty_split {pub const MAX:u32=100;}
mod powered_guard {
    pub enum Fault {Tracking}
    pub struct SessionBudget {pub original_end:u32}
    #[derive(Clone,Copy)] pub struct Limits {pub elapsed:u32,pub segment:u32}
    impl SessionBudget {
        pub fn reentry_reserved<const R:u32>(&mut self,elapsed:u32,_:Fault)->Result<Limits,()> {
            Ok(Limits{elapsed:elapsed+R,segment:self.original_end.checked_sub(elapsed+R).ok_or(())?})
        }
    }
}
mod powered_timer {
    pub fn reason()->u32 {8}
    pub fn prepare() {super::PREPARES.fetch_add(1,super::Relaxed);}
    pub fn wake(abort:&mut dyn FnMut()->bool)->bool {!abort()}
}
mod flying_bench {
    pub struct Seed {pub step:u8}
    pub fn reacquire_awake()->Option<(Seed,u16)> {Some((Seed{step:3},25))}
    pub fn reacquire_prepared(duty:u32)->Option<(Seed,u16)> {
        assert_eq!(super::POWER_DUTY.load(super::Relaxed),0);
        super::STAGED.store(duty,super::Relaxed);reacquire_awake()
    }
}
fn bridge_disabled()->bool {true}
fn clock_us()->u32 {3000}
fn observe_begin(_:u8,_:u32,_:u32) {POWER_DUTY.store(0,Relaxed);}
fn gates_off() {}
fn set_pin(_:u8,_:u8,_:bool) {}
fn coast_run_inner(_:u8,_:u32,_:Option<(flying_bench::Seed,u16)>,
    _:Option<(u32,u32,&mut dyn FnMut()->bool)>,limits:Option<powered_guard::Limits>,_:Option<()>)->bool {
    let limits=limits.unwrap();
    assert_eq!(limits.elapsed+limits.segment,10000);
    ARMED.store(POWER_DUTY.load(Relaxed),Relaxed);true
}
fn reset(duty:u32) {POWER_DUTY.store(duty,Relaxed);PREPARES.store(0,Relaxed);ARMED.store(0,Relaxed);}
fn budget()->Option<powered_guard::SessionBudget> {Some(powered_guard::SessionBudget{original_end:10000})}
#[test] fn retained_actual_duty_survives_controller_reset() {
    for duty in 40..=100 {
        reset(duty);
        assert_eq!(resume_once::<300>(1000,1662,&mut ||false,budget()),7);
        assert_eq!(ARMED.load(Relaxed),duty);
        assert_eq!(POWER_WINDOW_US.load(Relaxed),7700);
        assert_eq!(PREPARES.load(Relaxed),1);
        #[cfg(feature="bench-reentry-pwm-stage")]
        assert_eq!(STAGED.load(Relaxed),duty);
    }
}
#[test] fn unsupported_target_never_clamps_or_prepares() {
    for duty in [0,39,101,200,300,301,u32::MAX] {
        reset(duty);
        assert_eq!(resume_once::<300>(1000,1662,&mut ||false,budget()),8);
        assert_eq!(PREPARES.load(Relaxed),0);assert_eq!(ARMED.load(Relaxed),0);
    }
}
#[test] fn abort_and_missing_budget_still_refuse_before_preparation() {
    reset(80);assert_eq!(resume_once::<300>(1000,1662,&mut ||true,budget()),2);
    assert_eq!(PREPARES.load(Relaxed),0);
    reset(80);assert_eq!(resume_once::<300>(1000,1662,&mut ||false,None),3);
    assert_eq!(PREPARES.load(Relaxed),0);
}
'''
        with tempfile.TemporaryDirectory() as temp:
            src=Path(temp)/'snapshot.rs';exe=Path(temp)/'snapshot.exe'
            src.write_text(harness+body)
            for flags in [[], ['--cfg', 'feature="bench-reentry-pwm-stage"']]:
                subprocess.run(['rustc','--edition=2021','--test',*flags,str(src),'-o',str(exe)],check=True)
                subprocess.run([str(exe),'--test-threads=1'],check=True)


if __name__=='__main__':unittest.main()
