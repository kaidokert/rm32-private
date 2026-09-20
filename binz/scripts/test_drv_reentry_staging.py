"""Real staged-recovery evidence and strict opt-in report provenance."""
import unittest
from pathlib import Path
from drv_driven_handoff import verify
from drv_cpu_meter import decode


class ReentryStagingTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[1] / 'captures'
    marker = 'REENTRYSTATS used=1 preparation_before_acquisition=1 gate_authority=0'
    def test_higher_hold_and_supply_anchor_attempt_do_not_prove_recovery(self):
        from drv_driven_handoff import first_arm
        from drv_capture import verify_off
        for name, ms in [('staged_range52_01', 10000), ('current_anchor50_01', 30000)]:
            text = (self.root / (name + '.txt')).read_text()
            self.assertTrue(verify(text, ms)['powered_handoff_window_verified'])
            self.assertTrue(decode(text)['valid'])
        failed = (self.root / 'staged_reentry52_01.txt').read_text()
        self.assertEqual(first_arm(failed)[4], 1)
        self.assertIn('interval_ticks=1212 edge_age_ticks=246', failed)
        self.assertEqual(1212//2-((1212*16)>>6)-246, 57)  # 28.5us, below32us
        self.assertIn('CORESEED armed=0', failed)
        verify_off(failed[failed.rfind('COAST END'):].encode())
        with self.assertRaises(ValueError):
            verify(failed, 10000, dropout=True, reentry=True)

    def test_real_recoveries_keep_original_deadline(self):
        for name, margin in [('reentry_stage48_01', 118), ('reentry_stage50_01', 159),
                             ('reentry_stage50_02', 221), ('reentry_stage50_03', 97)]:
            text = (self.root / (name + '.txt')).read_text()
            result = verify(text, 10000, dropout=True, reentry=True)
            self.assertTrue(result['powered_recovery_verified'])
            p = result['powered']
            self.assertEqual(int(p['original_end_elapsed_us']) - int(p['final_elapsed_us']), margin)
            self.assertTrue(decode(text)['valid'])
            self.assertIn(self.marker, text)
        exercised = (self.root / 'reentry_stage50_01.txt').read_text()
        self.assertIn('DRIVENSEEDRESTART count=1 anchor_epoch=3 ', exercised)
        # Losing the restart provenance cannot turn a gap into twelve intervals.
        bad = '\n'.join(line for line in exercised.splitlines() if not line.startswith('DRIVENSEEDRESTART '))
        with self.assertRaises(ValueError):
            verify(bad, 10000, dropout=True, reentry=True)

    def test_conflicting_or_duplicate_staging_provenance_refuses(self):
        text = (self.root / 'reentry_stage50_01.txt').read_text()
        for bad in [text + '\n' + self.marker, text.replace(self.marker, self.marker.replace('used=1', 'used=0')),
                    text.replace(self.marker, self.marker.replace('gate_authority=0', 'gate_authority=1'))]:
            with self.assertRaises(ValueError):
                verify(bad, 10000, dropout=True, reentry=True)


if __name__ == '__main__':
    unittest.main()
