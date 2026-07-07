#!/usr/bin/env python3
"""Render the commutation-advance sweep (cl_adv_sweep.py captures).

Three panels vs advance (deg), one line per amp: speed, current,
qualified-ZC coverage. Points from degraded engagements (qzc < 60 %
at adv=0 reference) are drawn hollow.

Usage:
    python scripts/plot_adv_map.py --tag advmap
"""

import argparse
import glob
import pathlib
import re
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

from magpie import parse_frames, raw_to_ma

ap = argparse.ArgumentParser()
ap.add_argument("--tag", default="advmap")
ap.add_argument("-o", "--out", default=None)
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
pts = {}
for path in sorted(glob.glob(str(capdir / f"{args.tag}_a*_adv*.bin"))):
    m = re.search(rf"{re.escape(args.tag)}_a(\d+)_adv([mp])(\d+)\.bin$", path)
    if not m:
        continue
    amp = int(m.group(1))
    adv = int(m.group(3)) * (-1 if m.group(2) == "m" else 1)
    frames = parse_frames(pathlib.Path(path).read_bytes())
    if len(frames) < 100:
        continue
    lens = [f["len_us"] for f in frames if f["len_us"] > 0]
    q = 100 * sum(1 for f in frames if f["qzc_off_us"] != 0xFFFF) / len(frames)
    pts.setdefault(amp, {})[adv] = dict(
        fe=1e6 / (6 * statistics.mean(lens)),
        i=raw_to_ma(statistics.mean([f["i_avg"] for f in frames])),
        qzc=q,
    )

if not pts:
    raise SystemExit("no captures")

fig, axes = plt.subplots(1, 3, figsize=(15, 4.5))
fig.suptitle("FALCON: commutation advance under lock (7.5 V, board 1, no caps)", fontsize=12)
colors = plt.cm.viridis([i / max(1, len(pts) - 1) for i in range(len(pts))])

for (amp, series), color in zip(sorted(pts.items()), colors):
    advs = sorted(series)
    healthy = series.get(0, {}).get("qzc", 0) >= 60
    style = dict(color=color, marker="o" if healthy else "x",
                 ls="-" if healthy else ":",
                 label=f"amp {amp}" + ("" if healthy else " (degraded engage)"))
    axes[0].plot(advs, [series[a]["fe"] for a in advs], **style)
    axes[1].plot(advs, [series[a]["i"] for a in advs], **style)
    axes[2].plot(advs, [series[a]["qzc"] for a in advs], **style)

for ax, (title, ylab) in zip(
    axes,
    [("speed", "f_e (Hz)"), ("current", "mA"), ("lock coverage", "qzc %")],
):
    ax.set_title(title, fontsize=10)
    ax.set_ylabel(ylab)
    ax.set_xlabel("advance (deg)")
    ax.grid(True, alpha=0.3)
axes[0].legend(fontsize=8)

fig.tight_layout()
out = args.out or str(capdir / f"{args.tag}_map.png")
fig.savefig(out, dpi=110)
print(f"wrote {out}")
