#!/usr/bin/env python3
"""Collect a window of internal-temperature-sensor samples from temp_capture.rs.

Reads the "T <raw>" stream (one 12-bit ADC count per line) plus the one-time
"TSCAL ..." banner, and writes a window file the ratch22-probe temp_study reads:

    # TSCAL cal1=.. cal2=.. t1=.. t2=.. vdda_calib=..
    <raw>
    <raw>
    ...

Usage: python scripts/temp_grab.py --n 512 --out captures/temp_window.txt
"""
import argparse
import datetime
import pathlib
import re
import sys
import time

import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--n", type=int, default=512)
ap.add_argument("--hz", type=float, default=20.0, help="firmware sample cadence")
ap.add_argument("--label", default="", help="capture identity note (e.g. 'heated')")
ap.add_argument("--out", default="captures/temp_window.txt")
a = ap.parse_args()

T_RE = re.compile(r"^T (\d+)")
CAL_RE = re.compile(
    r"TSCAL cal1=(\d+) cal2=(\d+) t1=(-?\d+) t2=(-?\d+) vdda_calib=(\d+)"
)

samples = []
cal = None
with serial.Serial(a.port, a.baud, timeout=1.0) as p:
    p.reset_input_buffer()
    buf = ""
    t0 = time.monotonic()
    deadline = a.n * 0.1 + 20
    while len(samples) < a.n and time.monotonic() - t0 < deadline:
        buf += p.read(4096).decode("ascii", errors="replace")
        while "\n" in buf:
            line, buf = buf.split("\n", 1)
            line = line.strip()
            m = CAL_RE.search(line)
            if m:
                cal = m.groups()
            m = T_RE.match(line)
            if m:
                samples.append(int(m.group(1)))
                if len(samples) % 64 == 0:
                    print(f"  {len(samples)}/{a.n}", end="\r")
print()

if len(samples) < a.n:
    sys.exit(f"only got {len(samples)} samples (is temp_capture flashed & streaming?)")

pathlib.Path(a.out).parent.mkdir(parents=True, exist_ok=True)
captured = datetime.datetime.now().astimezone().isoformat(timespec="seconds")
with open(a.out, "w") as f:
    # Provenance the ratch22 agent asked us to preserve with every capture:
    # cadence + capture identity (unit conversion via the TSCAL line below).
    f.write(f"# META hz={a.hz} captured={captured} n={a.n} label={a.label or 'idle'}\n")
    if cal:
        f.write(
            f"# TSCAL cal1={cal[0]} cal2={cal[1]} t1={cal[2]} t2={cal[3]} "
            f"vdda_calib={cal[4]}\n"
        )
    for s in samples[:a.n]:
        f.write(f"{s}\n")

rng = f"{min(samples)}..{max(samples)}"
print(f"wrote {a.n} samples to {a.out} (raw range {rng}, cal={cal})")
