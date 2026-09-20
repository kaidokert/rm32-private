import base64
import struct
import unittest
import zlib
from drv_core_history import decode


def frame(rows, drop=0):
    lines = [f'COREHISTORY n={len(rows)} drop={drop} fields=test wire=h85-v1']
    for row in rows:
        raw = struct.pack('<8H', *row)
        raw += struct.pack('<I', zlib.crc32(raw))
        lines.append('H85 ' + base64.a85encode(raw).decode())
    return '\n'.join(lines + ['COREHISTORY END'])


class HistoryTests(unittest.TestCase):
    def test_modes_and_desync_preserved(self):
        rows = decode(frame([(10, 0, 2, 1666, 1666, 1666, 1, 0),
                             (100, 6, 1, 1806, 1806, 3000, 1, 0)]))
        self.assertEqual(rows[1]['previous_average'], 3000)
        self.assertEqual(rows[1]['kind'], 6)

    def test_reject_overflow_truncation_bad_fields_and_time(self):
        row = (10, 0, 2, 1666, 1666, 1666, 1, 0)
        for text in (frame([row], 1), frame([row]).replace('n=1', 'n=2'),
                     frame([row]).replace('COREHISTORY END', ''),
                     frame([row, (9, *row[1:])]),
                     frame([(10, 0, 7, *row[3:])])):
            with self.assertRaises(ValueError): decode(text)

    def test_crc_rejected(self):
        text = frame([(10, 0, 2, 1666, 1666, 1666, 1, 0)])
        lines = text.splitlines()
        raw = bytearray(base64.a85decode(lines[1][4:]))
        raw[0] ^= 1
        lines[1] = 'H85 ' + base64.a85encode(raw).decode()
        with self.assertRaises(ValueError): decode('\n'.join(lines))
