#!/usr/bin/env python3
"""CONDOR: hz × amp heatmaps from a 2D sweep of MAGPIE captures.

Globs `captures/<tag>_f{hz}_a{amp}_*.bin` (produced by
`sweep_windows.py --amps ...`) and renders a rinz `zc_map`-style
4-panel figure:

  1. load-angle proxy: mean first-valid-ZC offset from window centre,
     in electrical degrees (window = 60°). Positive = ZC late.
  2. per-sector spread: std of the 6 per-sector mean offsets (deg) —
     structural asymmetry.
  3. zc-found %: windows with any gate-surviving edge.
  4. window current mean (mA).

Cells with too few windows are blanked. This is the MAGPIE-record
version (first-ZC based); a WAXWING/harmonic-fit upgrade can reuse
the same layout.

Usage:
    python scripts/plot_zc_map.py --tag condor [-o out.png]
"""

import argparse
import pathlib
import re
import statistics

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

from magpie import parse_frames, raw_to_ma

ap = argparse.ArgumentParser()
ap.add_argument("--tag", default="condor")
ap.add_argument("-o", "--out", default=None)
ap.add_argument("--min-windows", type=int, default=50)
args = ap.parse_args()

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
pat = re.compile(rf"{re.escape(args.tag)}_f(\d+)_a(\d+)_\w+\.bin$")

cells = {}
for path in capdir.glob(f"{args.tag}_f*_a*.bin"):
    m = pat.search(path.name)
    if not m:
        continue
    hz, amp = int(m.group(1)), int(m.group(2))
    frames = parse_frames(path.read_bytes())
    if len(frames) < args.min_windows:
        continue
    zc_deg = []  # per-window ZC offset from window centre, deg elec
    sector_means: dict[int, list[float]] = {s: [] for s in range(6)}
    for f in frames:
        if f["zc_found"] and f["len_us"] > 0:
            deg = (f["zc_off_us"] / f["len_us"] - 0.5) * 60.0
            zc_deg.append(deg)
            sector_means[f["sector"]].append(deg)
    found_pct = 100.0 * sum(1 for f in frames if f["zc_found"]) / len(frames)
    i_ma = raw_to_ma(statistics.mean([f["i_avg"] for f in frames]))
    per_sector = [statistics.mean(v) for v in sector_means.values() if len(v) >= 3]
    cells[(hz, amp)] = dict(
        angle=statistics.mean(zc_deg) if zc_deg else np.nan,
        spread=statistics.stdev(per_sector) if len(per_sector) >= 2 else np.nan,
        found=found_pct,
        i_ma=i_ma,
    )

if not cells:
    raise SystemExit(f"no parseable captures matching {args.tag}_f*_a*.bin")

hzs = sorted({hz for hz, _ in cells})
amps = sorted({amp for _, amp in cells})
grids = {k: np.full((len(amps), len(hzs)), np.nan) for k in ("angle", "spread", "found", "i_ma")}
for (hz, amp), c in cells.items():
    r, col = amps.index(amp), hzs.index(hz)
    for k in grids:
        grids[k][r][col] = c[k]

panels = [
    ("angle", "load-angle proxy (deg elec, first-ZC vs window centre)", "coolwarm", (-30, 30)),
    ("spread", "per-sector spread (deg) — structural asymmetry", "magma", None),
    ("found", "zc-found %", "viridis", (0, 100)),
    ("i_ma", "window current mean (mA)", "plasma", None),
]

fig, axes = plt.subplots(2, 2, figsize=(14, 9))
fig.suptitle(f"ZC map (MAGPIE first-ZC): {args.tag}", fontsize=13)
extent = None  # use categorical ticks — grids may be irregular

for ax, (key, title, cmap, clim) in zip(axes.flat, panels):
    im = ax.imshow(
        grids[key],
        origin="lower",
        aspect="auto",
        cmap=cmap,
        vmin=None if clim is None else clim[0],
        vmax=None if clim is None else clim[1],
    )
    ax.set_xticks(range(len(hzs)), [str(h) for h in hzs], fontsize=8)
    ax.set_yticks(range(len(amps)), [str(a) for a in amps], fontsize=8)
    ax.set_xlabel("hz")
    ax.set_ylabel("amp %")
    ax.set_title(title, fontsize=10)
    fig.colorbar(im, ax=ax, shrink=0.9)

fig.tight_layout()
out = args.out or str(capdir / f"{args.tag}_map.png")
fig.savefig(out, dpi=110)
print(f"wrote {out}  ({len(cells)} grid cells, hz={hzs}, amp={amps})")
