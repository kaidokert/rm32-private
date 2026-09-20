from pathlib import Path
import unittest
from drv_qualification_check import verify

class QualificationCheckTests(unittest.TestCase):
    def test_retained_hardware_overhead_refusal(self):
        text=(Path(__file__).resolve().parents[1]/'captures/qualwindow_scope01.txt').read_text()
        with self.assertRaisesRegex(ValueError,'overhead refused'):verify(text)
        from drv_capture import verify_off
        verify_off(text.split('FINALOFF\n')[1].encode())
    def test_limits_and_semantics_both_required(self):
        text=(Path(__file__).resolve().parents[1]/'captures/qualwindow_scope01.txt').read_text()
        synthetic=text.replace('max_us=6','max_us=4').replace('max_us=5','max_us=2')
        self.assertTrue(verify(synthetic)['outputs_off'])
        with self.assertRaises(ValueError):verify(synthetic.replace('passed=16','passed=15',1))
