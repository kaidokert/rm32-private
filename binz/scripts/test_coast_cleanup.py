"""Exercise actual foreground wrapper with a stubbed inner body, not hardware."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from scripts.test_recovery_budget import function


class Cleanup(unittest.TestCase):
    def test_all_return_values_safe_before_adc_restore(self):
        root = Path(__file__).resolve().parents[1]
        source = (root/'examples/support/core_bench.rs').read_text()
        wrapper = function(source.replace('fn coast_run_inner(', 'pub fn coast_run_inner('), 'coast_run_inner')
        harness = wrapper + r'''
mod flying_acquire {pub struct Seed;}
mod powered_guard {pub struct Limits;pub struct Feedback;}
static mut ORDER:u32=0;
static mut RESULT:bool=false;
fn coast_run_body(_:u8,_:u32,_:Option<(flying_acquire::Seed,u16)>,
 _:Option<(u32,u32,&mut dyn FnMut()->bool)>,_:Option<powered_guard::Limits>,
 _:Option<(powered_guard::Feedback,u16)>)->bool {unsafe {ORDER=1;RESULT}}
fn gates_off() {unsafe {assert_eq!(ORDER,1);ORDER=2;}}
fn set_pin(p:u8,n:u8,on:bool) {unsafe {assert_eq!((p,n,on),(3,1,false));assert_eq!(ORDER,2);ORDER=3;}}
mod adc_stream {pub fn stop() {unsafe {assert_eq!(super::ORDER,3);super::ORDER=4;}}}
fn main() {unsafe {for result in [false,true] {
 RESULT=result;ORDER=0;
 assert_eq!(coast_run_inner(1,200,None,None,None,None),result);
 assert_eq!(ORDER,4);
}}}
'''
        with tempfile.TemporaryDirectory() as temp:
            src, exe = Path(temp)/'cleanup.rs', Path(temp)/'cleanup.exe'
            src.write_text(harness)
            subprocess.run(['rustc','--edition=2021','--cfg','feature="bench-dma-feedback"',
                            str(src),'-o',str(exe)],check=True)
            subprocess.run([str(exe)],check=True)


if __name__ == '__main__':
    unittest.main()
