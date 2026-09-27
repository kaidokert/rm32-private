import unittest
import tempfile
from pathlib import Path
from order_trace import delta, paired, errors


class OrderTest(unittest.TestCase):
    def test_wrap_and_identity_not_time_proximity(self):
        a = [[0xffffffff, 65530, 60, 60, 15, 6], [0, 54, 60, 60, 15, 1]]
        b = [[0xffffffff, 12, 9, 14, 0xfffffff8, 0, 1], [0, 72, 69, 74, 472, 480, 2]]
        p = paired(a, b)
        self.assertEqual([r["entry_to_bridge_us"] for r in p], [20, 20])
        self.assertEqual([r["deadline_error_us"] for r in p], [5, 5])
        self.assertEqual(p[0]["bracket_ticks"], 8)
        self.assertEqual(delta(0xffffffff, 0, 32), 1)

    def test_mismatched_sector_is_not_repaired(self):
        with self.assertRaisesRegex(ValueError, "sector"):
            paired([[7, 100, 60, 60, 15, 3]], [[7, 116, 115, 118, 0, 8, 3]])

    def test_bracket_uncertainty_is_retained(self):
        p = paired([[7, 100, 60, 60, 15, 3]], [[7, 116, 115, 120, 0, 240, 4]])
        self.assertEqual(p[0]["bracket_ticks"], 240)

    def test_negative_service_order_is_not_a_large_positive_latency(self):
        with self.assertRaisesRegex(ValueError, "service or bridge"):
            paired([[7, 100, 60, 60, 15, 3]], [[7, 116, 117, 120, 0, 8, 4]])

    def test_incomplete_unfrozen_or_wrong_units_fail_closed(self):
        good = ("ORDERSNAP row_v=1 fine_hz=8000000 frozen=1 acc_len=1 acc_total=1 com_len=1 com_total=1\n"
                "ORDERA 7 100 60 60 15 3\nORDERB 7 116 115 120 0 8 4\nORDEREND\n")
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "order.txt"
            p.write_text(good, encoding="utf-8")
            self.assertEqual(errors(p), [])
            for bad in [good.replace("ORDEREND\n", ""), good.replace("frozen=1", "frozen=0"),
                        good.replace("8000000", "64000000"), good.replace("com_len=1", "com_len=2"),
                        good.replace("ORDERA 7 100", "ORDERA 7 -1")]:
                p.write_text(bad, encoding="utf-8")
                self.assertTrue(errors(p))


if __name__ == "__main__":
    unittest.main()
