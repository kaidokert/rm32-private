#!/usr/bin/env python3
r"""Capture the ENGAGE transient of am32_clone: from a stopped rotor, apply
throttle and grab the first N commutation intervals (ci) off the ZC_TRACE
stream — spanning open-loop spin-up → closed lock in one window.

This is the in-window regime change multi-window catch24 can detect/localize
(a commanded throttle *step* is slew-limited to ~1%/50ms, so it barely moves
within a catch24 window; the engage does not — it's an abrupt open-loop kick).

Writes a temp_study/multiwin-compatible window file (one ci per line + # META).
Kill-guarded; refuses pct > --max-pct.

Usage: python scripts/zct_engage_grab.py --pct 35 --n 512 --out captures/engage.txt
"""
import argparse
import datetime
import pathlib
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--pct", type=int, default=35)
ap.add_argument("--n", type=int, default=512)
ap.add_argument("--max-pct", type=int, default=50)
ap.add_argument("--out", default="captures/engage.txt")
a = ap.parse_args()

if a.pct > a.max_pct:
    sys.exit(f"pct {a.pct}% exceeds --max-pct {a.max_pct}%")


def parse_ci(buf, out):
    i = 0
    while i + 15 <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xA9 and (buf[i + 2] & 0x07) < 7:
            ci = buf[i + 5] | (buf[i + 6] << 8)
            if 30 <= ci <= 20000:
                out.append(ci)
            i += 15
        else:
            i += 1
    return buf[i:]


cis = []
ser = serial.Serial(a.port, a.baud, timeout=0.05)
try:
    # ensure stopped + coasted to rest
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.6)
    time.sleep(2.5)
    ser.reset_input_buffer()
    # engage: apply throttle and grab the first N ci from the very first
    # commutations (the spin-up), keeping the deadman fed.
    buf = bytearray()
    t0 = time.monotonic()
    last = 0.0
    while len(cis) < a.n and time.monotonic() - t0 < a.n * 0.02 + 15:
        if time.monotonic() - last > 0.25:
            paced_write(ser, f"{a.pct}\n".encode())
            last = time.monotonic()
        buf += ser.read(65536)
        buf = parse_ci(buf, cis)
finally:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.5)
    paced_write(ser, b"w")
    time.sleep(0.2)
    ser.close()

if len(cis) < a.n:
    sys.exit(f"only captured {len(cis)} ci (engage may have failed)")

cis = cis[:a.n]
head = sum(cis[:32]) / 32
tail = sum(cis[-32:]) / 32
captured = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
pathlib.Path(a.out).parent.mkdir(parents=True, exist_ok=True)
with open(a.out, "w") as f:
    f.write(f"# META hz=0 captured={captured} n={a.n} label=engage_{a.pct}pct\n")
    for v in cis:
        f.write(f"{v}\n")
print(f"wrote {a.n} ci to {a.out}  head(32)={head:.0f} tail(32)={tail:.0f} "
      f"ticks  range {min(cis)}..{max(cis)}")
