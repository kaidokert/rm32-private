from pathlib import Path
import unittest
from drv_adc_trigger_check import validate


class TriggerTests(unittest.TestCase):
    def test_real_probe_and_failed_or_missing_results(self):
        text=(Path(__file__).resolve().parents[1]/'captures/adc_trigger_01.txt').read_text()
        self.assertEqual([r[1] for r in validate(text)],[3247,3246,3246])
        for bad in [text.replace('result=0','result=5',1),
                    text.replace('elapsed_us=3247','elapsed_us=10000'),
                    text.replace('ADCTRIGGER result=0','MISSING result=0',1),
                    text[:text.rfind('off')]]:
            with self.assertRaises((ValueError,RuntimeError)): validate(bad)
