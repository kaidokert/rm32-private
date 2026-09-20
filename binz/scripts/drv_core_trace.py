"""Validate fixture-only CORETRACE framing/CRC and consecutive AM32 ZCT steps.

This checks recorded ordering, NOT full control replay or physical rotor lock.
"""
import argparse
import base64
import re
import struct
import zlib
from pathlib import Path


def validate(text):
    blocks = []
    active = None
    for line in text.splitlines():
        if line.startswith('CORETRACE ci='):
            if active is not None:
                raise ValueError('missing trace end')
            match = re.fullmatch(r'CORETRACE ci=(\d+) records=(\d+) format=zct_bytes_as_u16_a85_crc', line)
            if not match:
                raise ValueError('unknown trace header')
            active = dict(ci=int(match[1]), expected=int(match[2]), rows=[])
        elif line.startswith('Z85 '):
            if active is None:
                raise ValueError('orphan record')
            data = base64.a85decode(line[4:])
            if len(data) != 34 or zlib.crc32(data[:-4]) != int.from_bytes(data[-4:], 'little'):
                raise ValueError('record size/CRC')
            words = struct.unpack('<15H', data[:-4])
            if any(w > 255 for w in words):
                raise ValueError('not packed bytes')
            raw = bytes(words)
            if raw[:2] != b'\x5b\xa9' or not 1 <= raw[2] & 7 <= 6:
                raise ValueError('ZCT sync/step')
            step = raw[2] & 7
            rows = active['rows']
            if rows and step != rows[-1][0] % 6 + 1:
                raise ValueError('nonconsecutive steps')
            rows.append((step, *struct.unpack('<6H', raw[3:])))
        elif line == 'CORETRACE END':
            if active is None or len(active['rows']) != active['expected']:
                raise ValueError('trace length')
            blocks.append(active)
            active = None
    if active is not None or not blocks:
        raise ValueError('incomplete/no trace')
    return blocks


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    args = parser.parse_args()
    for block in validate(args.capture.read_text()):
        rows = block['rows']
        print(f"ci={block['ci']} records={len(rows)} CRC/order=PASS "
              f"thiszc_range={min(r[1] for r in rows)}..{max(r[1] for r in rows)}")
