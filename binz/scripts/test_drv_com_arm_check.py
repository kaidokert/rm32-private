import unittest
from pathlib import Path
from drv_com_arm_check import verify


class ArmCheck(unittest.TestCase):
    def test_hardware_capture_and_refusals(self):
        text=(Path(__file__).resolve().parents[1]/'captures/comkeep_544_arm01.txt').read_text()
        self.assertEqual(verify(text)['trials'],128)
        for bad in [text.replace('failed=0','failed=1'),
                    text.replace('max_arm_us=1','max_arm_us=11'),
                    text.replace('min_slack_us=31','min_slack_us=0'),
                    text.replace('trials=128','trials=127'),
                    text.replace('FINALOFF','MISSING')]:
            with self.subTest(bad=bad),self.assertRaises(ValueError):verify(bad)
