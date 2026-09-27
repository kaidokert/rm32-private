"""Host-only regression and mutation tests; never open a serial port."""
import tempfile
import unittest
import re
from pathlib import Path

import propless

FIXTURES = Path(__file__).resolve().parent / "testdata"
RAW = FIXTURES / "e359-propless150_01.txt"


class UnloadedChecks(unittest.TestCase):
    def test_absolute_selection_from_every_authorized_previous_state(self):
        for target in (600, 650, 700, 750, 800):
            for start in range(375, 801, 25):
                duty = start
                for key in propless.absolute_climb_pre(target):
                    duty = max(375, duty - 25) if key == "-" else min(800, duty + 25)
                self.assertEqual(duty, target)
        with self.assertRaises(ValueError):
            propless.absolute_climb_pre(825)

    def test_duplicate_or_misplaced_off_records_refused(self):
        text = RAW.read_text(encoding="utf-8")
        done = next(s for s in text.splitlines() if s.startswith("BEMFDONE "))
        self.assertTrue(self.check_text(text + "\n" + done))
        self.assertTrue(self.check_text(text.replace("POSTSTOP", "INVALIDSTOP")))

    def test_request_validation_precedes_hardware(self):
        self.assertEqual(propless.request_errors("9", 150, "", False), [])
        self.assertEqual(propless.request_errors("8", 250, "", False), [])
        self.assertEqual(propless.request_errors("l", 600, "++++++++", False), [])
        for args in (("9", 600, "", False), ("l", 600, "", False),
                     ("l", 600, "f", False), ("l", 850, "+", False),
                     ("Z", 600, "xxxx", False), ("9", 150, "", True),
                     ("8", 150, "", False), ("8", 250, "+", False),
                     ("8", 250, "", True), ("5", 250, "", False)):
            self.assertTrue(propless.request_errors(*args))

    def check_text(self, text, duty=150, dwell=9000, period=1333):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "capture.txt"
            path.write_text(text, encoding="utf-8")
            return propless.verdict(path, duty, dwell, period=period)

    def test_carrier_is_declared_independently_and_ccr_checked(self):
        original = RAW.read_text(encoding="utf-8")
        for period, ccr in ((1333, 199), (1000, 150)):
            text = (f"# expected_run_period_ticks {period}\n" + original)
            text = text.replace("BEMFRUN ",
                                f"BEMFRUN run_period_ticks={period} startup_ticks=6400 ")
            text = text.replace("applied_ccr=199", f"applied_ccr={ccr}")
            self.assertEqual(self.check_text(text, period=period), [])
            for old, new in ((f"run_period_ticks={period}", "run_period_ticks=6400"),
                             ("startup_ticks=6400", "startup_ticks=1000"),
                             (f"# expected_run_period_ticks {period}", ""),
                             (f"applied_ccr={ccr}", "applied_ccr=1")):
                self.assertTrue(self.check_text(text.replace(old, new), period=period))
        self.assertTrue(self.check_text(original, period=1000))

    def test_actual_period_not_hidden_by_target_header(self):
        text = RAW.read_text(encoding="utf-8")
        good = text.replace("applied_ccr=199", "applied_ccr=199 applied_period=1333")
        self.assertEqual(self.check_text(good), [])
        self.assertTrue(self.check_text(good.replace("applied_period=1333", "applied_period=1000")))
        new = good.replace("BEMFRUN ", "BEMFRUN entry_period_ticks=1333 ")
        self.assertEqual(self.check_text(new), [])
        self.assertTrue(self.check_text(new.replace(" applied_period=1333", "")))

    def test_fixture_refuses_undeclared_carrier_before_hardware(self):
        import subprocess
        import sys
        run = subprocess.run([sys.executable, str(FIXTURES.parent / "bemf_run.py"),
                              "--propless", "--no-ladder", "--rung-duty", "500",
                              "--command", "l", "--pre", "++++"],
                             capture_output=True, text=True)
        self.assertEqual(run.returncode, 2)
        self.assertIn("requires --run-period-ticks", run.stderr)

    def test_actual_gentle_run_is_exploration_not_30s_qualification(self):
        self.assertEqual(propless.verdict(RAW, 150, 9000), [])
        self.assertTrue(propless.verdict(RAW, 150, 30_000))

    def test_old_no_prop50_identity_failure_is_not_removed(self):
        path = FIXTURES / "noprop-open-50pct_01.txt"
        self.assertTrue(any("rate vs coast 986" in f
                            for f in propless.verdict(path, 500, 9000)))

    def test_bad_duty_and_safety_evidence_are_load_bearing(self):
        text = RAW.read_text(encoding="utf-8")
        for old, new in (("late_arms=0", "late_arms=1"),
                         ("blank_latched=0", "blank_latched=1"),
                         ("storm=0", "storm=1"),
                         ("tripped=0", "tripped=1"),
                         ("ceiling_tenths=150", "ceiling_tenths=140"),
                         ("applied_cap=600", "applied_cap=100"),
                         ("applied_ccr=199", "applied_ccr=1"),
                         ("moe=0", "moe=1"),
                         ("reason=2", "reason=26")):
            with self.subTest(field=old):
                self.assertIn(old, text)
                self.assertTrue(self.check_text(text.replace(old, new)))
        self.assertTrue(self.check_text(text, duty=600))
        self.assertTrue(self.check_text(text.replace("BEMFRCOMP ", "MISSING ")))
        self.assertTrue(self.check_text(text.replace("PREFLIGHT ", "MISSING ")))

    def test_required_fields_do_not_inherit_legacy_defaults(self):
        text = RAW.read_text(encoding="utf-8")
        for key, fields in propless.REQUIRED.items():
            for field in fields:
                with self.subTest(record=key, field=field):
                    lines = text.splitlines()
                    for i, line in enumerate(lines):
                        if line.startswith(key + " "):
                            lines[i] = re.sub(r"(?<!\S)" + field + r"=\S+", "", line)
                    self.assertTrue(self.check_text("\n".join(lines)))
        self.assertTrue(self.check_text(text.replace("BEMFTAIL ", "MISSING ")))
        self.assertTrue(self.check_text(text.replace("span_us=2277432", "span_us=1")))

    def test_stop_before_hold_reports_fault_as_well_as_missing_identity(self):
        text = RAW.read_text(encoding="utf-8")
        text = text.replace("reason=2", "reason=26")
        text = text.replace("span_us=2277432", "span_us=0")
        fails = self.check_text(text)
        self.assertTrue(any("reason 26" in f for f in fails))
        self.assertTrue(any("identity unavailable" in f for f in fails))

    def test_sag_dump_requires_complete_frozen_versioned_evidence(self):
        dump = ("SAGSNAP judged=3 fast_len=3 slow_len=0 frozen=1 num=95 den=100 "
                "streak_to_latch=3 row_v=3 fine_hz=8000000 span16_us=8192\n"
                "SAGROW 100 800 10 1200 2048 2048 2048 1200 1500 1200 1500 0 1 500 20\n"
                "SAGROW 201 1608 12 1200 2048 2048 2048 1200 1500 1200 1500 0 2 500 22\n"
                "SAGROW 302 2416 14 1200 2048 2048 2048 1200 1500 1200 1500 0 3 500 23\n"
                "SAGEND\n")
        with tempfile.TemporaryDirectory() as folder:
            p = Path(folder) / "sag.txt"
            p.write_text(dump, encoding="utf-8")
            self.assertEqual(propless.sag_dump_errors(p), [])
            for old, new in (("SAGEND", ""), ("fast_len=3", "fast_len=4"),
                             ("frozen=1", "frozen=0"), ("num=95", "num=90"),
                             ("fine_hz=8000000", "fine_hz=64000000")):
                p.write_text(dump.replace(old, new), encoding="utf-8")
                self.assertTrue(propless.sag_dump_errors(p))


if __name__ == "__main__":
    unittest.main()
