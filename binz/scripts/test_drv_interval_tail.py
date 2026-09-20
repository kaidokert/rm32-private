import base64
import struct
import unittest
import zlib

from drv_interval_tail import (
    complete_cycles,
    decode,
    speed_event_limit_us,
    speed_watch_violations,
    verify_speed_watch,
)


def record(first, pairs):
    words = [first & 0xFFFF, first >> 16]
    for gap, reference in pairs:
        words.extend([gap, reference])
    while len(words) < 10:
        words.extend([0, 0])
    raw = struct.pack("<10H", *words)
    return "IT85 " + base64.a85encode(raw + struct.pack("<I", zlib.crc32(raw))).decode()


def record_v2(first, rows, tag="IT86"):
    words = [first & 0xFFFF, first >> 16]
    for row in rows:
        words.extend(row)
    while len(words) < 10:
        words.append(0)
    raw = struct.pack("<10H", *words)
    return tag + " " + base64.a85encode(raw + struct.pack("<I", zlib.crc32(raw))).decode()


class IntervalTail(unittest.TestCase):
    def test_decode_names_measured_interval_without_policy_claim(self):
        text = (
            "INTERVALTAIL n=5 skipped=0 total=5 fields=x\n"
            + record(0, [(100, 200), (110, 210), (250, 900), (120, 220)])
            + "\n"
            + record(4, [(400, 220)])
            + "\nEND\n"
        )
        events = decode(text)
        self.assertEqual([event["ordinal"] for event in events], list(range(5)))
        self.assertEqual(events[2]["measured_interval_half_us"], 900)
        self.assertNotIn("reference_half_us", events[2])

    def test_complete_cycles_discards_partial_edges(self):
        gaps = [1, 2, 3, 4, 10, 20, 30, 40, 50, 60]
        events = [
            {"ordinal": ordinal, "gap_us": gap, "measured_interval_half_us": 2 * gap}
            for ordinal, gap in zip(range(2, 12), gaps)
        ]
        cycles = complete_cycles(events)
        self.assertEqual(
            cycles,
            [{"cycle_ordinal": 1, "period_us": 210, "gaps_us": (10, 20, 30, 40, 50, 60)}],
        )

    def test_crc_and_bounds_are_strict(self):
        good = "INTERVALTAIL n=1 skipped=0 total=1 fields=x\n" + record(0, [(1, 2)])
        with self.assertRaises(ValueError):
            decode(good.replace("n=1", "n=2"))
        with self.assertRaises(ValueError):
            decode(good[:-1] + ("!" if good[-1] != "!" else '"'))

    def test_v2_preserves_average_limit_and_exact_violation(self):
        text = (
            "INTERVALTAIL n=3 skipped=0 total=3 "
            "fields=ordinal_lo,ordinal_hi,gap_us,measured_interval_half_us,"
            "average_interval_half_us,event_limit_us two_per_row=1 wire=it86-v2\n"
            + record_v2(0, [(100, 200, 180, 270), (280, 560, 185, 270)])
            + "\n"
            + record_v2(2, [(120, 240, 190, 270)])
            + "\nEND\n"
        )
        events = decode(text)
        self.assertEqual(events[1]["average_interval_half_us"], 185)
        self.assertEqual(events[1]["event_limit_us"], 270)
        self.assertEqual(speed_watch_violations(events), [events[1]])
        retained_first = dict(events[1], ordinal=100)
        self.assertEqual(speed_watch_violations([retained_first]), [retained_first])
        with self.assertRaises(ValueError):
            speed_watch_violations(decode(text.replace("IT86", "IT85").replace("average_interval_half_us,", "")))

    def test_v3_unpacks_origin_without_changing_deadline_math(self):
        text = (
            "INTERVALTAIL n=2 skipped=0 total=2 "
            "fields=ordinal_lo,ordinal_hi,gap_us,measured_interval_half_us,"
            "average_interval_half_us,event_limit_us+origin2 two_per_row=1 wire=it87-v3\n"
            + record_v2(0, [(100, 200, 180, 270), (280, 560, 185, 270 | (1 << 14))], "IT87")
            + "\nEND\n"
        )
        events = decode(text)
        self.assertEqual([event["origin"] for event in events], [0, 1])
        self.assertEqual(events[1]["event_limit_us"], 270)
        self.assertEqual(speed_watch_violations(events), [events[1]])

    def test_v2_transition_verifier_matches_firmware_order(self):
        events = [
            {"ordinal": 10, "gap_us": 100, "measured_interval_half_us": 200,
             "average_interval_half_us": 200, "event_limit_us": 1000},
            {"ordinal": 11, "gap_us": 110, "measured_interval_half_us": 220,
             "average_interval_half_us": 180, "event_limit_us": 300},
            {"ordinal": 12, "gap_us": 120, "measured_interval_half_us": 240,
             "average_interval_half_us": 220, "event_limit_us": 270},
        ]
        self.assertEqual(speed_event_limit_us(200), 300)
        self.assertEqual(
            verify_speed_watch(events, final_limit_us=270),
            {"events": 3, "transitions": 2, "violations": 0,
             "expected_final_limit_us": 270},
        )
        broken = [dict(event) for event in events]
        broken[2]["event_limit_us"] = 271
        with self.assertRaises(ValueError):
            verify_speed_watch(broken)


if __name__ == "__main__":
    unittest.main()
