import unittest
from pathlib import Path
from drv_local_cycles import window_cycles, report


class LocalCycleTests(unittest.TestCase):
    def test_uniform_overlap_and_short_window(self):
        rows=[dict(us=100+i*500,step=i%6+1) for i in range(13)]
        self.assertIsNone(window_cycles(rows[:6]))
        r=window_cycles(rows)
        self.assertEqual(r['overlapping_cycles'],7)
        self.assertEqual((r['min_us'],r['median_us'],r['max_us']),(3000,3000,3000))
        self.assertFalse(r['independent_samples'])
        rows[4]['step']=1
        with self.assertRaises(ValueError):window_cycles(rows)

    def test_failed_acceleration_is_not_aggregate_steady_speed(self):
        text=(Path(__file__).resolve().parents[1]/'captures/range340_start61_hold71_10s_01.txt').read_text()
        r=report(text)
        self.assertEqual(r['outcome'],'powered_stopped')
        self.assertLess(r['aggregate_ehz'],320)
        self.assertGreater(r['tail']['reciprocal_median_ehz'],330)
        self.assertFalse(r['steady_state_proven'])
        self.assertFalse(r['gap_interpolated'])
        self.assertFalse(r['rejected_event_included'])
        self.assertEqual(r['outputs_off_verified'],1)
