import unittest
from pathlib import Path
from drv_driven_handoff import verify_cycle450,verify_cycle400
from drv_seed_profile import verify as verify_seed,EXPECTED

class Cycle450(unittest.TestCase):
    def test_actual_capture_full_verifier(self):
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/cycle450_601_hold75_10s.txt').read_text()
        self.assertTrue(verify(text,10000)['powered_handoff_window_verified'])
        with self.assertRaises(ValueError):verify(text.replace('cycle_min_us=2223','cycle_min_us=2222'),10000)

    def test_exact_opt_in_and_separate_seed(self):
        text='RUNLIMIT cycle_min_us=2223 event_min_us=238 cycle_max_us=6000 event_max_us=1000 experimental=1\n'
        verify_cycle450(text,True);verify_cycle400(text,False)
        verify_seed(text,False)
        verify_seed(text+'SEEDPROFILE '+EXPECTED+'\n',True)
        for candidate,required in [(text,False),(text.replace('2223','2500'),True),
                (text.replace('238','237'),True),(text+text,True),('',True)]:
            with self.assertRaises(ValueError):verify_cycle450(candidate,required)
