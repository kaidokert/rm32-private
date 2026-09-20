import unittest
from pathlib import Path
from drv_sparse_fault import report


class SparseFault(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text=(Path(__file__).resolve().parents[1]/'captures'/
                  'qualsparse_epoch476_start61_reentry72_30s_01.txt').read_text()

    def test_real_guard_veto_is_not_persistence_failure(self):
        r=report(self.text)
        self.assertTrue(r['reference_persistence_passed'])
        self.assertFalse(r['final_call']['accepted'])
        self.assertEqual(r['selected_successful_overlaps'],2)
        self.assertIsNone(r['persistence_duration_us'])
        self.assertFalse(r['preemption_cause_proven'])
        self.assertTrue(r['outputs_off_verified'])
        boundary=r['preceding_boundary']
        self.assertEqual(boundary['accepted_before'],1909)
        self.assertEqual(boundary['step'],2)
        self.assertEqual(boundary['interval_us'],589)
        self.assertEqual(boundary['successor_interval_us'],417.5)
        self.assertTrue(boundary['no_selected_call_proven'])
        self.assertFalse(boundary['physical_edge_delay_measured'])

    def test_missing_callback_proof_rejected(self):
        text='\n'.join(l for l in self.text.splitlines() if not l.startswith('CYCLECORE'))
        with self.assertRaises(ValueError):report(text)

    def test_final_count_must_match_not_only_bound(self):
        with self.assertRaises(ValueError):
            report(self.text.replace('SPARSEBIND epoch=134 accepted=1915',
                                     'SPARSEBIND epoch=134 accepted=1916'))

    def test_retained_suffix_covers_boundary_despite_older_omissions(self):
        text=(Path(__file__).resolve().parents[1]/'captures'/
              'qualsparse_critical478_start61_reentry72_30s_01.txt').read_text()
        r=report(text)
        self.assertEqual(r['omitted_selected'],1105)
        self.assertEqual(r['preceding_boundary']['accepted_before'],3475)
        self.assertTrue(r['preceding_boundary']['no_selected_call_proven'])


if __name__=='__main__':unittest.main()
