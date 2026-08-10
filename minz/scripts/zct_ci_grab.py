#!/usr/bin/env python3
r"""Spin am32_clone at a throttle and capture a window of commutation
INTERVALS (ci, 0.5 us ticks) from the ZC_TRACE stream (0x5B 0xA9 sync,
ci at bytes 5:7) for ratch22 catch24 analysis.

Writes a temp_study-compatible window file (one ci per line + # META).
Kill-guarded (0\n x3 + w on every exit); refuses pct > --max-pct.

Usage: python scripts/zct_ci_grab.py --pct 35 --n 512 --out captures/ci_35.txt
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
ap.add_argument("--max-pct", type=int, default=50, help="bench safety ceiling")
ap.add_argument("--settle", type=float, default=2.5, help="lock time before capture")
ap.add_argument("--out", default="captures/ci_window.txt")
a = ap.parse_args()

if a.pct > a.max_pct:
    sys.exit(f"pct {a.pct}% exceeds --max-pct {a.max_pct}% (bench ceiling)")


def parse_ci(buf, out):
    """Scan a bytearray for ZC_TRACE records, append plausible ci values.
    Returns the unconsumed tail."""
    i = 0
    while i + 15 <= len(buf):
        if buf[i] == 0x5B and buf[i + 1] == 0xA9 and (buf[i + 2] & 0x07) < 7:
            ci = buf[i + 5] | (buf[i + 6] << 8)
            if 30 <= ci <= 20000:  # sane commutation interval (0.5 us ticks)
                out.append(ci)
            i += 15
        else:
            i += 1
    return buf[i:]


cis = []
ser = serial.Serial(a.port, a.baud, timeout=0.05)
try:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.6)
    time.sleep(1.0)
    # spin up + settle
    t0 = time.monotonic()
    while time.monotonic() - t0 < a.settle:
        paced_write(ser, f"{a.pct}\n".encode())
        time.sleep(0.3)
    ser.reset_input_buffer()
    # capture
    buf = bytearray()
    t0 = time.monotonic()
    last = 0.0
    while len(cis) < a.n and time.monotonic() - t0 < a.n * 0.02 + 20:
        if time.monotonic() - last > 0.3:
            paced_write(ser, f"{a.pct}\n".encode())  # keep the deadman alive
            last = time.monotonic()
        buf += ser.read(65536)
        buf = parse_ci(buf, cis)
        if len(cis) % 128 < 4:
            print(f"  {len(cis)}/{a.n}", end="\r")
finally:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.5)
    paced_write(ser, b"w")
    time.sleep(0.2)
    ser.close()
print()

if len(cis) < a.n:
    sys.exit(f"only captured {len(cis)} ci records")

cis = cis[:a.n]
mean = sum(cis) / len(cis)
fe = 2e6 / (6 * mean) if mean > 0 else 0  # 0.5us ticks -> electrical Hz
captured = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
pathlib.Path(a.out).parent.mkdir(parents=True, exist_ok=True)
with open(a.out, "w") as f:
    # per-commutation series: cadence is the mean commutation rate (Hz),
    # so the study's freq axis reads in electrical-cycle terms.
    comm_hz = 2e6 / mean if mean > 0 else 0
    f.write(f"# META hz={comm_hz:.1f} captured={captured} n={a.n} "
            f"label=motor_{a.pct}pct\n")
    for v in cis:
        f.write(f"{v}\n")
print(f"wrote {a.n} ci to {a.out}  mean={mean:.0f} ticks (~{fe:.0f} Hz elec), "
      f"range {min(cis)}..{max(cis)}")
