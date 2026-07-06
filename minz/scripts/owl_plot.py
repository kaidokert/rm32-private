#!/usr/bin/env python3
"""OWL: plot shadow-lock performance from MAGPIE v3 captures.

One row per capture file, two panels:
  left  — ZC position within the window over time: grey = raw
          first-gate-surviving edge (noise-prone), colored by sector =
          persistence-QUALIFIED ZC. Dashed line = window midpoint.
  right — prediction error (actual boundary − predicted) over time,
          colored by sector, with the ±15 %-of-window FALCON gate band.

Usage:
    python scripts/owl_plot.py captures/owl_f100.bin captures/owl_f200.bin ...
"""

import argparse
import pathlib
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

from magpie import PRED_NONE, parse_frames

ap = argparse.ArgumentParser()
ap.add_argument("captures", nargs="+")
ap.add_argument("-o", "--out", default=None)
args = ap.parse_args()

rows = len(args.captures)
fig, axes = plt.subplots(rows, 2, figsize=(15, 4 * rows), squeeze=False)
colors = plt.cm.tab10.colors

for r, path in enumerate(args.captures):
    frames = parse_frames(pathlib.Path(path).read_bytes())
    if not frames:
        axes[r][0].set_title(f"{path}: no frames")
        continue
    t0 = frames[0]["start"]
    for f in frames:
        f["t"] = ((f["start"] - t0) & 0xFFFFFFFF) * 10e-6
    mean_len = statistics.mean(f["len_us"] for f in frames)
    f_est = 1e6 / (6 * mean_len)

    ax = axes[r][0]
    raw_t = [f["t"] for f in frames if f["zc_found"]]
    raw_z = [f["zc_off_us"] for f in frames if f["zc_found"]]
    ax.scatter(raw_t, raw_z, s=2, color="0.75", label="raw first-ZC (unqualified)")
    for s in range(6):
        fs = [f for f in frames if f["sector"] == s and f["qzc_off_us"] != 0xFFFF]
        ax.scatter(
            [f["t"] for f in fs],
            [f["qzc_off_us"] for f in fs],
            s=3,
            color=colors[s],
            label=f"sec {s}" if r == 0 else None,
        )
    ax.axhline(mean_len / 2, color="k", ls="--", lw=0.8, label="window midpoint")
    ax.axhline(mean_len, color="0.5", lw=0.8)
    qz = [f["qzc_off_us"] for f in frames if f["qzc_off_us"] != 0xFFFF]
    q_pct = 100 * len(qz) / len(frames)
    stats = f"qualified {q_pct:.0f}%"
    if len(qz) >= 2:
        stats += f", {statistics.mean(qz):.0f}±{statistics.stdev(qz):.0f}µs"
    ax.set_title(f"{pathlib.Path(path).name}  f≈{f_est:.0f} Hz — qZC position ({stats})", fontsize=10)
    ax.set_ylabel("µs from window start")
    ax.set_ylim(0, mean_len * 1.15)
    ax.grid(True, alpha=0.3)
    if r == 0:
        ax.legend(fontsize=7, ncol=4, loc="lower right")

    ax = axes[r][1]
    err_all = []
    for s in range(6):
        fs = [f for f in frames if f["sector"] == s and f["pred_err_us"] != PRED_NONE]
        err_all += [f["pred_err_us"] for f in fs]
        ax.scatter(
            [f["t"] for f in fs],
            [f["pred_err_us"] for f in fs],
            s=3,
            color=colors[s],
        )
    gate = 0.15 * mean_len
    ax.axhspan(-gate, gate, color="tab:green", alpha=0.08, label="FALCON gate ±15% window")
    ax.axhline(0, color="k", lw=0.8)
    stats = ""
    if len(err_all) >= 2:
        m, sd = statistics.mean(err_all), statistics.stdev(err_all)
        stats = f"err {m:+.0f}±{sd:.0f}µs = {100 * sd / mean_len:.1f}% jitter"
    ax.set_title(f"prediction error ({stats})", fontsize=10)
    ax.set_ylabel("µs")
    ax.set_ylim(-gate * 2, gate * 2)
    ax.grid(True, alpha=0.3)
    if r == 0:
        ax.legend(fontsize=7, loc="lower right")

for ax in axes[-1]:
    ax.set_xlabel("time (s)")

fig.suptitle("OWL shadow lock: qualified ZC + commutation prediction (steering nothing)", fontsize=12)
fig.tight_layout()
out = args.out or "captures/owl_plot.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
