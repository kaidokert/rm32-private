import unittest
from pathlib import Path
from drv_seedmask_check import verify,verify_mode,MARKER

class SeedMask(unittest.TestCase):
    def good(self):
        final=(Path(__file__).resolve().parents[1]/'captures/carrier20_484_guard01.txt').read_text().split('FINALOFF\n')[-1]
        row='SEEDMASKCHECK passed=6 total=6 pending_request_injected=1 enable_primitive_only=1 disabled=1 gate_authority=0\n'
        return row*3+'FINALOFF\n'+final
    def test_strict_disabled_contract(self):
        self.assertFalse(verify(self.good())['bootstrap_isr_tested'])
        for bad in [self.good().replace('passed=6','passed=5',1),self.good().replace('disabled=1','disabled=0',1),
                    self.good()+'FINALOFF\n',self.good().replace('pending_request_injected=1','pending_request_injected=0')]:
            with self.assertRaises((ValueError,RuntimeError)):verify(bad)
    def test_marker(self):
        verify_mode(MARKER+'\n')
        for bad in ['',MARKER+'\n'+MARKER,MARKER.replace('software_latch=1','software_latch=0')]:
            with self.assertRaises(ValueError):verify_mode(bad)

if __name__=='__main__':unittest.main()
