"""Actual publication wrapper with mocked hardware; not register timing proof."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from scripts.test_recovery_budget import function

ROOT=Path(__file__).resolve().parents[1]


class PreparedRecoveryAdapter(unittest.TestCase):
    def test_counter_activation_checks_preserved_configuration(self):
        source=(ROOT/'examples/support/com_timer.rs').read_text()
        body=function(source,'prepare_recovery_counter')
        body=body.replace('let t=&*stm32::TIM16::ptr();','')
        for index,name in enumerate(('cr1','dier','sr','psc','arr')):
            body=body.replace(f't.{name}().read().bits()',f'read({index})')
        body=body.replace('t.cnt().write(|w|w.bits(0));','write(5,0);')
        body=body.replace('t.cr1().write(|w|w.bits(1));','write(0,1);')
        harness='use std::sync::atomic::{AtomicBool,Ordering::Relaxed};\n'+body+r'''
static RECOVERY_PREPARED:AtomicBool=AtomicBool::new(false);
static mut REGS:[u32;6]=[0,0,0,31,65535,999];
static mut WRITES:u32=0;static mut MASK:bool=false;static mut FLAGS:u8=0;
fn read(i:usize)->u32{unsafe{assert!(MASK);REGS[i]}}
fn write(i:usize,v:u32){unsafe{assert!(MASK);REGS[i]=v;WRITES+=1;}}
mod core_bench{pub fn active()->bool{unsafe{super::FLAGS&8!=0}}}
mod powered_timer {
 pub fn owns()->bool{unsafe{assert!(super::MASK);super::FLAGS&1!=0}}
 pub fn ready()->bool{unsafe{super::FLAGS&2!=0}}
 pub fn outputs_disabled()->bool{unsafe{super::FLAGS&4!=0}}
}
mod cortex_m{pub mod interrupt{pub fn free<R>(f:impl FnOnce(&())->R)->R{unsafe{
 assert!(!super::super::MASK);super::super::MASK=true;
 let r=f(&());super::super::MASK=false;r
}}}}
fn main(){unsafe{
 for flags in 0..16{for bad in 0..6{
  FLAGS=flags;REGS=[0,0,0,31,65535,999];WRITES=0;
  if bad<5{REGS[bad]^=1;}
  RECOVERY_PREPARED.store(false,Relaxed);
  let expected=flags==7&&bad==5;
  assert_eq!(prepare_recovery_counter(),expected);
  assert_eq!(RECOVERY_PREPARED.load(Relaxed),expected);
  assert_eq!(WRITES,if expected{2}else{0});assert!(!MASK);
  if expected{assert_eq!(REGS,[1,0,0,31,65535,0]);}
 }}
}}
'''
        with tempfile.TemporaryDirectory() as directory:
            src=Path(directory)/'activate.rs';exe=Path(directory)/'activate.exe'
            src.write_text(harness)
            subprocess.run(['rustc',str(src),'-o',str(exe)],check=True)
            subprocess.run([str(exe)],check=True)

    def test_one_shot_publication_and_revocation(self):
        source=(ROOT/'examples/support/com_timer.rs').read_text()
        body=function(source,'publish_recovery_deadline')
        body=body.replace('NVIC::mask(stm32::Interrupt::TIM16)','mask()')
        body=body.replace('(*stm32::TIM16::ptr()).cnt().read().bits() as u16','COUNTER')
        body=body.replace('cortex_m::Peripherals::steal().NVIC.set_priority(stm32::Interrupt::TIM16,0)','PRIORITY=0')
        harness='use std::sync::atomic::{AtomicBool,Ordering::Relaxed};\n'
        harness+=f'#[path="{(ROOT/"examples/support/prepared_handoff.rs").as_posix()}"] mod prepared_policy;\n'
        harness+=body+r'''
static RECOVERY_PREPARED:AtomicBool=AtomicBool::new(false);
static mut FLAGS:u8=0;static mut COUNTER:u16=100;
static mut STOPPED:bool=false;static mut MASKED:bool=false;
static mut PUBLISHED:u32=0;static mut PRIORITY:u8=128;
static mut CODE:u32=0;
fn t17()->u16 {10}
fn mask(){unsafe{MASKED=true;}}
mod powered_timer {
 pub fn owns()->bool {unsafe {super::FLAGS&1!=0}}
 pub fn ready()->bool {unsafe {super::FLAGS&2!=0}}
 pub fn outputs_disabled()->bool {unsafe {super::FLAGS&4!=0}}
}
struct Timer;
impl Timer {fn stop(){RECOVERY_PREPARED.store(false,Relaxed);unsafe{STOPPED=true;}}}
fn publish_prepared(_:u16)->u32 {unsafe {
 assert!(MASKED);PUBLISHED+=1;if CODE!=0 {Timer::stop();}CODE
}}
fn main(){unsafe {
 for prepared in [false,true] {for flags in 0..8 {for code in [0,6] {
  RECOVERY_PREPARED.store(prepared,Relaxed);FLAGS=flags;CODE=code;
  STOPPED=false;MASKED=false;PUBLISHED=0;PRIORITY=128;
  let result=publish_recovery_deadline(0,200);
  let eligible=prepared && flags==7;
  assert_eq!(result,eligible && code==0);
  assert_eq!(PUBLISHED,eligible as u32);
  assert!(!RECOVERY_PREPARED.load(Relaxed));
  assert_eq!(STOPPED,!result);
  if result {assert_eq!(PRIORITY,0);assert!(!publish_recovery_deadline(0,200));}
 }}}
 FLAGS=7;CODE=0;PUBLISHED=0;STOPPED=false;RECOVERY_PREPARED.store(true,Relaxed);
 assert!(!publish_recovery_deadline(0,10)); // original deadline already late
 assert!(STOPPED);assert_eq!(PUBLISHED,0);
}}
'''
        with tempfile.TemporaryDirectory() as directory:
            src=Path(directory)/'adapter.rs';exe=Path(directory)/'adapter.exe'
            src.write_text(harness)
            subprocess.run(['rustc',str(src),'-o',str(exe)],check=True)
            subprocess.run([str(exe)],check=True)
