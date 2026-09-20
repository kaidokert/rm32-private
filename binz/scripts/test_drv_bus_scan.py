import base64
import struct
import unittest
import zlib

from drv_bus_scan import decode, timestamp_events


def record(stamp, bus=1200):
    raw = struct.pack("<8H", stamp & 65535, stamp >> 16, bus, 1500,
                      2048, 2050, 2046, 450)
    return "BS85 " + base64.a85encode(raw + struct.pack("<I", zlib.crc32(raw))).decode()


class BusScanTest(unittest.TestCase):
    def test_crc_order_and_values(self):
        text = ("BUSSCAN n=2 total=2 fields=stamp_lo,stamp_hi,bus,vref,ia,ib,ic,duty "
                "chronological=1 diagnostic_only=1\n" + record(500000, 1190) + "\n"
                + record(500226, 1120) + "\n")
        rows = decode(text)
        self.assertEqual([r["stamp_us"] for r in rows], [500000, 500226])
        self.assertEqual([r["bus"] for r in rows], [1190, 1120])
        valid = record(500000, 1190)
        corrupt = valid[:-1] + ("!" if valid[-1] != "!" else '"')
        with self.assertRaisesRegex(ValueError, "CRC"):
            decode(text.replace(valid, corrupt))

    def test_nonmonotone_refused(self):
        text = ("BUSSCAN n=2 total=2 fields=stamp_lo,stamp_hi,bus,vref,ia,ib,ic,duty\n"
                + record(500226) + "\n" + record(500000) + "\n")
        with self.assertRaisesRegex(ValueError, "strictly increasing"):
            decode(text)

    def test_event_anchor_uses_following_gap(self):
        events = [dict(ordinal=10, gap_us=70), dict(ordinal=11, gap_us=80),
                  dict(ordinal=12, gap_us=90)]
        stamped = timestamp_events(events, 1000)
        self.assertEqual([x["stamp_us"] for x in stamped], [830, 910, 1000])


if __name__ == "__main__":
    unittest.main()
