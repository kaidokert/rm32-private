#!/usr/bin/env python3
"""Decode the reference's driven-stage DI85 rows from a binz capture.

The qualified image prints every driven acceptance as a `DI85` snapshot:
little-endian u16 words packed with a CRC32, then ASCII85-encoded
(`binz/examples/support/snapshot.rs::record`). Fields, from the capture's own
`DI85FIELDS` line:

    epoch, step, before_us, after_us, interval_half_us, requested_arr,
    previous_accept_exists

firmware50 reads these to compare *where in its sector* the reference's
driven accepts land with where firmware50's do (LAB_NOTEBOOK E060). The
command times needed for that come from the `DC85`-style command rows when a
capture has them; otherwise the interval pattern alone is printed.

Usage:
    python scripts/decode_di85.py ../captures/<capture>.txt
"""

from __future__ import annotations

import base64
import struct
import sys
import zlib


def decode(payload: str) -> list[int] | None:
    raw = base64.a85decode(payload.strip())
    if len(raw) < 6:
        return None
    body, crc = raw[:-4], struct.unpack("<I", raw[-4:])[0]
    if zlib.crc32(body) & 0xFFFFFFFF != crc:
        return None
    return list(struct.unpack(f"<{len(body) // 2}H", body))


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    rows = []
    for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
        if line.startswith("DI85 "):
            words = decode(line[5:])
            if words is None:
                print("crc-fail", line.strip())
                continue
            rows.append(words)
    print("epoch step before_us after_us interval_us(half/2) arr prev  d_before_us")
    prev = None
    for w in rows:
        epoch, step, before, after, interval_half, arr, prev_exists = w[:7]
        d = "" if prev is None else str((before - prev) & 0xFFFF)
        print(f"{epoch:5} {step:4} {before:9} {after:8} {interval_half / 2:10.1f} "
              f"{arr:4} {prev_exists:4}  {d}")
        prev = before
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
