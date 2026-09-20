"""Retain successful holds separately from failed recovery campaigns."""
from pathlib import Path
import unittest
from drv_driven_handoff import verify
from drv_cycle_fault import context

ROOT=Path(__file__).resolve().parents[1]/'captures'


class Range345EvidenceTests(unittest.TestCase):
    def test_passes_and_recovery_failure_are_not_conflated(self):
        for stem,ms,recovery in [('reentry70_30s',30000,True),('hold71_10s',10000,False)]:
            text=(ROOT/f'range345_start61_{stem}_01.txt').read_text()
            result=verify(text,ms,dropout=recovery,reentry=recovery)
            self.assertTrue(result['powered_recovery_verified' if recovery else 'powered_handoff_window_verified'])
        text=(ROOT/'range345_start61_reentry71_30s_01.txt').read_text()
        with self.assertRaises(ValueError):verify(text,30000,dropout=True,reentry=True)
        fault=context(text)
        self.assertEqual(fault['guard']['delta_us'],2881)
        self.assertEqual(fault['outputs_off_verified'],1)
        self.assertFalse(fault['cause_identified'])
