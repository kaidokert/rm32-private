#!/usr/bin/env python3
"""Start/stop reliability count for am32_clone: N cycles of
(idle -> 20% -> confirm running via i-poll -> stop). Prints per-cycle
verdicts and the tally.

Usage: python scripts/clone_starts.py --n 8
"""

import argparse
import pathlib
import re
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

PORT = "COM41"
BAUD = 2_000_000

ap = argparse.ArgumentParser()
ap.add_argument("--n", type=int, default=8)
ap.add_argument("--pct", type=int, default=20)
ap.add_argument("--debug", action="store_true",
                help="one verbose cycle: print the raw printable wire")
a = ap.parse_args()

ser = serial.Serial(PORT, BAUD, timeout=0.05)
ok = 0

if a.debug:
    try:
        paced_write(ser, b"0\n")
        time.sleep(1.5)
        ser.reset_input_buffer()
        paced_write(ser, f"{a.pct}\n".encode())
        time.sleep(1.0)
        paced_write(ser, f"{a.pct}\n".encode())
        time.sleep(2.0)
        paced_write(ser, b"i")
        time.sleep(0.5)
        buf = ser.read(200000)
        printable = bytes(b for b in buf if 32 <= b < 127 or b == 10)
        print("wire tail:", printable[-400:])
    finally:
        paced_write(ser, b"0\n")
        time.sleep(0.5)
        paced_write(ser, b"0\n")
        ser.close()
    sys.exit(0)

try:
    # clean arm
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.7)
    time.sleep(2.0)
    for cyc in range(a.n):
        ser.reset_input_buffer()
        t0 = time.monotonic()
        last = 0.0
        started = False
        fe = 0
        while time.monotonic() - t0 < 6.0:
            if time.monotonic() - last > 0.8:
                paced_write(ser, f"{a.pct}\n".encode())
                last = time.monotonic()
            # drain the trace backlog, THEN poke `i` and read fresh
            # (the 0/8 incident: without the drain, the 64 KiB read
            # window was all backlog and the info line never landed).
            ser.read(200000)
            paced_write(ser, b"i")
            time.sleep(0.4)
            buf = ser.read(200000)
            m = None
            for m in re.finditer(
                    rb"i step=\d+ old=(\d+) run=(\d+) ci=\d+ avg=(\d+)",
                    buf):
                pass
            if m and m.group(2) == b"1" and int(m.group(3)) > 0:
                avg = int(m.group(3))
                if avg < 5000:  # < 2.5 ms => actually commutating
                    fe = int(2e6 / (6 * avg))
                    started = True
                    break
        tag = "START" if started else "FAIL"
        print(f"cycle {cyc + 1}: {tag}"
              + (f" ({fe} Hz, {time.monotonic() - t0:.1f}s)"
                 if started else ""))
        if started:
            ok += 1
        # stop + coast
        for _ in range(2):
            paced_write(ser, b"0\n")
            time.sleep(0.6)
        time.sleep(2.5)
finally:
    paced_write(ser, b"0\n")
    time.sleep(0.5)
    paced_write(ser, b"0\n")
    ser.close()
print(f"\n{ok}/{a.n} starts")
