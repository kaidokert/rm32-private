#!/usr/bin/env python3
"""Count and characterize steady-state current-spike events in MAGPIE
captures — the primary debug metric for the residual high-amp events
(operator direction 2026-07-13: the thing to debug is the SPIKES, not
the eventual dropout; they visibly correlate with f_e irregularities).

An event = a contiguous run of windows with i_max above --ma (default
2000 mA). For each event we report duration, peak current, the local
f_e deviation around it, and lead/lag: whether the window-length
irregularity appears BEFORE the current rise (commutation mistiming
driving the spike) or AFTER it (spike/sag disturbing the rotor).

Usage:
    python scripts/count_spikes.py --tag fullsweep_pr2            # all bins
    python scripts/count_spikes.py --tag fullsweep_pr2 --amp 50 -v
"""

import argparse
import glob
import pathlib
import re
import statistics
import sys

from magpie import parse_frames, raw_to_ma

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--tag", required=True)
ap.add_argument("--amp", type=int, default=None, help="only this rung")
ap.add_argument("--ma", type=float, default=2000.0, help="event threshold (mA)")
ap.add_argument("--dev", type=float, default=3.0,
                help="f_e-irregularity threshold (%% window-len deviation)")
ap.add_argument("-v", "--verbose", action="store_true", help="per-event table")
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
bins = sorted(glob.glob(str(capdir / f"{args.tag}_a*.bin")))
if args.amp is not None:
    bins = [b for b in bins if re.search(rf"_a{args.amp}\.bin$", b)]
if not bins:
    sys.exit("no captures for tag/amp")

HOOD = 12  # windows of context on each side of an event


def rolling_median(vals, half=25):
    out = []
    for i in range(len(vals)):
        lo, hi = max(0, i - half), min(len(vals), i + half + 1)
        out.append(statistics.median(vals[lo:hi]))
    return out


for b in bins:
    frames = parse_frames(pathlib.Path(b).read_bytes())
    if len(frames) < 100:
        print(f"{pathlib.Path(b).name}: only {len(frames)} frames, skipped")
        continue
    span_s = (frames[-1]["start"] - frames[0]["start"]) * 10 / 1e6
    ima = [raw_to_ma(f["i_max"]) for f in frames]
    lens = [f["len_us"] for f in frames]
    base = rolling_median(lens)
    devp = [100.0 * (l - m) / m if m else 0.0 for l, m in zip(lens, base)]

    # Contiguous >threshold runs → events.
    events = []
    i = 0
    while i < len(frames):
        if ima[i] > args.ma:
            j = i
            while j + 1 < len(frames) and ima[j + 1] > args.ma:
                j += 1
            events.append((i, j))
            i = j + 1
        else:
            i += 1

    n_lead = n_lag = n_none = 0
    rows = []
    for (i0, j0) in events:
        t_s = (frames[i0]["start"] - frames[0]["start"]) * 10 / 1e6
        dur_ms = max(
            (frames[j0]["start"] - frames[i0]["start"]) * 10 / 1e3, 0.01)
        peak = max(ima[i0:j0 + 1])
        lo, hi = max(0, i0 - HOOD), min(len(frames), j0 + 1 + HOOD)
        worst_dev = max((devp[k] for k in range(lo, hi)), key=abs, default=0.0)
        # First irregular window in the neighborhood vs event start.
        irr = next((k for k in range(lo, hi) if abs(devp[k]) >= args.dev), None)
        if irr is None:
            tag, n_none = "none", n_none + 1
        elif irr < i0:
            tag, n_lead = "len-LEADS", n_lead + 1
        else:
            tag, n_lag = "len-lags", n_lag + 1
        noz = sum(1 for k in range(lo, hi)
                  if frames[k]["qzc_off_us"] == 0xFFFF)
        rows.append((t_s, dur_ms, peak, worst_dev, tag, noz))

    name = pathlib.Path(b).name
    print(f"\n== {name}: {len(events)} events>{args.ma:.0f}mA in "
          f"{span_s:.1f}s = {len(events) / span_s:.1f}/s | "
          f"len-irregularity {args.dev:.0f}%: leads {n_lead}, "
          f"lags {n_lag}, none {n_none}")
    if rows:
        peaks = [r[2] for r in rows]
        durs = [r[1] for r in rows]
        print(f"   peaks mA: max {max(peaks):.0f} median "
              f"{statistics.median(peaks):.0f} | dur ms: max {max(durs):.1f} "
              f"median {statistics.median(durs):.1f}")
    if args.verbose:
        print("   t(s)    dur(ms)  peak(mA)  len-dev%%  order      noZC±hood")
        for t_s, dur_ms, peak, wd, tag, noz in rows:
            print(f"   {t_s:6.2f}  {dur_ms:7.1f}  {peak:8.0f}  {wd:+8.1f}"
                  f"  {tag:9s}  {noz}")
