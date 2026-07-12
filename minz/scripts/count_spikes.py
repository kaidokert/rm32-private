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
ap.add_argument("--monster", type=float, default=4000.0,
                help="monster-event threshold (mA) for the tier split")
ap.add_argument("--bin-ms", type=float, default=100.0,
                help="time-bin width for the clustering histogram (ms)")
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

        # Tier split: small (--ma..--monster) vs monster (>--monster).
        small = [r for r in rows if r[2] < args.monster]
        mons = [r for r in rows if r[2] >= args.monster]
        print(f"   tiers: small {len(small)} ({len(small)/span_s:.2f}/s) | "
              f"MONSTER≥{args.monster/1000:.0f}A {len(mons)} "
              f"({len(mons)/span_s:.2f}/s)")

        # Time-binned clustering: bin event START times, report the
        # per-bin count distribution + Fano factor (var/mean of counts).
        # Fano ≈ 1 → Poisson/uniform (independent events); Fano ≫ 1 →
        # clustered/bursty (events arrive in groups). Also report the
        # per-second series so bursts are visible.
        def fano(times, width_s):
            nb = max(1, int(span_s / width_s) + 1)
            counts = [0] * nb
            t0 = rows[0][0] if rows else 0.0
            for t in times:
                counts[min(nb - 1, int((t - t0) / width_s))] += 1
            m = statistics.mean(counts)
            v = statistics.pvariance(counts) if len(counts) > 1 else 0.0
            return counts, m, v / m if m > 0 else 0.0

        ev_t = [r[0] for r in rows]
        bw = args.bin_ms / 1000.0
        c_bin, m_bin, f_bin = fano(ev_t, bw)
        c_sec, m_sec, f_sec = fano(ev_t, 1.0)
        occ = sum(1 for c in c_bin if c) / len(c_bin) * 100
        print(f"   per-{args.bin_ms:.0f}ms bins: mean {m_bin:.2f} ev/bin, "
              f"Fano {f_bin:.2f} ({'CLUSTERED' if f_bin > 1.5 else 'uniform-ish'}), "
              f"{occ:.0f}% of bins have ≥1 event")
        print(f"   per-1s: {c_sec}  Fano {f_sec:.2f}")
        # Monster-only clustering (the events that matter for chops).
        if len(mons) >= 3:
            _, mm, mf = fano([r[0] for r in mons], 1.0)
            print(f"   MONSTER per-1s Fano {mf:.2f} "
                  f"({'CLUSTERED' if mf > 1.5 else 'uniform-ish'})")
    if args.verbose:
        print("   t(s)    dur(ms)  peak(mA)  len-dev%%  order      noZC±hood")
        for t_s, dur_ms, peak, wd, tag, noz in rows:
            print(f"   {t_s:6.2f}  {dur_ms:7.1f}  {peak:8.0f}  {wd:+8.1f}"
                  f"  {tag:9s}  {noz}")
