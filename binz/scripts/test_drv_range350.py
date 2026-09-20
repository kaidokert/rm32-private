"""Retained350-profile recoveries and the fixed three-attempt7.1 cohort."""
from pathlib import Path
import unittest
from drv_driven_handoff import verify

ROOT=Path(__file__).resolve().parents[1]/'captures'


class Range350EvidenceTests(unittest.TestCase):
    def test_first_recoveries(self):
        for duty, attempt in [(70,1),(71,1),(71,2),(71,3),(72,1),(72,2),(72,3)]:
            text=(ROOT/f'range350_start61_reentry{duty}_30s_{attempt:02d}.txt').read_text()
            result=verify(text,30000,dropout=True,reentry=True)
            self.assertTrue(result['powered_recovery_verified'])
            self.assertEqual(result['powered']['run_cycle_min_us'],'2858')
            self.assertEqual(result['powered']['run_event_min_us'],'238')
            self.assertEqual(result['powered']['outputs_off_verified'],1)

    def test_72_initial_hold(self):
        text=(ROOT/'range350_start61_hold72_10s_01.txt').read_text()
        result=verify(text,10000,dropout=False,reentry=False)
        self.assertTrue(result['powered_handoff_window_verified'])
        self.assertEqual(result['powered']['outputs_off_verified'],1)

    def test_73_refusal_retained(self):
        text=(ROOT/'range350_start61_hold73_10s_01.txt').read_text()
        with self.assertRaises(ValueError):verify(text,10000)
        from drv_cycle_fault import context
        fault=context(text)
        self.assertEqual(fault['guard']['delta_us'],2844)
        self.assertEqual(fault['outputs_off_verified'],1)
        self.assertFalse(fault['cause_identified'])
        from drv_local_cycles import report
        local=report(text)
        self.assertEqual(local['tail']['median_us'],2917)
        self.assertFalse(local['steady_state_proven'])
