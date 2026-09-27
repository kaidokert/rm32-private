"""Narrow source wiring checks; host behavioral tests live in bemf/timing_tests."""
import unittest
from pathlib import Path
from test_entry_arm import block_after, code

ROOT = Path(__file__).resolve().parent.parent


class WaitPolicyWiring(unittest.TestCase):
    def test_both_decision_paths_forward_type_into_exclusive_offer(self):
        roots = code((ROOT / "src/roots.rs").read_text())
        for marker in ["fn det_decide_plain<", "fn det_decide_logged<"]:
            body = block_after(roots, marker)
            self.assertIn("zc.offer_timed::<T, _, _>", body)
            self.assertIn("S.det().zc.root(at", body)

    def test_default_and_experiment_have_explicit_distinct_types(self):
        roots = code((ROOT / "src/roots.rs").read_text())
        self.assertIn("comp_root_timed::<L, C, crate::bemf::FreshEstimate>", block_after(roots, "fn comp_root<"))
        binary = code((ROOT / "bin/prior-pwm.rs").read_text())
        self.assertIn("comp_root_timed::<NoLog, NoChain, PreviousEstimate>", binary)
        self.assertIn("Production::new()", binary)


if __name__ == "__main__":
    unittest.main()
