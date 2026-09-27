"""Narrow source-order pin; register model and hardware pad test are separate."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class LatchPolicy(unittest.TestCase):
    def test_target_order_matches_model(self):
        source = (ROOT / "src/hw/pwm/latch.rs").read_text(encoding="utf-8")
        steps = ["w.bits(cr1 | 2)", "w.bits((cr2 | 1) & !4)",
                 "plan.ccmr1 | 0x0808", "plan.ccmr2 | 0x08", "t.ccr1().write",
                 "t.ccr2().write", "t.ccr3().write", "w.bits(plan.ccer)",
                 "w.comg().set_bit()", "w.bits(cr2)", "w.bits(cr1)"]
        positions = [source.index(s) for s in steps]
        self.assertEqual(positions, sorted(positions))
        for forbidden in (".ug()", ".cnt()", ".moe()"):
            self.assertNotIn(forbidden, source)

    def test_stop_check_is_inside_mask_before_latch(self):
        source = (ROOT / "src/roots.rs").read_text(encoding="utf-8")
        body = source.split("fn apply_com_plan<")[1].split("fn bridge_stamp")[0]
        self.assertLess(body.index("interrupt::free"), body.index("arm_allowed"))
        self.assertLess(body.index("arm_allowed"), body.index("latch::apply"))


if __name__ == "__main__":
    unittest.main()
