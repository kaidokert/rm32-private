"""No-hardware source-containment/mutation checks supplement emitted-code review.

This is a narrow recognizer for these functions, not a Rust parser/proof.
"""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def code(text):
    return re.sub(r"/\*.*?\*/|//[^\n]*", "", text, flags=re.S)


def block_after(text, marker):
    start = text.index("{", text.index(marker))
    depth = 0
    for end in range(start, len(text)):
        depth += (text[end] == "{") - (text[end] == "}")
        if depth == 0:
            return text[start + 1:end]
    raise ValueError("unclosed block")


def check_crossing(text):
    body = block_after(code(text), "pub fn com_arm_crossing(")
    inside = block_after(body, "cortex_m::interrupt::free(")
    required = ["arm_allowed(", "prepare_crossing()", "hw::clock::raw()",
                "crossing_left(wait, spent)", "stop_expired_arm()",
                "sched_raw.store(", ".phase.store(1", "start_crossing("]
    at = 0
    for mark in required:
        at = inside.index(mark, at) + len(mark)
        if body.count(mark) != inside.count(mark):
            raise ValueError("arm operation outside critical section: " + mark)
    expired = block_after(code(text), "fn stop_expired_arm()")
    if "guard_trip(Reason::LateArm)" not in expired:
        raise ValueError("expired path no longer stops")


def check_acceptance_window(text):
    body = block_after(code(text), "fn det_decide_plain_window<")
    # This closure starts with a match; extract its lexical match block.
    inside = block_after(body, "W::run(")
    required = ["sector_start_raw.store", "accept_raw.store", "accept_avg.store",
                "accept_blank.store", "publish_accept_sequence()", "arm_marked::<C>(raw, wait)"]
    at = 0
    for mark in required:
        at = inside.index(mark, at) + len(mark)
        # The pre-offer rebase also writes sector_start_raw, without arming.
        outside_allowed = 1 if mark == "sector_start_raw.store" else 0
        if body.count(mark) != inside.count(mark) + outside_allowed:
            raise ValueError("publication/arm outside window: " + mark)
    for mark in ["note_margin(", "beat_row::<C>", "late_arms.store", "spent_max.store",
                 "finish_accept::<C>"]:
        if mark in inside or mark not in body:
            raise ValueError("bookkeeping moved into window or lost: " + mark)
    if body.index("zc.offer_timed") > body.index("arm_marked::<C>"):
        raise ValueError("arm before qualification")


class EntryArmContainment(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.src = (ROOT / "src/roots.rs").read_text(encoding="utf-8")

    def test_actual_source(self):
        check_crossing(self.src)

    def test_acceptance_window_actual_source(self):
        check_acceptance_window(self.src)

    def test_acceptance_window_rejects_bookkeeping_inside(self):
        begin = self.src.index("fn det_decide_plain_window<")
        tail = self.src[begin:].replace("Some((wait, arm_marked::<C>(raw, wait)))",
            "note_margin(wait, 0); Some((wait, arm_marked::<C>(raw, wait)))", 1)
        with self.assertRaises(ValueError):
            check_acceptance_window(self.src[:begin] + tail)

    def test_acceptance_window_rejects_arm_moved_outside(self):
        begin = self.src.index("fn det_decide_plain_window<")
        tail = self.src[begin:].replace("arm_marked::<C>(raw, wait)", "None", 1)
        tail = tail.replace("match accepted_arm {", "arm_marked::<C>(raw, wait); match accepted_arm {", 1)
        with self.assertRaises(ValueError):
            check_acceptance_window(self.src[:begin] + tail)

    def test_empty_free_with_decision_and_writes_outside_is_rejected(self):
        begin = self.src.index("pub fn com_arm_crossing(")
        tail = self.src[begin:].replace("cortex_m::interrupt::free(|_| {",
            "cortex_m::interrupt::free(|_| {}); (|| {", 1)
        with self.assertRaises(ValueError):
            check_crossing(self.src[:begin] + tail)

    def test_writes_or_validation_only_in_comments_do_not_count(self):
        for operation in ["arm_allowed", "start_crossing", "stop_expired_arm"]:
            begin = self.src.index("pub fn com_arm_crossing(")
            tail = self.src[begin:].replace(operation, "removed_operation", 1)
            tail = "// " + operation + "\n" + tail
            with self.assertRaises(ValueError):
                check_crossing(self.src[:begin] + tail)

    def test_boot_check_cannot_permanently_mask_production_handover(self):
        board = code((ROOT / "bin/board.rs").read_text(encoding="utf-8"))
        start = board.index("roots::guard_arm_tracking()")
        tail = board[start:start + 1000]
        self.assertIn("hw::nvic::unmask(stm32::Interrupt::TIM16)", tail)

    def test_full_wrap_is_an_explicit_clock_precondition_not_arithmetic_proof(self):
        elapsed = 65537
        spent16 = elapsed & 65535
        self.assertEqual(spent16, 1)
        self.assertGreater(max(20 - spent16, 0), 0)
        self.assertEqual(max(20 - elapsed, 0), 0)
        # Runtime assurance is the bounded masked path + priority-zero tracking
        # stop before re-entry, not this subtraction. Debugger/peripheral stalls
        # violating that contract are not certified by this test.


if __name__ == "__main__":
    unittest.main()
