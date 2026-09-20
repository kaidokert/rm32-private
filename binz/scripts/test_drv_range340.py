"""Retained340-profile qualification, not a claim of repeatability."""
from pathlib import Path
import unittest
from drv_driven_handoff import verify

ROOT=Path(__file__).resolve().parents[1]/'captures'


class Range340EvidenceTests(unittest.TestCase):
    def test_complete70_cohort_and_failed71_step(self):
        for suffix in ['01','02','03']:
            text=(ROOT/f'range340_start61_reentry70_30s_{suffix}.txt').read_text()
            self.assertTrue(verify(text,30000,dropout=True,reentry=True)['powered_recovery_verified'])
        text=(ROOT/'range340_start61_hold71_10s_01.txt').read_text()
        with self.assertRaises(ValueError):verify(text,10000)
        from drv_cycle_fault import context
        fault=context(text)
        self.assertEqual(fault['guard']['delta_us'],2930)
        self.assertEqual(fault['outputs_off_verified'],1)
        self.assertFalse(fault['physical_overspeed_proven'])

    def test_preceding_recovery_hold_and_first_new_recovery(self):
        for stem,ms,reentry in [('reentry69_30s',30000,True),
                                ('hold70_10s',10000,False),
                                ('reentry70_30s',30000,True)]:
            with self.subTest(stem=stem):
                text=(ROOT/f'range340_start61_{stem}_01.txt').read_text()
                result=verify(text,ms,dropout=reentry,reentry=reentry)
                key='powered_recovery_verified' if reentry else 'powered_handoff_window_verified'
                self.assertTrue(result[key])
                self.assertEqual(result['powered']['run_cycle_min_us'],'2942')
                self.assertEqual(result['powered']['run_event_min_us'],'245')
                self.assertEqual(result['powered']['outputs_off_verified'],1)
