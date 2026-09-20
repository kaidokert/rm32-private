from pathlib import Path
import unittest
from drv_fault_pair import report

class FaultPairTests(unittest.TestCase):
    def test_retained_failures_and_conservation(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,delta in [('range350_start61_hold73_10s_01.txt',2844),
                           ('timelinediv_start61_reentry72_30s_02.txt',2854),
                           ('timelineinline_start61_reentry72_30s_03.txt',2842)]:
            r=report((root/name).read_text())
            self.assertEqual(r['guard_cycle_us'],delta)
            self.assertEqual(sum(r['paired_sector_changes_us']),
                             r['reference_current_us']-r['reference_previous_us'])
            self.assertEqual(r['outputs_off_verified'],1)
            self.assertFalse(r['preemption_proven'])
            self.assertFalse(r['independent_rotor'])

    def test_pass_is_not_a_fault_sample(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        with self.assertRaises(ValueError):
            report((root/'timelineinline_start61_reentry72_30s_01.txt').read_text())
