import unittest
from drv_seed_profile import verify,EXPECTED
class SeedProfile(unittest.TestCase):
    def test_explicit_exact_separate_profile(self):
        old='RUNLIMIT cycle_min_us=2500 event_min_us=238 cycle_max_us=6000 event_max_us=1000 experimental=1\n'
        new=old+'SEEDPROFILE '+EXPECTED+'\n'
        verify(old,False);verify(new,True)
        for text,flag in [(old,True),(new,False),(new+new,True),
                          (new.replace('834','833'),True),(new.replace('2500','2778'),True)]:
            with self.assertRaises(ValueError):verify(text,flag)
