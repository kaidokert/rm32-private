"""Retained335 hardware evidence: a declared cohort and a failed next step."""
from pathlib import Path
import unittest
from drv_driven_handoff import verify
from drv_cycle_fault import context

ROOT = Path(__file__).resolve().parents[1] / 'captures'


class Range335EvidenceTests(unittest.TestCase):
    def test_three_attempt_recovery_cohort(self):
        for suffix in ['01', '02', '03']:
            with self.subTest(attempt=suffix):
                text = (ROOT / f'range335_start61_reentry69_30s_{suffix}.txt').read_text()
                result = verify(text, 30000, dropout=True, reentry=True)
                self.assertTrue(result['powered_recovery_verified'])
                self.assertEqual(result['powered']['run_cycle_min_us'], '2986')
                self.assertEqual(result['powered']['run_event_min_us'], '248')
                self.assertEqual(result['powered']['outputs_off_verified'], 1)

    def test_next_duty_failure_remains_a_failure(self):
        text = (ROOT / 'range335_start61_hold70_10s_01.txt').read_text()
        with self.assertRaises(ValueError):
            verify(text, 10000)
        fault = context(text)
        self.assertEqual(fault['guard']['delta_us'], 2964)
        self.assertEqual(fault['outputs_off_verified'], 1)
        self.assertFalse(fault['cause_identified'])
