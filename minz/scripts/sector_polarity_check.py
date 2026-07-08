#!/usr/bin/env python3
"""Empirical per-sector BEMF polarity from WAXWING cdumps.

For every A/B float window: fit the (phase − neutral) trace, report
the slope direction, whether an in-window crossing exists, and the
early/late sign pattern. Answers "does sector N's BEMF rise or fall
through neutral?" — the firmware convention says even sectors fall
(post-ZC below neutral, COMP VALUE=1) and odd sectors rise.

Usage:
    python scripts/sector_polarity_check.py captures/wax48s2_*.txt captures/probe_f100_a10_*.txt
"""

import glob
import pathlib
import statistics
import sys

import numpy as np

from magpie import parse_cdump

sys.stdout.reconfigure(errors="replace")

PHASE = {2: "a", 5: "a", 1: "b", 4: "b"}


def analyze(path):
    frames, hz = parse_cdump(pathlib.Path(path).read_text())
    n = len(frames)
    A = np.array([f["a"] for f in frames], float)
    B = np.array([f["b"] for f in frames], float)
    sector = np.array([f["sector"] for f in frames], int)
    vbus = float(np.percentile(np.maximum(A, B), 98))

    neutral = np.zeros(n)
    for i in range(n):
        s = sector[i]
        if s in (0, 3):
            neutral[i] = (A[i] + B[i]) / 2
        elif s == 1:
            neutral[i] = A[i] / 2
        elif s == 4:
            neutral[i] = (vbus + A[i]) / 2
        elif s == 2:
            neutral[i] = B[i] / 2
        else:
            neutral[i] = (vbus + B[i]) / 2

    wins, w0 = [], 0
    for i in range(1, n):
        if sector[i] != sector[i - 1]:
            wins.append((w0, i, sector[i - 1]))
            w0 = i
    lens = [b - a for a, b, _ in wins] or [1]
    med = statistics.median(lens)
    wins = [w for w in wins if 4 < w[1] - w[0] < 3 * med]

    stats = {s: dict(n=0, rising=0, falling=0, crossing=0, early_below=0, late_below=0) for s in (1, 2, 4, 5)}
    for s0, s1, sec in wins:
        if sec not in PHASE:
            continue
        trace = A if PHASE[sec] == "a" else B
        lo = s0 + 2  # skip flyback frames
        e = trace[lo:s1] - neutral[lo:s1]
        if len(e) < 4 or np.ptp(e) == 0:
            continue
        m, c = np.polyfit(np.arange(len(e)), e, 1)
        st = stats[sec]
        st["n"] += 1
        st["rising" if m > 0 else "falling"] += 1
        xz = -c / m if m != 0 else -1
        if 0 <= xz <= len(e):
            st["crossing"] += 1
        third = max(1, len(e) // 3)
        if np.mean(e[:third]) < 0:
            st["early_below"] += 1
        if np.mean(e[-third:]) < 0:
            st["late_below"] += 1

    print(f"\n{pathlib.Path(path).name}  ({hz/1000:.0f} kframes/s, {len(wins)} windows)")
    print(f"{'sec':>3} {'n':>4} {'rising%':>8} {'falling%':>9} {'in-win ZC%':>11} "
          f"{'early<0%':>9} {'late<0%':>8}  convention")
    for s in (1, 2, 4, 5):
        st = stats[s]
        if not st["n"]:
            continue
        conv = "FALL, late below (VALUE=1)" if s % 2 == 0 else "RISE, late above (VALUE=0)"
        print(f"{s:>3} {st['n']:>4} {100*st['rising']/st['n']:>7.0f}% "
              f"{100*st['falling']/st['n']:>8.0f}% {100*st['crossing']/st['n']:>10.0f}% "
              f"{100*st['early_below']/st['n']:>8.0f}% {100*st['late_below']/st['n']:>7.0f}%  {conv}")


files = []
for pat in sys.argv[1:]:
    files += glob.glob(pat)
if not files:
    sys.exit("no files match")
for f in sorted(files):
    analyze(f)
