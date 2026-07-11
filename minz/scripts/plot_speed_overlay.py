#!/usr/bin/env python3
"""Overlay speed-vs-throttle across lock-map runs (e.g. two bench
voltages) plus the per-rung ratio — the direct test of "the ceiling
was supply voltage": below the power knee every rung should scale by
exactly V2/V1.

Usage:
    python scripts/plot_speed_overlay.py range60e:7.45V range60f:8.05V
"""

import glob
import pathlib
import re
import statistics
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

from magpie import parse_frames

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
specs = sys.argv[1:] or ["range60e:7.45V", "range60f:8.05V"]

runs = []
for spec in specs:
    tag, _, label = spec.partition(":")
    pts = {}
    for path in sorted(glob.glob(str(capdir / f"{tag}_a*.bin"))):
        m = re.search(rf"{re.escape(tag)}_a(\d+)\.bin$", path)
        if not m:
            continue
        frames = parse_frames(pathlib.Path(path).read_bytes())
        lens = [f["len_us"] for f in frames if f["len_us"] > 0]
        if len(lens) >= 100:
            pts[int(m.group(1))] = 1e6 / (6 * statistics.mean(lens))
    runs.append((tag, label or tag, pts))

fig, (ax_s, ax_r) = plt.subplots(1, 2, figsize=(13, 4.5))
fig.suptitle("Speed vs throttle across bench voltages — sub-knee rungs must scale by V2/V1", fontsize=11)

for tag, label, pts in runs:
    amps = sorted(pts)
    ax_s.plot(amps, [pts[a] for a in amps], marker="o", label=f"{tag} ({label})")
ax_s.set_ylabel("f_e (Hz)")
ax_s.set_xlabel("throttle (amp %)")
ax_s.grid(True, alpha=0.3)
ax_s.legend(fontsize=9)

if len(runs) == 2:
    (_, l1, p1), (_, l2, p2) = runs
    common = sorted(set(p1) & set(p2))
    ratios = [p2[a] / p1[a] for a in common]
    ax_r.plot(common, ratios, marker="s", color="tab:green")
    ax_r.axhline(1.0, color="grey", lw=0.8)
    ax_r.set_ylabel(f"speed ratio  {l2} / {l1}")
    ax_r.set_xlabel("throttle (amp %)")
    ax_r.grid(True, alpha=0.3)

fig.tight_layout()
out = capdir / "speed_overlay.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
