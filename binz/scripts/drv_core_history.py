"""Decode mixed polling/IRQ commutation history; timestamps are not ZC times."""
import argparse
import base64
import re
import struct
import zlib
from pathlib import Path

FIELDS = 'us kind step event_interval average previous_average polling zero_crosses'.split()


def decode(text):
    header = re.search(r'^COREHISTORY n=(\d+) drop=(\d+) .*wire=h85-v1$', text, re.M)
    if not header or 'COREHISTORY END' not in text:
        raise ValueError('missing/unsupported history framing')
    count, dropped = map(int, header.groups())
    if dropped:
        raise ValueError('incomplete history: overflow')
    rows = []
    for line in text.splitlines():
        if not line.startswith('H85 '):
            continue
        raw = base64.a85decode(line[4:])
        if len(raw) != 20 or zlib.crc32(raw[:16]) != int.from_bytes(raw[16:], 'little'):
            raise ValueError('history length/CRC')
        row = dict(zip(FIELDS, struct.unpack('<8H', raw[:16])))
        if row['kind'] not in (0, 6) or not 1 <= row['step'] <= 6 or row['polling'] not in (0, 1):
            raise ValueError('history fields')
        if rows and row['us'] < rows[-1]['us']:
            raise ValueError('history timestamp order/wrap')
        rows.append(row)
    if len(rows) != count:
        raise ValueError('history count')
    return rows


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    args = parser.parse_args()
    for row in decode(args.capture.read_text()):
        print(row)
