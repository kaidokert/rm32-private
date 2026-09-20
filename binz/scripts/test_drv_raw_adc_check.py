from pathlib import Path
import unittest
from drv_raw_adc_check import verify
class RawAdcTests(unittest.TestCase):
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/rawcoexist_01.txt').read_text().split('FINALOFF\n')[1]
        row='RAWADC scans=128 captures=128 fault=0 elapsed_us=25700 vref_min=1500 disabled=1 period_us=201 adc_channels=5 dma_polled=1 authority=0\n'
        return row*3+'FINALOFF\n'+final
    def test_good_and_bad(self):
        text=self.fixture();self.assertEqual(len(verify(text)),3)
        for old,new in [('captures=128','captures=127'),('fault=0','fault=1'),('elapsed_us=25700','elapsed_us=100'),('vref_min=1500','vref_min=0'),('period_us=201','period_us=101')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1))
