"""Offline candidate selection is not a hardware handoff qualification."""
import unittest
from pathlib import Path
from drv_driven_run import records, seed_window
from drv_driven_handoff import verify


class SeedReanchorTests(unittest.TestCase):
    def test_low_hysteresis_refusal_is_not_a_sustained_run(self):
        from drv_driven_run import verify as acquisition_verify
        from drv_capture import verify_off
        text = (Path(__file__).resolve().parents[1] / 'captures' /
                'hystlow_hold64_01.txt').read_text()
        result = acquisition_verify(text, transfer=False)
        self.assertFalse(result['driven_seed_ready'])
        self.assertEqual(result['driven_seed_fault'], 3)
        self.assertEqual(result['irq_accepts'], 22)
        rows = seed_window(text, records(text, 'DI85', 7), True)
        self.assertEqual([r[0] for r in rows[:2]], [4, 5])
        self.assertEqual(2*(rows[1][2]-rows[0][2]), 2238)
        self.assertIn('COMPHYST code=1 startup_and_bemf=1 raw_reads_unchanged=1', text)
        verify_off(text[text.rfind('COAST END'):].encode())
        with self.assertRaises(ValueError):
            verify(text, 10000)

    def test_actual_recovery_and_new_build_late_arm_are_not_conflated(self):
        from drv_cpu_meter import decode
        from drv_driven_handoff import first_arm
        from drv_driven_run import verify as acquisition_verify
        from drv_capture import verify_off
        root = Path(__file__).resolve().parents[1] / 'captures'
        text = (root / 'cpu_union_range_reentry46_01.txt').read_text()
        self.assertTrue(verify(text, 10000, dropout=True, reentry=True)['powered_recovery_verified'])
        self.assertTrue(decode(text)['valid'])
        refused = (root / 'reanchor_reentry48_01.txt').read_text()
        self.assertTrue(acquisition_verify(refused, transfer=True)['driven_seed_ready'])
        self.assertEqual(first_arm(refused)[4], 1)
        self.assertIn('DRIVENSEEDRESTART count=0 anchor_epoch=0', refused)
        self.assertIn('CORESEED armed=0', refused)
        verify_off(refused[refused.rfind('COAST END'):].encode())
        with self.assertRaises(ValueError):
            verify(refused, 10000, dropout=True, reentry=True)

    def setUp(self):
        self.text = (Path(__file__).resolve().parents[1] / 'captures' /
                     'cpu_union_range_reentry48_01.txt').read_text()
        self.rows = records(self.text, 'DI85', 7)
        self.marker = '\nDRIVENSEEDRESTART count=1 anchor_epoch=3 max_restarts=1 fresh_intervals=12 original_deadline=1\n'

    def test_new_candidate_excludes_the_missing_interval(self):
        window = seed_window(self.text + self.marker, self.rows, True)
        self.assertEqual([r[0] for r in window], list(range(3, 16)))
        self.assertEqual(2*(window[-1][2]-window[0][2])//12, 1566)
        self.assertEqual(window[-1][2]*2, 24930)
        # Adding policy text cannot relabel this old failure as a motor pass.
        with self.assertRaises(ValueError):
            verify(self.text + self.marker, 10000, dropout=True, reentry=True)

    def test_no_marker_preserves_strict_legacy_window(self):
        self.assertEqual(seed_window(self.text, self.rows, True)[0][0], 1)

    def test_wrong_duplicate_or_unsupported_policy_refuses(self):
        for marker in [self.marker*2, self.marker.replace('anchor_epoch=3', 'anchor_epoch=4'),
                       self.marker.replace('count=1', 'count=0'),
                       self.marker.replace('max_restarts=1', 'max_restarts=2')]:
            with self.assertRaises(ValueError):
                seed_window(self.text + marker, self.rows, True)
        bad = list(self.rows); first = list(bad[0]); first[3] = first[2] + 21; bad[0] = first
        with self.assertRaises(ValueError):
            seed_window(self.text + self.marker, bad, True)


if __name__ == '__main__':
    unittest.main()
