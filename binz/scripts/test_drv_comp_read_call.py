import unittest
from drv_driven_handoff import verify_read_call


class ReadCall(unittest.TestCase):
    marker='COMPREADCALL noninline=1 live_read=1 read_count_unchanged=1 timing_equivalence_proven=0\n'

    def test_baseline_and_explicit_candidate(self):
        verify_read_call('')
        verify_read_call(self.marker,True)

    def test_fail_closed(self):
        for text,required in [(self.marker,False),('',True),
                              (self.marker*2,True),
                              (self.marker.replace('live_read=1','live_read=0'),True)]:
            with self.subTest(text=text,required=required),self.assertRaises(ValueError):
                verify_read_call(text,required)
