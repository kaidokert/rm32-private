import unittest
from pathlib import Path
from drv_average_refusal import verify

class Refusal(unittest.TestCase):
    def test_saved_hardware_capture_and_negative_cases(self):
        text=(Path(__file__).resolve().parents[1]/'captures/averageguard_703_refusal.txt').read_text()
        self.assertTrue(verify(text)['missing_threshold_refused'])
        marker='RUN refused: average current configuration missing; gates + en OFF'
        for bad in [text.replace(marker,''),text+marker,text+'RUN: align=',
                    text.replace('en=0','en=1'),text.replace('FINALOFF\n','')]:
            with self.assertRaises((ValueError,RuntimeError)):verify(bad)

if __name__=='__main__':unittest.main()
