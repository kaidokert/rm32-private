from pathlib import Path
import unittest
from drv_adc_phase_check import verify
class RawCoexistTests(unittest.TestCase):
    def test_retained_hardware_and_mutations(self):
        text=(Path(__file__).resolve().parents[1]/'captures/rawcoexist_01.txt').read_text()
        self.assertEqual(verify(text,True),['0']*3)
        for old,new in [('captures=32','captures=31'),('config_preserved=1','config_preserved=0'),('authority=0','authority=1'),('max_counter_us=0','max_counter_us=3')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1),True)
