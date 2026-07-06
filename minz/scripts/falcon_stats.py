#!/usr/bin/env python3
"""FALCON discriminator statistics from WAXWING probe bursts.

For every float window with an analog channel (phase A: sectors 2/5,
phase B: 1/4) compute the ground-truth ZC (least-squares crossing of
phase − neutral) and replay candidate discriminators against it:

  comp-rule : the v2 firmware rule — wrap-sampled COMP bit must equal
              the expected post-ZC level for 2 consecutive PWM cycles.
  adc-rule  : same 2-consecutive persistence, but on the SIGN of the
              mid-ON ADC sample (phase − neutral) — the "virtual
              comparator" FALCON could use instead.

Reported per capture group: window count, ground-truth coverage, and
for each rule: accept rate, premature-accept rate (accept ≥2 frames
BEFORE the analog ZC — the self-lock trap), and accept latency
mean±sd in frames after the analog ZC.

Usage:
    python scripts/falcon_stats.py captures/probe_*.txt
"""

import argparse
import glob
import pathlib
import re
import statistics
import sys

import numpy as np

from magpie import parse_cdump

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("patterns", nargs="+")
ap.add_argument("--blank-frames", type=int, default=2)
args = ap.parse_args()

files = []
for pat in args.patterns:
    files += glob.glob(pat)
if not files:
    sys.exit("no files match")

PHASE_WINDOWS = {"a": (2, 5), "b": (1, 4)}


def analyze(path):
    frames, _hz = parse_cdump(pathlib.Path(path).read_text())
    n = len(frames)
    A = np.array([f["a"] for f in frames], float)
    B = np.array([f["b"] for f in frames], float)
    sector = np.array([f["sector"] for f in frames], int)
    comp = np.array([f["comp"] for f in frames], int)
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

    # window segmentation; drop absurdly long tails (post-kill coast)
    wins = []
    w0 = 0
    for i in range(1, n):
        if sector[i] != sector[i - 1]:
            wins.append((w0, i, sector[i - 1]))
            w0 = i
    lens = [b - a for a, b, _ in wins] or [1]
    med = statistics.median(lens)
    wins = [w for w in wins if w[1] - w[0] < 3 * med]

    out = []  # per-window dicts
    for s0, s1, sec in wins:
        trace = A if sec in PHASE_WINDOWS["a"] else B if sec in PHASE_WINDOWS["b"] else None
        if trace is None or s1 - s0 < args.blank_frames + 4:
            continue
        lo = s0 + args.blank_frames
        e = trace[lo:s1] - neutral[lo:s1]
        x = np.arange(len(e))
        if np.ptp(e) == 0:
            continue
        m, c = np.polyfit(x, e, 1)
        if m == 0:
            continue
        xz = -c / m
        if not (0 <= xz <= len(e)):
            continue  # no in-window analog crossing
        zc = lo + xz

        expected = 1 if sec % 2 == 0 else 0
        row = dict(sector=sec, zc=zc, win=(s0, s1))
        for rule, sig in (
            ("comp", comp[s0:s1]),
            ("adc", (np.where(trace[s0:s1] - neutral[s0:s1] < 0, 1, 0) if True else None)),
        ):
            # post-ZC signal level convention: comp==1 means phase
            # below neutral (POLARITY=0, INM=phase). adc sign mapped
            # identically so `expected` applies to both.
            acc = None
            for k in range(args.blank_frames, len(sig) - 1):
                if sig[k] == expected and sig[k + 1] == expected:
                    acc = s0 + k + 1  # accepted at 2nd confirm
                    break
            row[rule] = acc
        out.append(row)
    return out


groups = {}
for f in sorted(files):
    key = re.sub(r"_\d+\.txt$", "", pathlib.Path(f).name)
    groups.setdefault(key, []).extend(analyze(f))

print(f"{'group':28} {'wins':>5} | {'rule':4} {'acc%':>5} {'prem%':>6} {'lat(frames)':>12}")
for key, rows in groups.items():
    if not rows:
        print(f"{key:28} {'0':>5} | no analyzable windows")
        continue
    for rule in ("comp", "adc"):
        acc = [r for r in rows if r[rule] is not None]
        prem = [r for r in acc if r[rule] < r["zc"] - 2]
        lat = [r[rule] - r["zc"] for r in acc if r[rule] >= r["zc"] - 2]
        line = f"{key:28} {len(rows):>5} | {rule:4} {100 * len(acc) / len(rows):>4.0f}%"
        line += f" {100 * len(prem) / len(rows):>5.0f}%"
        if len(lat) >= 2:
            line += f" {statistics.mean(lat):>+6.1f}±{statistics.stdev(lat):<4.1f}"
        print(line)
