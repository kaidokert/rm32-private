"""Actual publisher/conversion with host guard seams; not an ISR timing proof."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DmaGuardDelivery(unittest.TestCase):
    def test_explicit_wire_mode(self):
        from drv_dma_guard import verify
        for line, mode in [('DMAFEEDBACK trigger_us=201', False),
                           ('DMAFEEDBACK guard_irq=0', False),
                           ('DMAFEEDBACK guard_irq=1', True)]:
            self.assertEqual(verify(line, mode), mode)
            with self.assertRaises(ValueError):
                verify(line, not mode)
        for line in ['', 'DMAFEEDBACK guard_irq=2', 'DMAFEEDBACK guard_irq=1 guard_irq=1']:
            with self.assertRaises(ValueError):
                verify(line, True)

    def test_publisher(self):
        source = (ROOT / 'examples/support/powered_timer.rs').read_text()
        publisher = 'pub fn stream_feedback' + source.split('pub fn stream_feedback', 1)[1].split('#[cfg(feature="bench-dma-feedback")]', 1)[0]
        publisher = publisher.replace('unsafe {core::ptr::read_volatile(VREFINT_CAL_ADDR as *const u16)} as u32', '1500')
        convert = 'pub fn convert' + source.split('pub fn convert', 1)[1].split('pub fn read_channel', 1)[0]
        harness = r'''
#[path="ACCEPTED"] mod accepted_timing;
#[path="GUARD"] mod powered_guard;
use powered_guard::{Feedback,Fault,RunGuard};
thread_local! {
    static STATE: std::cell::RefCell<(bool,u32,Option<RunGuard<2223,100,true>>)> = std::cell::RefCell::new((false,0,None));
}
fn owns()->bool {STATE.with(|s|s.borrow().0)}
fn trip(_:Fault) {STATE.with(|s|s.borrow_mut().0=false);}
fn stream_current(phase:[u16;3])->bool {
    if !powered_guard::phase_valid(phase) {trip(Fault::Current);false}else{true}
}
fn feedback_inner(f:Feedback,at:Option<u32>)->bool {
    STATE.with(|s| {let mut s=s.borrow_mut();let now=s.1;
        if let Some(g)=s.2.as_mut() {
            if g.feedback_aged(now,now.wrapping_sub(at.unwrap()),f).is_some() {s.0=false;}
        }
        s.0
    })
}
fn setup(origin:u32) {STATE.with(|s|*s.borrow_mut()=(true,origin,Some(
    RunGuard::with_limits(origin,0,1,Feedback{phase:[2048;3],bus_mv:11700,vref:1500},100000,50000).unwrap())));}
fn now(n:u32) {STATE.with(|s|s.borrow_mut().1=n);}
fn stamp()->u32 {STATE.with(|s|s.borrow().2.as_ref().unwrap().feedback_timestamp())}
#[test] fn publication_uses_acquisition_not_delivery() {
    for origin in [0,u32::MAX-1000] {
        setup(origin);
        for n in 1..=8 {now(origin.wrapping_add(n*201+50));
            assert!(stream_feedback([2048,2048,2048,1400,1500],origin.wrapping_add(n*201)));
            assert_eq!(stamp(),origin.wrapping_add(n*201));
        }
    }
}
#[test] fn stale_age_and_lapsed_previous_deadline_still_refuse() {
    for (time,acquired) in [(1001,1001),(1001,0)] {
        setup(0);now(time);assert!(!stream_feedback([2048,2048,2048,1400,1500],acquired));
        assert_eq!(stamp(),0);
    }
}
#[test] fn electrical_and_revoked_owner_refuse() {
    for raw in [[847,2048,2048,1400,1500],[2048,3249,2048,1400,1500],
        [2048,2048,847,1400,1500],[2048,2048,2048,500,1500],
        [2048,2048,2048,1400,0],[2048,2048,2048,1400,4095]] {
        setup(0);now(201);assert!(!stream_feedback(raw,201));assert_eq!(stamp(),0);
    }
    setup(0);trip(Fault::HostAbort);assert!(!stream_feedback([2048;5],0));
}
'''.replace('ACCEPTED', (ROOT / 'examples/support/accepted_timing.rs').as_posix()).replace('GUARD', (ROOT / 'examples/support/powered_guard.rs').as_posix())
        with tempfile.TemporaryDirectory() as tmp:
            src = Path(tmp) / 'delivery.rs'
            src.write_text(harness + publisher + convert)
            exe = Path(tmp) / 'delivery.exe'
            subprocess.run(['rustc', '--edition=2021', '--test', str(src), '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)


if __name__ == '__main__':
    unittest.main()
