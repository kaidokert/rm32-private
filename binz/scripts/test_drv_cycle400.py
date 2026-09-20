import unittest
from pathlib import Path
from drv_driven_handoff import verify_cycle400,verify_cycle360,verify

class Cycle400(unittest.TestCase):
    def test_strict_profile_and_old_seed_contract(self):
        old=(Path(__file__).resolve().parents[1]/'captures/live_570_armed70_baseline.txt').read_text()
        # Synthetic protocol mutation only; NEVER reclassify the real capture.
        candidate=old.replace('cycle_min_us=2778','cycle_min_us=2500')
        verify_cycle400(candidate,True)
        verify_cycle360(candidate,False)
        self.assertTrue(verify(candidate,10000)['powered_handoff_window_verified'])
        for text,required in [(candidate,False),(old,True),
                (candidate.replace('event_min_us=238','event_min_us=237'),True),
                (candidate+'\nRUNLIMIT invalid\n',True)]:
            with self.assertRaises(ValueError):verify_cycle400(text,required)
