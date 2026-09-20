import base64
import struct
import unittest
import zlib
from drv_core_trace import validate


def capture(steps):
    lines = [f'CORETRACE ci=1667 records={len(steps)} format=zct_bytes_as_u16_a85_crc']
    for step in steps:
        raw = b'\x5b\xa9' + bytes([step]) + struct.pack('<6H',1667,1667,417,0,0,1667)
        data = struct.pack('<15H', *raw)
        data += struct.pack('<I', zlib.crc32(data))
        lines.append('Z85 ' + base64.a85encode(data).decode())
    return '\n'.join(lines + ['CORETRACE END'])


class CoreTraceTests(unittest.TestCase):
    def test_consecutive_wrap(self):
        self.assertEqual(len(validate(capture([5,6,1,2]))[0]['rows']), 4)

    def test_missing_step(self):
        with self.assertRaises(ValueError):
            validate(capture([1,3]))

    def test_truncation(self):
        with self.assertRaises(ValueError):
            validate(capture([1]).replace('CORETRACE END',''))

    def test_crc(self):
        text = capture([1])
        start = text.index('Z85 ') + 4
        text = text[:start] + ('!' if text[start] != '!' else '"') + text[start+1:]
        with self.assertRaises(ValueError):
            validate(text)
