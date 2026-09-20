import unittest
from pathlib import Path
from drv_filter_latency import decode
class LatencyTests(unittest.TestCase):
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/dutysplit_atomic_02.txt').read_text().split('FINALOFF\n')[1]
        return 'FILTERLATENCY n=1 latest_capture=1 authority=0 tick_half_us=1 prefix=1\nFILTERLAT seq=1 step=2 epoch=8 valid=1 over=1 capture=65530 before=4 after=6 raw=0 pwm=100\nFINALOFF\n'+final
    def test_wrap_and_ambiguity(self):
        v=decode(self.fixture())[0];self.assertEqual(v['latest_capture_age_us'],5)
        self.assertFalse(v['original_irq_latency_proven']);self.assertEqual(v['raw_bracket_us'],1)
    def test_no_new_capture(self):self.assertIsNone(decode(self.fixture().replace('valid=1','valid=0'))[0]['latest_capture_age_us'])
    def test_refuses_invalid(self):
        for old,new in [('n=1','n=2'),('authority=0','authority=1'),('step=2','step=0'),('after=6','after=500'),('pwm=100','pwm=3000'),('FINALOFF','MISSING')]:
            with self.subTest(old=old),self.assertRaises(ValueError):decode(self.fixture().replace(old,new))
