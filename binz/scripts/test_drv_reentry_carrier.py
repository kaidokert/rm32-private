import unittest
from drv_driven_handoff import verify_reentry_carrier


class ReentryCarrierTests(unittest.TestCase):
    def test_provenance_is_exact_and_unique(self):
        line = ('REENTRYCARRIER prepared_before_edge=1 live_period_check=1 '
                'output_authority_retained=0 failure_reset_unchanged=1')
        verify_reentry_carrier(line)
        for bad in ['', line+'\n'+line,
                    line.replace('live_period_check=1', 'live_period_check=0'),
                    line.replace('output_authority_retained=0', 'output_authority_retained=1'),
                    line.replace('failure_reset_unchanged=1', 'failure_reset_unchanged=0')]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                verify_reentry_carrier(bad)

    def test_old_capture_does_not_claim_new_preparation(self):
        from pathlib import Path
        text = (Path(__file__).resolve().parents[1] /
                'captures/quietstamp_start61_reentry68_30s_01.txt').read_text()
        with self.assertRaises(ValueError):
            verify_reentry_carrier(text)
