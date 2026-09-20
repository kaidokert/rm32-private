"""Host-only regression for MCU-stamped target-duty dwell verification."""

import unittest

from live_armed_baseline import target_dwell_report


LINE = (
    "LIVEACK target_tenths=500 seen=1 accepted_us=20000000 "
    "stop_us=55000000 age_us=35000000 same_powered_segment=1 "
    "ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1\n"
)


class TargetDwellTests(unittest.TestCase):
    def test_actual_dwell_passes(self):
        self.assertEqual(target_dwell_report(LINE, 500, 55_000_000, 30_000)["dwell_us"], 35_000_000)

    def test_short_dwell_refused(self):
        with self.assertRaisesRegex(RuntimeError, "too short"):
            target_dwell_report(LINE, 500, 55_000_000, 36_000)

    def test_wrong_segment_refused(self):
        with self.assertRaisesRegex(RuntimeError, "incoherent"):
            target_dwell_report(LINE, 500, 54_000_000, 30_000)

    def test_missing_stamp_refused(self):
        with self.assertRaisesRegex(RuntimeError, "missing unique"):
            target_dwell_report("", 500, 55_000_000, 30_000)

    def test_foreground_normal_stop_uses_conservative_last_event(self):
        text = (
            "LIVEACK target_tenths=500 seen=1 accepted_us=23632885 stop_us=0 age_us=0 "
            "same_powered_segment=1 ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1\n"
            "POWERPATH reason=0 stop_us=0 active=0 disabled=1\n"
            "TRACKSTOP event_fault=0 last_event_us=54999917 sector=4\n"
            "DRIVENENTRY refusal=0 adopt_refusal=0 coast_stop=1 fly_seeded=1 postrun_only=1\n"
        )
        result=target_dwell_report(text,500,54_999_917,30_000)
        self.assertEqual(result['dwell_us'],31_367_032)
        self.assertEqual(result['source'],'last_accepted_event_lower_bound')

    def test_foreground_fault_cannot_be_a_hold(self):
        text = (
            "LIVEACK target_tenths=500 seen=1 accepted_us=23632885 stop_us=0 age_us=0 "
            "same_powered_segment=1 ack_queued=1 pwm_transfer_within_one_carrier=1 postrun_only=1\n"
            "POWERPATH reason=26 stop_us=0 active=0 disabled=1\n"
            "TRACKSTOP event_fault=0 last_event_us=54999917 sector=4\n"
            "DRIVENENTRY refusal=0 adopt_refusal=0 coast_stop=1 fly_seeded=1 postrun_only=1\n"
        )
        with self.assertRaisesRegex(RuntimeError,'normal-stop'):
            target_dwell_report(text,500,54_999_917,30_000)


if __name__ == "__main__":
    unittest.main()
