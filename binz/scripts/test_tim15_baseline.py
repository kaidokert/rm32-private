import unittest
from pathlib import Path
from drv_baseline_check import verify_cases

class Backend(unittest.TestCase):
    def test_real_route_capture_and_backend_mismatch(self):
        text=(Path(__file__).resolve().parents[1]/'captures/tim15_717_baseline.txt').read_text().replace('\r','')
        result=verify_cases(text,tim15=True)
        self.assertEqual(result[3]['counts'],[128]*5)
        self.assertEqual(result[3]['elapsed_us'],25633)
        for bad in [text.replace('timer=15','timer=3'),text.replace('extsel=4','extsel=3'),
                    text.replace('backend_only=1','backend_only=0')]:
            with self.assertRaises(ValueError):verify_cases(bad,tim15=True)
        with self.assertRaises(ValueError):verify_cases(text,tim15=False)

if __name__=='__main__':unittest.main()
