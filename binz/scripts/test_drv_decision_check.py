import unittest
from unittest.mock import patch
from drv_decision_check import verify


class CheckTests(unittest.TestCase):
    def test_retained_actual_scope_check(self):
        from pathlib import Path
        text=(Path(__file__).resolve().parents[1]/'captures/decision_scope01.txt').read_text()
        result=verify(text)
        self.assertEqual(result['maxima_us'],[9,9,9,8,8])
        self.assertFalse(result['timing_qualified'])

    @patch('drv_decision_check.verify_off')
    def test_lifecycle_is_not_timing_qualification(self,_):
        body='\n'.join(f'DECISIONCHECK mode={i} passed=16 total=16 max_us=12 scope_only=1 gate_authority=0' for i in range(5))
        body+='\nDECISIONCHECK END disabled=1\nFINALOFF\n'
        self.assertTrue(verify(body)['lifecycle_passed'])
        self.assertFalse(verify(body)['timing_qualified'])
        for bad in [body.replace('passed=16','passed=15'),body.replace('disabled=1','disabled=0'),
                    body.replace('mode=3','mode=2'),body+body]:
            with self.assertRaises(ValueError):verify(bad)
