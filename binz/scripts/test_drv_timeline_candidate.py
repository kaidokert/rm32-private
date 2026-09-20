"""Retained timeline candidate hardware result; no timing-gain generalization."""
from pathlib import Path
import re
import unittest
from drv_driven_handoff import verify

class TimelineCandidateTests(unittest.TestCase):
    def test_inline_fixed_cohort_outcomes(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for attempt in [1,2,3]:
            text=(root/f'timelineinline_start61_reentry72_30s_{attempt:02d}.txt').read_text()
            if attempt<3:
                self.assertTrue(verify(text,30000,dropout=True,reentry=True)['powered_recovery_verified'])
            else:
                with self.assertRaises(ValueError):verify(text,30000,dropout=True,reentry=True)
                from drv_cycle_fault import context
                fault=context(text)
                self.assertEqual(fault['guard']['delta_us'],2842)
                self.assertEqual(fault['outputs_off_verified'],1)
            age,arm=re.search(r'CORESEED assumed=0 .*?edge_age_ticks=(\d+).*?arm_us=(\d+)',text).groups()
            self.assertEqual((int(age),int(arm)),(170,7))

    def test_inline_first_recovery(self):
        text=(Path(__file__).resolve().parents[1]/'captures'/
              'timelineinline_start61_reentry72_30s_01.txt').read_text()
        result=verify(text,30000,dropout=True,reentry=True)
        self.assertTrue(result['powered_recovery_verified'])
        self.assertEqual(result['powered']['stack_untouched_bytes'],2676)
        age,arm=re.search(r'CORESEED assumed=0 .*?edge_age_ticks=(\d+).*?arm_us=(\d+)',text).groups()
        self.assertEqual((int(age),int(arm)),(170,7))

    def test_restored_reference(self):
        text=(Path(__file__).resolve().parents[1]/'captures'/
              'timelineref_start61_reentry72_30s_01.txt').read_text()
        self.assertTrue(verify(text,30000,dropout=True,reentry=True)['powered_recovery_verified'])
        age,arm=re.search(r'CORESEED assumed=0 .*?edge_age_ticks=(\d+).*?arm_us=(\d+)',text).groups()
        self.assertEqual((int(age),int(arm)),(170,13))

    def test_matched_recovery(self):
        text=(Path(__file__).resolve().parents[1]/'captures'/
              'timelinediv_start61_reentry72_30s_01.txt').read_text()
        result=verify(text,30000,dropout=True,reentry=True)
        self.assertTrue(result['powered_recovery_verified'])
        self.assertEqual(result['powered']['outputs_off_verified'],1)
        self.assertEqual(result['powered']['run_cycle_min_us'],'2858')
        age,arm=re.search(r'CORESEED assumed=0 .*?edge_age_ticks=(\d+).*?arm_us=(\d+)',text).groups()
        self.assertEqual((int(age),int(arm)),(172,8))

    def test_failed_second_attempt_is_not_qualification(self):
        text=(Path(__file__).resolve().parents[1]/'captures'/
              'timelinediv_start61_reentry72_30s_02.txt').read_text()
        with self.assertRaises(ValueError):verify(text,30000,dropout=True,reentry=True)
        from drv_cycle_fault import context
        fault=context(text)
        self.assertEqual(fault['guard']['delta_us'],2854)
        self.assertEqual(fault['outputs_off_verified'],1)
        self.assertFalse(fault['cause_identified'])
        age,arm=re.search(r'CORESEED assumed=0 .*?edge_age_ticks=(\d+).*?arm_us=(\d+)',text).groups()
        self.assertEqual((int(age),int(arm)),(172,8))
