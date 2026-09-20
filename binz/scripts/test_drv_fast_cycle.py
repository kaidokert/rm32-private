import unittest
from drv_fast_cycle import decode,verify
class FastCycleTests(unittest.TestCase):
    def test_event100_requires_exact_profile_selection(self):
        from drv_driven_handoff import verify_cycle450
        text='RUNLIMIT cycle_min_us=2223 event_min_us=100 cycle_max_us=6000 event_max_us=1000 experimental=1\n'
        verify_cycle450(text,True,True)
        for required,event100 in [(False,False),(True,False),(False,True)]:
            with self.assertRaises(ValueError):verify_cycle450(text,required,event100)
        with self.assertRaises(ValueError):verify_cycle450(text.replace('event_min_us=100','event_min_us=99'),True,True)
    def test_explicit_selection_and_bounds(self):
        for count,minimum in [(0,0),(1,2222),(100,1800)]:
            text=f'FASTCYCLE r1 {count} {minimum}\n'
            self.assertEqual(verify(text,True)['count'],count)
            with self.assertRaises(ValueError):verify(text)
            with self.assertRaises(ValueError):decode(text+text)
        self.assertIsNone(verify(''))
        with self.assertRaises(ValueError):verify('',True)
    def test_invalid_stats_refused(self):
        for count,minimum in [(0,1),(1,0),(1,2223),(4294967296,1800)]:
            with self.assertRaises(ValueError):decode(f'FASTCYCLE r1 {count} {minimum}\n')
