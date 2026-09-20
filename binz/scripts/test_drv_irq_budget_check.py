import unittest
from pathlib import Path
from drv_seed_timing_check import verify

class BudgetCheck(unittest.TestCase):
    def test_actual_disabled_boundary_and_malformed_refusal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        text=(root/'seedtiming_561_budget.txt').read_text()
        self.assertEqual(verify(text,True)['cases'],5)
        for old,new in [('failed=0','failed=1'),('boundary_us=50','boundary_us=51'),
                        ('synthetic_elapsed=1','synthetic_elapsed=0'),
                        ('IRQBUDGETCHECK','MISSINGCHECK')]:
            with self.assertRaises(ValueError): verify(text.replace(old,new),True)
        # A new test mode never turns the previous isolated timing failure green.
        old=(root/'seedtiming_559b_disabled.txt').read_text()
        with self.assertRaises(ValueError): verify(old)
        with self.assertRaises(ValueError): verify(old,True)
