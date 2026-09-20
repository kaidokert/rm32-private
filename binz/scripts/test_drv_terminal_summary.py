"""Execute the actual post-stop formatter with hardware/state stubs on the host."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class TerminalSummary(unittest.TestCase):
    def test_actual_formatter(self):
        source = (ROOT / 'examples/support/driven_run.rs').read_text()
        function = source.split('pub fn terminal_summary', 1)[1].split('pub fn dump', 1)[0]
        harness = r'''
use std::fmt::Write;
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering::Relaxed};
static ACTIVE:AtomicBool=AtomicBool::new(false);
static ENABLE:AtomicBool=AtomicBool::new(false);
static APPLIED_BEMF_DUTY:AtomicU32=AtomicU32::new(80);
static TRANSFER_RESULT:AtomicU32=AtomicU32::new(0);
fn owns()->bool {ACTIVE.load(Relaxed)}
fn get_idr(_:u8,_:u8)->bool {ENABLE.load(Relaxed)}
struct State {reason:u32,stop:u32,start:u32,duty:u32,scans:usize}
static mut STATE:State=State{reason:4,stop:4266,start:223,duty:61,scans:0};
static mut ADC:[[u16;7];2]=[
    [0,0,2000,2050,2100,11000,1500],
    [1,0,1900,2080,2048,10900,1500]];
mod powered_timer {
    use super::*;
    pub static ACTIVE:AtomicBool=AtomicBool::new(false);
    pub static DISABLED:AtomicBool=AtomicBool::new(true);
    pub fn owns()->bool {ACTIVE.load(Relaxed)}
    pub fn outputs_disabled()->bool {DISABLED.load(Relaxed)}
    pub fn summary<W:Write>(out:&mut W) {writeln!(out,"POWER_SNAPSHOT").unwrap();}
}
mod core_bench {
    use super::*;
    pub fn driven_entry_summary<W:Write>(out:&mut W) {writeln!(out,"ENTRY_SNAPSHOT").unwrap();}
    pub fn terminal_summary<W:Write>(out:&mut W) {writeln!(out,"CORE_SNAPSHOT").unwrap();}
}
'''
        harness += 'pub fn terminal_summary' + function
        harness += r'''
#[test] fn formatter_cases() {
    let render=|| {let mut s=String::new();terminal_summary(&mut s);s};
    let s=render();
    assert!(s.contains("acquisition_reason=4 acquisition_us=4043"));
    assert!(s.contains("scans=0 peak_abs_raw=0 bus_min_mv=0"));
    assert!(s.contains("uncalibrated=1"));
    assert!(s.contains("ENTRY_SNAPSHOT"));
    assert!(!s.contains("POWER_SNAPSHOT"));
    assert!(!s.contains("CORE_SNAPSHOT"));
    unsafe {STATE.scans=2;}
    assert!(render().contains("scans=2 peak_abs_raw=148 bus_min_mv=10900"));
    TRANSFER_RESULT.store(2,Relaxed);
    assert!(!render().contains("POWER_SNAPSHOT"));
    assert!(!render().contains("CORE_SNAPSHOT"));
    TRANSFER_RESULT.store(1,Relaxed);
    assert!(render().contains("POWER_SNAPSHOT"));
    for flag in [&ACTIVE,&ENABLE,&powered_timer::ACTIVE] {
        flag.store(true,Relaxed);assert!(render().is_empty());flag.store(false,Relaxed);
    }
    powered_timer::DISABLED.store(false,Relaxed);assert!(render().is_empty());
}
'''
        with tempfile.TemporaryDirectory(prefix='binz-terminal-summary-') as temp:
            src = Path(temp) / 'summary.rs'
            exe = Path(temp) / 'summary.exe'
            src.write_text(harness)
            subprocess.run(['rustc', '--edition=2021', '--test', '--target',
                            'x86_64-pc-windows-msvc', '--cfg',
                            'feature="bench-driven-handoff"', str(src), '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)

    def test_only_quiet_post_coast_caller(self):
        shell = (ROOT / 'examples/shell-pwm.rs').read_text()
        compact = ''.join(shell.split())
        start = 'ifdump_pending&&drive_mode==0&&!coast&&!normal_restart_waiting{'
        block = compact.split(start, 1)[1]
        block = block.split('//DrainALLavailableUARTbytes', 1)[0]
        self.assertIn('ifobservation_count>0&&!dump_armed{driven_run::terminal_summary', block)
        self.assertEqual(shell.count('driven_run::terminal_summary'), 1)


if __name__ == '__main__':
    unittest.main()
