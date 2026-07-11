#!/usr/bin/env python3
"""Overlay per-rung supply voltage (and current) from lock-map meta
CSVs across runs — the direct observable for supply-limit theories.

Usage:
    python scripts/plot_vbat_overlay.py range60c range60d range60e
"""

import pathlib
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
tags = sys.argv[1:] or ["range60c", "range60d", "range60e"]

fig, (ax_v, ax_i) = plt.subplots(1, 2, figsize=(13, 4.5))
fig.suptitle("Supply under load across runs — bus voltage is the supply-limit tell", fontsize=12)

for tag in tags:
    p = capdir / f"{tag}_meta.csv"
    if not p.exists():
        continue
    amps, vb, ia = [], [], []
    for line in p.read_text().splitlines()[1:]:
        parts = line.split(",")  # 3-col (old) or 4-col (with vbat_min)
        a, v, i = parts[0], parts[1], parts[-1]
        if float(v) > 0 and float(i) > 0:  # skip dead/idle rows
            amps.append(int(a))
            vb.append(float(v))
            ia.append(float(i))
    ax_v.plot(amps, vb, marker="o", label=tag)
    ax_i.plot(amps, ia, marker="o", label=tag)

ax_v.set_ylabel("vbat under load (V)")
ax_v.set_xlabel("throttle (amp %)")
ax_v.grid(True, alpha=0.3)
ax_v.legend(fontsize=9)
ax_i.set_ylabel("supply current (A)")
ax_i.set_xlabel("throttle (amp %)")
ax_i.grid(True, alpha=0.3)
ax_i.legend(fontsize=9)

fig.tight_layout()
out = capdir / "vbat_overlay.png"
fig.savefig(out, dpi=110)
print(f"wrote {out}")
