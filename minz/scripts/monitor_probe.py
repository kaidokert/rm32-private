#!/usr/bin/env python3
r"""am32_clone firmware self-monitor probe (build with --features monitor).

Ladders throttle through --levels (percent) and reads the firmware `mon`
line at each dwell: cumulative + per-step surprises, per-update DWT cost,
and the interval EW mean/variance the surprise band is built on. Both the
`i` and `mon` text lines are regex-extracted from the mixed ZC-trace
binary stream. Kill-guarded: 0\n x3 + w on every exit path.

Bench safety: refuses any level above --max-pct (default 50).

Usage: python scripts/monitor_probe.py --levels 20,35,50 --dwell 3
"""
import argparse
import pathlib
import re
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--levels", default="20,35,50")
ap.add_argument("--dwell", type=float, default=3.0)
ap.add_argument("--max-pct", type=int, default=50, help="bench safety ceiling")
ap.add_argument("--tier", type=int, default=None,
                help="cycle 'm' to reach this tier (0/1/2) before the ladder")
ap.add_argument("--k", type=int, default=None,
                help="drive 'k'/'K' to reach this sensitivity before the ladder")
a = ap.parse_args()

levels = [int(x) for x in a.levels.split(",")]
for lvl in levels:
    if lvl > a.max_pct:
        sys.exit(f"level {lvl}% exceeds --max-pct {a.max_pct}% (bench ceiling)")

I_RE = re.compile(
    rb"i step=(\d+) old=(\d+) run=(\d+) ci=(\d+) avg=(\d+) zc=(\d+) "
    rb"duty=(\d+) iraw=(\d+) vbat=(\d+)"
)
MON_RE = re.compile(
    rb"mon tier=(\d+) k=(\d+) n=(\d+) cyc=(\d+) min=(\d+) "
    rb"isurp=(\d+) imean=(-?\d+) ivar=(\d+) "
    rb"csurp=(\d+) cmean=(-?\d+) cvar=(\d+) "
    rb"skew=(-?\d+) kurt=(-?\d+) eps=(\d+)"
)


def poll(ser):
    ser.read(300000)  # drain ZC-trace backlog so the i/mon lines land fresh
    paced_write(ser, b"i")
    time.sleep(0.4)
    buf = ser.read(300000)
    i = None
    for i in I_RE.finditer(buf):
        pass
    m = None
    for m in MON_RE.finditer(buf):
        pass
    return (i.groups() if i else None, m.groups() if m else None)


def set_tier(ser, target):
    """Cycle 'm' (1->2->0->1) until tier==target. Readable at idle (the mon
    line prints the statics directly, no commutation needed)."""
    for _ in range(4):
        _, mg = poll(ser)
        if mg and int(mg[0]) == target:
            print(f"  tier set to {target}")
            return
        paced_write(ser, b"m")
        time.sleep(0.3)


def set_k(ser, target):
    for _ in range(15):
        _, mg = poll(ser)
        if not mg:
            return
        cur = int(mg[1])
        if cur == target:
            print(f"  K set to {target}")
            return
        paced_write(ser, b"K" if cur < target else b"k")
        time.sleep(0.2)


ser = serial.Serial(a.port, a.baud, timeout=0.05)
prev_surp = 0
try:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.6)
    time.sleep(1.5)
    if a.tier is not None:
        set_tier(ser, a.tier)
    if a.k is not None:
        set_k(ser, a.k)
    for lvl in levels:
        t0 = time.monotonic()
        while time.monotonic() - t0 < a.dwell:
            paced_write(ser, f"{lvl}\n".encode())
            time.sleep(0.4)
        ig, mg = poll(ser)
        if ig:
            run, ci, avg = ig[2].decode(), ig[3].decode(), int(ig[4])
            fe = int(2e6 / (6 * avg)) if avg > 0 else 0
            print(f"[{lvl:>3}%] run={run} ci={ci} avg={avg} ({fe:>4} Hz) "
                  f"iraw={ig[7].decode()} vbat={ig[8].decode()}")
        else:
            print(f"[{lvl:>3}%] no i line")
        if mg:
            (tier, k, n, cyc, cmin, isurp, imean, ivar,
             csurp, cmean, cvar, skew, kurt, eps) = (int(g) for g in mg)
            print(f"        mon[t{tier} k{k}]: n={n} floor={cmin}cyc "
                  f"| ci surp={isurp}(+{isurp - prev_surp}) mean={imean}t var={ivar} "
                  f"| cur surp={csurp} mean={cmean} var={cvar}")
            if tier >= 2:
                print(f"                 shape: skew={skew / 1000:+.3f} "
                      f"kurt={kurt / 1000:+.3f} epochs={eps}")
            prev_surp = isurp
        else:
            print("        mon: no line")
finally:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.5)
    paced_write(ser, b"w")
    time.sleep(0.2)
    ser.close()
    print("-- stopped --")
