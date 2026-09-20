import unittest
from pathlib import Path
from drv_driven_handoff import verify

class RefusalReport(unittest.TestCase):
    def test_valid_seed_can_still_refuse_fresh_arm(self):
        root=Path(__file__).resolve().parents[1]
        for name,remaining,age in [('seed400_583_recovery80_30s.txt',51,168),
                                   ('commonage_585_recovery80_30s.txt',58,164)]:
            text=(root/'captures'/name).read_text()
            with self.assertRaisesRegex(ValueError,f'fresh-arm margin refused: remaining_ticks={remaining} <64, edge_age_ticks={age}'):
                verify(text,30000,dropout=True,reentry=True)
            with self.assertRaisesRegex(ValueError,'lacks finaloff'):
                verify(text[:text.rfind('FINALOFF\n')],30000,dropout=True,reentry=True)

    def test_retained_cycle_too_fast_is_failure_not_staging_error(self):
        text=(Path(__file__).resolve().parents[1]/'captures/cycle400_577_recovery80_30s.txt').read_text()
        with self.assertRaisesRegex(ValueError,'recovery acquisition refused: result=14, elapsed_us=3112, intervals=5'):
            verify(text,30000,dropout=True,reentry=True)
        bad=text[:text.rfind('FINALOFF\n')]
        with self.assertRaisesRegex(ValueError,'lacks finaloff'):
            verify(bad,30000,dropout=True,reentry=True)
