"""Run the actual guarded writer body against instrumented hardware/guard seams.

This verifies ordering/refusal/publication, not guard policy or MCU timing.
"""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class LiveWriter(unittest.TestCase):
    def test_actual_writer(self):
        source = (ROOT / 'examples/support/powered_timer.rs').read_text()
        body = 'pub fn update_live_duty' + source.split('pub fn update_live_duty', 1)[1].split('pub fn feedback(', 1)[0]
        support = (ROOT / 'examples/support/live_duty.rs').as_posix()
        envelope = (ROOT / 'examples/support/duty_envelope.rs').as_posix()
        harness = r'''
use std::sync::atomic::{AtomicU32,AtomicBool,Ordering::Relaxed};
static REASON:AtomicU32=AtomicU32::new(0);
static COMMIT_MAX_US:AtomicU32=AtomicU32::new(0);
static VETO:AtomicU32=AtomicU32::new(0);
static OWN:AtomicBool=AtomicBool::new(true);
static ENABLE:AtomicBool=AtomicBool::new(true);
static FAULT_PIN:AtomicBool=AtomicBool::new(true);
static MASK:AtomicBool=AtomicBool::new(false);
static CLOCK:AtomicU32=AtomicU32::new(0);
static COST:AtomicU32=AtomicU32::new(2);
static REFUSE:AtomicBool=AtomicBool::new(false);
static mut EVENTS:Vec<u32>=Vec::new();
fn event(n:u32) {assert!(MASK.load(Relaxed));unsafe {(&mut *std::ptr::addr_of_mut!(EVENTS)).push(n);}}
fn owns()->bool {OWN.load(Relaxed)}
fn get_idr(p:u8,_:u8)->bool {if p==3 {ENABLE.load(Relaxed)}else{FAULT_PIN.load(Relaxed)}}
fn now()->u32 {assert!(MASK.load(Relaxed));1234}
fn t17()->u16 {assert!(MASK.load(Relaxed),"writer stopwatch must be masked");CLOCK.fetch_add(COST.load(Relaxed),Relaxed) as u16}
fn trip(f:powered_guard::Fault) {
    if REASON.load(Relaxed)==0 {REASON.store(f as u32,Relaxed);}
    OWN.store(false,Relaxed);ENABLE.store(false,Relaxed);
}
fn abort() {trip(powered_guard::Fault::HostAbort);}
mod cortex_m {pub mod interrupt {
    pub struct CriticalSection;
    pub fn free<R>(f:impl FnOnce(&CriticalSection)->R)->R {
        assert!(!super::super::MASK.swap(true,super::super::Relaxed));
        let result=f(&CriticalSection);
        super::super::MASK.store(false,super::super::Relaxed);result
    }
}}
mod powered_guard {
    #[derive(Clone,Copy)] pub enum Fault {TickGap=3,FeedbackStale=4,Driver=7,HostAbort=9}
}
struct Guard {fault:Option<powered_guard::Fault>,deadline:u32}
impl Guard {
    fn poll(&mut self,stamp:u32,pin:bool,abort:bool)->Option<powered_guard::Fault> {
        event(1);assert_eq!(stamp,1234);assert!(!abort);
        if !pin {Some(powered_guard::Fault::Driver)}else{self.fault}
    }
}
static mut GUARD:Option<Guard>=None;
mod core_bench {
    use super::*;
    pub static CURRENT:AtomicU32=AtomicU32::new(70);
    pub static READY:AtomicBool=AtomicBool::new(true);
    pub fn live_duty_current()->Option<u32> {if READY.load(Relaxed) {Some(CURRENT.load(Relaxed))}else{None}}
    pub fn publish_live_duty(d:u32,_:&cortex_m::interrupt::CriticalSection) {
        event(4);CURRENT.store(d,Relaxed);
    }
}
mod phase_role_live {
    use super::*;
    pub static PREPARED:AtomicU32=AtomicU32::new(70);
    pub fn prepared_matches(d:u32)->bool {d!=0 && PREPARED.load(Relaxed)==d}
    pub fn publish_live(d:u32,_:&cortex_m::interrupt::CriticalSection) {event(3);PREPARED.store(d,Relaxed);}
}
mod live_duty_hw {
    use super::*;
    pub fn update(_:live_duty::Prepared)->bool {event(2);!REFUSE.load(Relaxed)}
}
'''
        harness += f'#[path="{envelope}"] mod duty_envelope;\n#[path="{support}"] mod live_duty;\n' + body
        harness += r'''
#[test] fn actual_writer_cases() {
    // Each state starts a new mock session. All tests run serially in this test.
    for case in 0..11 {
        REASON.store(0,Relaxed);VETO.store(0,Relaxed);CLOCK.store(65530,Relaxed);
        COST.store(2,Relaxed);OWN.store(true,Relaxed);ENABLE.store(true,Relaxed);
        FAULT_PIN.store(true,Relaxed);REFUSE.store(false,Relaxed);
        core_bench::READY.store(true,Relaxed);core_bench::CURRENT.store(70,Relaxed);
        phase_role_live::PREPARED.store(70,Relaxed);
        unsafe {EVENTS=Vec::new();GUARD=Some(Guard{fault:None,deadline:98765});}
        match case {
            1=>OWN.store(false,Relaxed),
            2=>REASON.store(12,Relaxed),
            3=>ENABLE.store(false,Relaxed),
            4=>core_bench::READY.store(false,Relaxed),
            5=>phase_role_live::PREPARED.store(69,Relaxed),
            6=>unsafe {GUARD=None;},
            7=>unsafe {(&mut *std::ptr::addr_of_mut!(GUARD)).as_mut().unwrap().fault=Some(powered_guard::Fault::FeedbackStale);},
            8=>FAULT_PIN.store(false,Relaxed),
            9=>REFUSE.store(true,Relaxed),
            10=>COST.store(101,Relaxed),_=>{}
        }
        let ok=update_live_duty(live_duty::Prepared::new(3200,300).unwrap());
        assert_eq!(ok,case==0,"case {case}");assert!(!MASK.load(Relaxed));
        let events=unsafe {&*std::ptr::addr_of!(EVENTS)};
        match case {
            0|10=>assert_eq!(events,&[1,2,3,4]),
            7|8=>assert_eq!(events,&[1]),
            9=>assert_eq!(events,&[1,2]),
            _=>assert!(events.is_empty()),
        }
        if case!=0 {assert!(!owns());assert!(!ENABLE.load(Relaxed));}
        assert_eq!(REASON.load(Relaxed),match case {0=>0,2=>12,7=>4,8=>7,10=>3,_=>9});
        let expected=if case==0||case==10 {300}else{70};
        assert_eq!(core_bench::CURRENT.load(Relaxed),expected);
        unsafe {if let Some(g)=(&*std::ptr::addr_of!(GUARD)).as_ref() {assert_eq!(g.deadline,98765);}}
    }
}
'''
        with tempfile.TemporaryDirectory(prefix='binz-live-writer-') as tmp:
            src, exe = Path(tmp) / 'writer.rs', Path(tmp) / 'writer.exe'
            src.write_text(harness)
            subprocess.run(['rustc', '--edition=2021', '--test', '--target',
                            'x86_64-pc-windows-msvc', str(src), '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)


if __name__ == '__main__':
    unittest.main()
