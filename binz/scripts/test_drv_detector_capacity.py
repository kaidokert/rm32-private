import unittest
from drv_detector_capacity import analyze


class CapacityTests(unittest.TestCase):
    def visit(self, reasons):
        rows = [dict(step=1, sector_us=0, decision_reason=0)]
        rows += [dict(step=2, sector_us=i*100, decision_reason=r)
                 for i, r in enumerate(reasons)]
        rows += [dict(step=3, sector_us=0, decision_reason=0)]
        return analyze(rows)

    def test_three_cannot_supply_four(self):
        self.assertEqual(self.visit([2, 2, 3])['capacity_visits'], 0)

    def test_four_has_capacity_not_proof(self):
        self.assertEqual(self.visit([1, 1, 1, 1])['capacity_visits'], 1)

    def test_invalid_breaks_streak(self):
        result = self.visit([2, 2, 0, 3, 3])
        self.assertEqual(result['capacity_visits'], 0)
        self.assertEqual(result['max_valid_streak'], 2)


if __name__ == '__main__':
    unittest.main()
