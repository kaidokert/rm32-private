import unittest
from drv_recovery_budget import budget

class RecoveryBudget(unittest.TestCase):
    def test_reference_integer_rounding(self):
        for hz in range(1,2001):
            r=budget(hz);ci=r['synthetic_interval_ticks']
            self.assertGreaterEqual(ci*6*hz,2_000_000)
            self.assertLess((ci-1)*6*hz,2_000_000)
            self.assertEqual(r['wait_ticks'],ci//2-(ci*16)//64)
            self.assertEqual(r['age_limit_ticks']+64,r['wait_ticks'])
            self.assertFalse(r['hardware_qualified'])
    def test_qualification_and_age_are_independent(self):
        self.assertTrue(budget(400)['current_seed_profile_admits_constant_train'])
        self.assertGreater(budget(400)['required_saving_ticks'],0)
        self.assertFalse(budget(410,0)['current_seed_profile_admits_constant_train'])
        self.assertEqual(budget(410,0)['required_saving_ticks'],0)
    def test_invalid(self):
        for hz in [0,-1,2001,1.5]:
            with self.assertRaises(ValueError):budget(hz)
