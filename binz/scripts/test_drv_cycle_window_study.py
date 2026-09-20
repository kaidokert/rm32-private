import unittest
from pathlib import Path
from drv_cycle_window_study import first_refusal,fault_assessment


def events(gaps):
    out=[(0,1)];at=0
    for i,gap in enumerate(gaps):
        at+=gap;out.append((at,(i+1)%6+1))
    return out


class CycleWindowStudy(unittest.TestCase):
    def test_late_boundary_not_universally_cancelled(self):
        for delay,expected in [(100,None),(150,'cycle_floor')]:
            e=events([475]*70)
            at,step=e[25];e[25]=(at+delay,step)
            self.assertEqual(first_refusal(e,cycles=1)['reason'],'cycle_floor')
            result=first_refusal(e,cycles=2)
            self.assertEqual(result['reason'] if result else None,expected)

    def test_sustained_acceleration_has_extra_detection_delay(self):
        e=events([475]*30+[400]*30)
        old=first_refusal(e,cycles=1);new=first_refusal(e,cycles=2)
        self.assertEqual(old['reason'],'cycle_floor')
        self.assertEqual(new['reason'],'cycle_floor')
        self.assertGreater(new['index'],old['index'])
        self.assertLessEqual(new['index']-old['index'],6)

    def test_harmonic_fast_gap_missing_and_order_fail(self):
        self.assertEqual(first_refusal(events([250]*30),cycles=2)['reason'],'cycle_floor')
        self.assertEqual(first_refusal(events([475]*20+[237]),cycles=2)['reason'],'fast_event')
        self.assertEqual(first_refusal(events([475]*20+[1001]),cycles=2)['reason'],'stale')
        e=events([475]*30);at,_=e[20];e[20]=(at,1)
        self.assertEqual(first_refusal(e,cycles=2)['reason'],'order')

    def test_actual_failure_stays_a_failure(self):
        p=Path(__file__).resolve().parents[1]/'captures/cycle360_550_start61_hold72_10s_01.txt'
        r=fault_assessment(p.read_text())
        self.assertEqual(r['original_outcome'],'powered_stopped')
        self.assertTrue(r['single_below_floor'])
        self.assertFalse(r['two_cycle_below_floor'])
        self.assertFalse(r['safe_to_deploy'])

    def test_current_8_percent_refusal_is_not_reclassified(self):
        p=Path(__file__).resolve().parents[1]/'captures/envelope_598_hold80_60s.txt'
        r=fault_assessment(p.read_text())
        self.assertEqual(r['cycle_floor_us'],2500)
        self.assertEqual(r['reference_one_cycle_us'],2489.5)
        self.assertEqual(r['reference_two_cycle_mean_us'],2543.25)
        self.assertEqual(r['original_outcome'],'powered_stopped')
        self.assertTrue(r['single_below_floor'])
        self.assertFalse(r['two_cycle_below_floor'])
        self.assertFalse(r['whole_run_replayed'])
        self.assertFalse(r['physical_lock_proven'])
        self.assertFalse(r['safe_to_deploy'])
