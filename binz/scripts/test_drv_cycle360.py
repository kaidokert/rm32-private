import unittest
from pathlib import Path
from drv_driven_handoff import verify,verify_cycle360


class Cycle360(unittest.TestCase):
    def test_profile_only_synthetic_replay_and_unchanged_acquisition(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'inline_540_start61_reentry69_30s_01.txt').read_text()
        candidate=text.replace('cycle_min_us=2858 event_min_us=238',
                               'cycle_min_us=2778 event_min_us=238')
        verify_cycle360(candidate,True)
        self.assertTrue(verify(candidate,30000,dropout=True,reentry=True)['powered_recovery_verified'])
        for bad,required in [(candidate,False),(text,True),
                             (candidate.replace('event_min_us=238','event_min_us=231'),True),
                             (candidate+'\nRUNLIMIT cycle_min_us=2778\n',True)]:
            with self.subTest(required=required),self.assertRaises(ValueError):
                verify_cycle360(bad,required)
        # Synthetic fixture mutation, never a reclassification of real evidence.
        with self.assertRaises(ValueError):
            verify(candidate.replace('cycle_min_ticks=5716','cycle_min_ticks=5556'),
                   30000,dropout=True,reentry=True)
