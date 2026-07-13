#!/usr/bin/env python3
"""Onset probe: parse a MAGPIE .bin capture and print the per-window
timeline around the current ramp, to establish cause-order — does the
commutation TIMING fail (interval jump / ZC miss) BEFORE the CURRENT
ramps, or does the current lead?

Usage:
    python scripts/onset_probe.py captures/sweep55_t48.bin [--ma 2000] [--pre 30]
"""

import argparse
import pathlib

import magpie

ap = argparse.ArgumentParser()
ap.add_argument("binfile")
ap.add_argument("--ma", type=float, default=2000, help="current threshold marking the ramp onset")
ap.add_argument("--pre", type=int, default=30, help="windows to show before onset")
ap.add_argument("--post", type=int, default=40, help="windows to show after onset")
args = ap.parse_args()

buf = pathlib.Path(args.binfile).read_bytes()
frames = magpie.parse_frames(buf)
print(f"{len(frames)} windows parsed from {args.binfile}")

# Find the onset: first window whose i_max exceeds the threshold.
onset = None
for i, f in enumerate(frames):
    if magpie.raw_to_ma(f["i_max"]) >= args.ma:
        onset = i
        break

if onset is None:
    print(f"no window with i_max >= {args.ma} mA")
    # still dump the tail
    onset = max(0, len(frames) - args.post)

lo = max(0, onset - args.pre)
hi = min(len(frames), onset + args.post)
print(f"onset (i_max >= {args.ma:.0f} mA) at window {onset}; showing {lo}..{hi}\n")
print(f"{'idx':>4} {'seq':>3} {'sec':>3} {'zc':>2} {'int_us':>6} {'qzc_off':>7} "
      f"{'i_avg':>6} {'i_max':>6} {'vbat':>5}")
print("-" * 56)
prev_int = None
for i in range(lo, hi):
    f = frames[i]
    zc = "Y" if f["zc_found"] else "."
    qzc = f["qzc_off_us"]
    qzc_s = "----" if qzc == 0xFFFF else str(qzc)
    imax = magpie.raw_to_ma(f["i_max"])
    iavg = magpie.raw_to_ma(f["i_avg"])
    vb = magpie.vbat_raw_to_v(f["vbat_raw"]) if f["vbat_raw"] is not None else 0.0
    intv = f["len_us"]
    # flag a big interval jump (commutation timing change)
    jump = ""
    if prev_int and intv > 0 and abs(intv - prev_int) > 0.25 * prev_int:
        jump = f"  <<INT {intv - prev_int:+d}us"
    prev_int = intv if intv > 0 else prev_int
    mark = "  <== onset" if i == onset else ""
    print(f"{i:>4} {f['seq']:>3} {f['sector']:>3} {zc:>2} {intv:>6} {qzc_s:>7} "
          f"{iavg:>6.0f} {imax:>6.0f} {vb:>5.2f}{jump}{mark}")
