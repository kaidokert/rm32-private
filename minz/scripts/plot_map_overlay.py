#!/usr/bin/env python3
"""Overlaid operating map: throttle vs electrical Hz / amps / volts,
am32_clone vs AM32, from two map_sweep.py CSVs.

Usage:
    python scripts/plot_map_overlay.py \
        --clone captures/map_clone_map.csv \
        --am32 captures/map_am32_map.csv \
        --out captures/map_overlay.png
"""

import argparse
import csv

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

ap = argparse.ArgumentParser()
ap.add_argument("--clone", required=True)
ap.add_argument("--am32", required=True)
ap.add_argument("--out", default="captures/map_overlay.png")
args = ap.parse_args()


def load(path):
    rows = []
    with open(path, newline="") as fh:
        for r in csv.DictReader(fh):
            rows.append((int(r["rung"]), float(r["f_e_hz"]),
                         float(r["ma"]), float(r["mv"])))
    rows.sort()
    return rows


cl = load(args.clone)
am = load(args.am32)

fig, axes = plt.subplots(3, 1, figsize=(9, 11), sharex=True)
panels = (("electrical Hz", 1), ("current mA", 2), ("bus mV", 3))
for ax, (title, idx) in zip(axes, panels):
    ax.plot([r[0] for r in cl], [r[idx] for r in cl],
            "o-", label="am32_clone", color="tab:blue")
    ax.plot([r[0] for r in am], [r[idx] for r in am],
            "s--", label="AM32", color="tab:orange")
    ax.set_ylabel(title)
    ax.grid(True, alpha=0.3)
    ax.legend()
axes[0].set_title("Operating map: am32_clone vs AM32 (same bench, "
                  "back-to-back)")
axes[2].set_xlabel("throttle %")
fig.tight_layout()
fig.savefig(args.out, dpi=130)
print(f"wrote {args.out}")

# Console summary table
print(f"\n{'pct':>4} | {'clone Hz':>8} {'am32 Hz':>8} | "
      f"{'clone mA':>8} {'am32 mA':>8} | {'clone mV':>8} {'am32 mV':>8}")
amd = {r[0]: r for r in am}
for r in cl:
    a = amd.get(r[0])
    if a:
        print(f"{r[0]:>4} | {r[1]:>8.0f} {a[1]:>8.0f} | "
              f"{r[2]:>8.0f} {a[2]:>8.0f} | {r[3]:>8.0f} {a[3]:>8.0f}")
