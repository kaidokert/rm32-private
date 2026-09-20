from pathlib import Path
import unittest
from drv_direct_check import verify,cycle_report


class DirectCheckTests(unittest.TestCase):
    def test_retained_specialized_bracket_and_legacy_gate(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'direct_511_scope01.txt').read_text()
        with self.assertRaisesRegex(ValueError,'5>4'):verify(text)
        self.assertEqual(verify(text,cycle_gate=True)['cost_gate'],'raw_128_256_cycles')
        with self.assertRaisesRegex(ValueError,'raw cycle cost'):
            verify((root/'direct_510_scope01.txt').read_text(),cycle_gate=True)
        with self.assertRaises(ValueError):verify(self.text(),cycle_gate=True)
        for bad in [text.replace('max_cycles=236','max_cycles=257',1),
                    text.replace('max_cycles=88','max_cycles=129',1),
                    text.replace('passed=16','passed=15',1),text.replace('QD85 ','QD85 !',1),
                    text.replace('FINALOFF','NOFINALOFF')]:
            with self.assertRaises(ValueError):verify(bad,cycle_gate=True)
    def test_cycle_clock_bounds_and_unrounded_limits(self):
        text='\n'.join(f'DIRECTCYCLES preload={p} mode={m} max_cycles={256 if m==2 else 128} valid=1 core_hz=64000000 reload=63999 subtraction=0 gate_authority=0'
                       for p in (0,16,32) for m in range(5))
        self.assertTrue(cycle_report(text)['within_2_4_us'])
        self.assertFalse(cycle_report(text.replace('max_cycles=256','max_cycles=257',1))['within_2_4_us'])
        self.assertFalse(cycle_report(text.replace('max_cycles=128','max_cycles=129',1))['within_2_4_us'])
        for bad in [text+'\n'+text,text.replace('valid=1','valid=0',1),
                    text.replace('reload=63999','reload=64000',1),text.replace('max_cycles=128','max_cycles=64000',1),
                    text.replace('subtraction=0','subtraction=1',1)]:
            with self.assertRaises(ValueError):cycle_report(bad)
    def text(self):
        return (Path(__file__).resolve().parents[1]/'captures/direct_499_scope01.txt').read_text()
    def test_real_failure_and_synthetic_cost_boundary(self):
        text=self.text()
        with self.assertRaisesRegex(ValueError,'3>2'):verify(text)
        synthetic=text.replace('max_us=3','max_us=2').replace('max_us=7','max_us=4')
        self.assertTrue(verify(synthetic)['wire_passed'])
        for bad in [synthetic.replace('max_us=4','max_us=5',1),
                    synthetic.replace('passed=16','passed=15',1),
                    synthetic.replace('QD85 ','QD85 !',1),
                    synthetic.replace('final_accepted=40','final_accepted=41')]:
            with self.assertRaises(ValueError):verify(bad)
