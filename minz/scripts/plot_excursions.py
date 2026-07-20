#!/usr/bin/env python3
"""Side-by-side rotor-period excursion distributions: AM32 vs minz.

Same metric on all datasets: each period (AM32: consecutive zt_us
deltas from the ZC trace; minz: MAGPIE window len_us) vs the 6-deep
rolling mean of its predecessors; excursion = period/ref - 1.
Positive tail histogram on log-y + the >12.5% / >25% rates.

Usage:
    python scripts/plot_excursions.py \
        --am32 captures/fatalband_trace.csv \
        --before captures/ladder_cap_t100flood_r0.bin \
        --after captures/ladder_cap_t100vpwm_r0.bin \
        --out captures/excursion_sidebyside.png
"""

import argparse
import csv
import pathlib
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from magpie import parse_frames

ap = argparse.ArgumentParser()
ap.add_argument("--am32", required=True)
ap.add_argument("--before", required=True)
ap.add_argument("--after", required=True)
ap.add_argument("--out", default="captures/excursion_sidebyside.png")
ap.add_argument("--band", default=None,
                help="restrict to periods LO-HI us (e.g. 80-180) so all "
                     "datasets compare the same speed regime")
args = ap.parse_args()
band = None
if args.band:
    lo, hi = args.band.split("-")
    band = (float(lo), float(hi))


def excursions(periods):
    """Positive excursion fractions vs the 6-deep rolling mean.
    With --band, only periods whose REFERENCE sits in-band count
    (the rolling context still uses every sample)."""
    out = []
    hist = []
    for p in periods:
        if len(hist) == 6:
            ref = sum(hist) / 6
            in_band = band is None or (band[0] <= ref <= band[1])
            if ref > 0 and p > ref and in_band:
                out.append(p / ref - 1.0)
        hist.append(p)
        if len(hist) > 6:
            hist.pop(0)
    return np.array(out)


def am32_periods(path):
    """zt_us IS the raw ZC-to-ZC period per record (the smoothed ci_us
    converges toward it at startup). Same running-regime filter as the
    minz side."""
    zts = []
    with open(path) as f:
        for row in csv.DictReader(f):
            try:
                # old=1 rows are polling-mode/startup — running
                # (interrupt-mode) records only, like the minz side.
                if row.get("old", "0").strip() == "0":
                    zts.append(float(row["zt_us"]))
            except (KeyError, ValueError):
                pass
    d = np.array(zts)
    return d[(d > 20) & (d < 2000)]


def minz_periods(path):
    frames = parse_frames(pathlib.Path(path).read_bytes())
    return np.array([f["len_us"] for f in frames if 20 < f["len_us"] < 2000])


sets = [
    ("AM32 (fatal-band trace)", am32_periods(args.am32), "tab:green"),
    ("minz BEFORE eeprom fixes", minz_periods(args.before), "tab:red"),
    ("minz AFTER (adv15 + varPWM)", minz_periods(args.after), "tab:blue"),
]

fig, ax = plt.subplots(figsize=(12, 6))
bins = np.linspace(0, 1.0, 101)
lines = []
for name, periods, color in sets:
    exc = excursions(periods)
    n = len(periods)
    r125 = 1e3 * (exc > 0.125).sum() / max(n, 1)
    r250 = 1e3 * (exc > 0.25).sum() / max(n, 1)
    worst = 100 * exc.max() if len(exc) else 0
    label = (f"{name}  |  n={n}  >12.5%: {r125:.2f}/1k  "
             f">25%: {r250:.2f}/1k  worst +{worst:.0f}%")
    ax.hist(exc, bins=bins, histtype="step", lw=1.8, color=color,
            label=label, weights=np.full(len(exc), 1e3 / max(n, 1)))
    lines.append(label)
    print(label)

ax.axvline(0.125, color="0.4", ls="--", lw=0.8)
ax.axvline(0.25, color="0.4", ls=":", lw=0.8)
ax.set_yscale("log")
ax.set_xlabel("positive period excursion vs 6-deep rolling mean (fraction)")
ax.set_ylabel("events per 1000 periods (log)")
ax.set_title("Rotor-period excursion tail — same metric, same motor, same bench")
ax.legend(fontsize=9, loc="upper right")
ax.grid(alpha=0.3)
fig.tight_layout()
fig.savefig(args.out, dpi=110)
print(f"wrote {args.out}")
