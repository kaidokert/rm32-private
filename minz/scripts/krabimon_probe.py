#!/usr/bin/env python3
r"""am32_clone krabilorean-consumer probe (build with --features krabimon).

The consumer-validation counterpart to monitor_probe.py, but for the sibling
crate `krabilorean` (git-pinned tag v0.1.0-alpha.1). Ladders throttle through
--levels (percent) and reads the firmware `krab` + `krab.w` lines at each
dwell: online core_merge per-channel stats + measured online DWT cost, and
the windowed BasicProfile batch cost + histogram mode + autocorrelation
regularity markers.

Both the `i`, `krab`, and `krab.w` text lines are regex-extracted from the
mixed ZC-trace binary stream. Kill-guarded: 0\n x3 + w on every exit path,
plus a per-dwell current ceiling (--max-ma) that aborts the ladder.

Usage: python scripts/krabimon_probe.py --levels 20,30 --dwell 3 --tier 2
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
ap.add_argument("--levels", default="20,30")
ap.add_argument("--dwell", type=float, default=3.0)
ap.add_argument("--max-pct", type=int, default=45, help="bench safety ceiling")
ap.add_argument("--max-ma", type=int, default=900, help="per-dwell current abort")
ap.add_argument("--tier", type=int, default=2,
                help="cycle 'm' to reach this tier (0/1/2) before the ladder")
a = ap.parse_args()

levels = [int(x) for x in a.levels.split(",")]
for lvl in levels:
    if lvl > a.max_pct:
        sys.exit(f"level {lvl}% exceeds --max-pct {a.max_pct}% (bench ceiling)")

I_RE = re.compile(
    rb"i step=(\d+) old=(\d+) run=(\d+) ci=(\d+) avg=(\d+) zc=(\d+) "
    rb"duty=(\d+) iraw=(\d+) vbat=(\d+)"
)
KRAB_RE = re.compile(
    rb"krab tier=(\d+) n=(\d+) cyc=(\d+) min=(\d+) \| "
    rb"i\[mn=(-?\d+) mx=(-?\d+) avg=(-?\d+) var=(-?\d+) mad=(-?\d+)\] "
    rb"c\[mn=(-?\d+) mx=(-?\d+) avg=(-?\d+) mad=(-?\d+)\]"
)
KRABW_RE = re.compile(
    rb"krab\.w win=(\d+) wcyc=(\d+) wmin=(\d+) wvar=(-?\d+) mode=(-?\d+) "
    rb"acf1=(-?\d+) zc=(-?\d+) lmin=(-?\d+)"
)


def last(rx, buf):
    m = None
    for m in rx.finditer(buf):
        pass
    return m.groups() if m else None


def poll(ser):
    ser.read(400000)  # drain ZC-trace backlog so the text lines land fresh
    paced_write(ser, b"i")
    time.sleep(0.4)
    buf = ser.read(400000)
    return last(I_RE, buf), last(KRAB_RE, buf), last(KRABW_RE, buf)


def set_tier(ser, target):
    for _ in range(4):
        _, kg, _ = poll(ser)
        if kg and int(kg[0]) == target:
            print(f"  tier set to {target}")
            return
        paced_write(ser, b"m")
        time.sleep(0.3)


ser = serial.Serial(a.port, a.baud, timeout=0.05)
try:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.6)
    time.sleep(1.5)
    set_tier(ser, a.tier)

    for lvl in levels:
        t0 = time.monotonic()
        aborted = False
        while time.monotonic() - t0 < a.dwell:
            paced_write(ser, f"{lvl}\n".encode())
            time.sleep(0.4)
        ig, kg, kw = poll(ser)
        if ig:
            run, ci, avg = ig[2].decode(), ig[3].decode(), int(ig[4])
            iraw = int(ig[7])
            ma = iraw * 3300 / 4095 / 30 * 1000  # 30 mV/A sense
            fe = int(2e6 / (6 * avg)) if avg > 0 else 0
            print(f"[{lvl:>3}%] run={run} ci={ci} avg={avg} ({fe:>4} Hz) "
                  f"iraw={iraw} (~{ma:.0f} mA) vbat={ig[8].decode()}")
            if ma > a.max_ma:
                print(f"  !! {ma:.0f} mA > {a.max_ma} — aborting ladder")
                aborted = True
        else:
            print(f"[{lvl:>3}%] no i line")
        if kg:
            (tier, n, cyc, cmin, imn, imx, iavg, ivar, imad,
             cmn, cmx, cavg, cmad) = (int(g) for g in kg)
            print(f"        krab[t{tier}]: n={n} online={cyc}cyc floor={cmin}cyc")
            print(f"          interval: min={imn} max={imx} mean={iavg}t "
                  f"var={ivar}t^2 mad={imad}t")
            print(f"          current:  min={cmn} max={cmx} mean={cavg} mad={cmad} (raw)")
        else:
            print("        krab: no line")
        if kw:
            win, wcyc, wmin, wvar, mode, acf1, zc, lmin = (int(x) for x in kw)
            print(f"          windowed: epochs={win} batch={wcyc}cyc floor={wmin}cyc "
                  f"| var={wvar}t^2 mode_bin={mode}")
            print(f"          autocorr: lag1={acf1 / 1000:+.3f} "
                  f"first_zero_cross=lag{zc} first_local_min=lag{lmin}")
        else:
            print("          windowed: no line")
        if aborted:
            break
finally:
    for _ in range(3):
        paced_write(ser, b"0\n")
        time.sleep(0.5)
    paced_write(ser, b"w")
    time.sleep(0.2)
    ser.close()
    print("-- stopped --")
