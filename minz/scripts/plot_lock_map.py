#!/usr/bin/env python3
"""Render the closed-loop lock map from cl_lock_map.py captures.

Four panels vs throttle (amp %):
  1. speed: mean electrical frequency ± the window-to-window band
  2. lock coverage: qualified-ZC % (per-sector spread as error bars)
  3. timing tightness: σ of window length and of qZC position, as %
     of the window
  4. current: window-mean ± the in-window min/max envelope (mA)

Usage:
    python scripts/plot_lock_map.py --tag lockmap [-o out.png]
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
ap.add_argument("--tag", default="lockmap")
ap.add_argument("-o", "--out", default=None)
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
rows = []
for path in sorted(glob.glob(str(capdir / f"{args.tag}_a*.bin"))):
    m = re.search(rf"{re.escape(args.tag)}_a(\d+)\.bin$", path)
    if not m:
        continue
    amp = int(m.group(1))
    data = pathlib.Path(path).read_bytes()
    frames = parse_frames(data)
    if len(frames) < 100:
        continue
    lens = [f["len_us"] for f in frames if f["len_us"] > 0]
    fe = [1e6 / (6 * l) for l in lens]
    qzc_pct = 100 * sum(1 for f in frames if f["qzc_off_us"] != 0xFFFF) / len(frames)
    sec_q = []
    for s in range(6):
        fs = [f for f in frames if f["sector"] == s]
        if fs:
            sec_q.append(100 * sum(1 for f in fs if f["qzc_off_us"] != 0xFFFF) / len(fs))
    qz = [f["qzc_off_us"] for f in frames if f["qzc_off_us"] != 0xFFFF]
    mean_len = statistics.mean(lens)
    rows.append(
        dict(
            amp=amp,
            fe_mean=statistics.mean(fe),
            fe_sd=statistics.stdev(fe),
            qzc=qzc_pct,
            qzc_lo=min(sec_q) if sec_q else 0,
            qzc_hi=max(sec_q) if sec_q else 0,
            len_jit=100 * statistics.stdev(lens) / mean_len,
            zc_jit=100 * statistics.stdev(qz) / mean_len if len(qz) > 2 else float("nan"),
            i_avg=raw_to_ma(statistics.mean([f["i_avg"] for f in frames])),
            i_min=raw_to_ma(statistics.mean([f["i_min"] for f in frames])),
            i_max=raw_to_ma(statistics.mean([f["i_max"] for f in frames])),
            broke=b"DESYNC" in data or b"TRIP" in data,
            n=len(frames),
        )
    )

if not rows:
    raise SystemExit("no captures found")
rows.sort(key=lambda r: r["amp"])
A = [r["amp"] for r in rows]

fig, axes = plt.subplots(2, 2, figsize=(13, 8))
fig.suptitle(
    "FALCON closed-loop lock map — board 1, no caps, 6.5 V bench "
    f"({sum(r['n'] for r in rows)} windows total)",
    fontsize=12,
)

ax = axes[0][0]
ax.errorbar(A, [r["fe_mean"] for r in rows], yerr=[r["fe_sd"] for r in rows],
            marker="o", color="tab:blue", capsize=3)
for r in rows:
    if r["broke"]:
        ax.plot(r["amp"], r["fe_mean"], "x", color="red", ms=12, mew=2)
ax.set_title("speed vs throttle (± window-to-window σ)", fontsize=10)
ax.set_ylabel("f_e (Hz)")
ax.grid(True, alpha=0.3)

ax = axes[0][1]
ax.errorbar(
    A,
    [r["qzc"] for r in rows],
    yerr=[
        [r["qzc"] - r["qzc_lo"] for r in rows],
        [r["qzc_hi"] - r["qzc"] for r in rows],
    ],
    marker="s", color="tab:green", capsize=3,
)
ax.set_title("qualified-ZC coverage (bars = best/worst sector)", fontsize=10)
ax.set_ylabel("% of windows")
ax.set_ylim(0, 105)
ax.grid(True, alpha=0.3)

ax = axes[1][0]
ax.plot(A, [r["len_jit"] for r in rows], marker="o", label="window-length σ")
ax.plot(A, [r["zc_jit"] for r in rows], marker="^", label="qZC position σ")
ax.set_title("timing jitter (% of window)", fontsize=10)
ax.set_ylabel("% of window")
ax.set_xlabel("throttle (amp %)")
ax.legend(fontsize=8)
ax.grid(True, alpha=0.3)

ax = axes[1][1]
ax.plot(A, [r["i_avg"] for r in rows], marker="o", color="tab:purple", label="mean")
ax.fill_between(A, [r["i_min"] for r in rows], [r["i_max"] for r in rows],
                color="tab:purple", alpha=0.15, label="in-window min/max")
ax.set_title("current + supply voltage", fontsize=10)
ax.set_ylabel("mA")
ax.set_xlabel("throttle (amp %)")
ax.grid(True, alpha=0.3)
# Supply voltage from the per-point health checks (sidecar CSV) —
# a droop at high amp is the PSU current limit engaging, which
# masquerades as a motor voltage ceiling.
meta_path = capdir / f"{args.tag}_meta.csv"
ax2 = None
if meta_path.exists():
    vb, vmin = {}, {}
    for line in meta_path.read_text().splitlines()[1:]:
        parts = line.split(",")
        if len(parts) >= 3 and float(parts[1]) > 0:
            vb[int(parts[0])] = float(parts[1])
            if len(parts) >= 4 and float(parts[2]) > 0:
                vmin[int(parts[0])] = float(parts[2])
    pts = [(a, vb[a]) for a in A if a in vb]
    if pts:
        ax2 = ax.twinx()
        ax2.plot([p[0] for p in pts], [p[1] for p in pts], marker="s",
                 color="tab:red", label="vbat (V)")
        mpts = [(a, vmin[a]) for a in A if a in vmin]
        if mpts:
            # Worst vbat since the PREVIOUS rung — transits included.
            ax2.plot([p[0] for p in mpts], [p[1] for p in mpts], marker="v",
                     ls="--", color="tab:red", alpha=0.6, label="vbat min (V)")
        ax2.set_ylabel("vbat / min (V)", color="tab:red")
        ax2.tick_params(axis="y", labelcolor="tab:red")
# One combined legend — the vbat lines live on the twin axis and
# ax.legend() alone silently drops them.
h1, l1 = ax.get_legend_handles_labels()
h2, l2 = (ax2.get_legend_handles_labels() if ax2 else ([], []))
ax.legend(h1 + h2, l1 + l2, fontsize=8, loc="upper left")

fig.tight_layout()
out = args.out or str(capdir / f"{args.tag}_map.png")
fig.savefig(out, dpi=110)
print(f"wrote {out}")
