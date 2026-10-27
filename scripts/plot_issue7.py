#!/usr/bin/env python3
"""Issue #7 figures (L431 vs binz on one commit) from the FC-bench
captures. Reads only what the bench wrote; refuses to plot a figure
whose inputs are missing.

Inputs (captures/gc/):
  issue7/curve/<board>_curve_<n>.csv   bf_curve.py (BF eRPM + EDT per rung)
  issue7/<board>_<ladder|slam>_<n>.log bf_ladder.py / bf_slam.py ESC logs
  carrier/<variant>_<n>.log            25 % BF holds for the start A/B

Usage: plot_issue7.py [--out docs/issue7]
"""
import argparse
import csv
import re
import sys
from collections import defaultdict
from pathlib import Path
from statistics import mean

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

sys.path.insert(0, str(Path(__file__).parent))
from fc_bench import parse_run  # noqa: E402

CAP = Path("captures/gc")
COLOR = {"l431": "#2f6db3", "binz": "#b5591d"}
LABEL = {"l431": "L431", "binz": "binz (G071)"}
INK, MUTED, GRID = "#1d2420", "#5d6862", "#e3e7e4"


def style(ax, xlabel, ylabel):
    ax.set_xlabel(xlabel, color=MUTED)
    ax.set_ylabel(ylabel, color=MUTED)
    ax.grid(True, color=GRID, linewidth=0.8)
    ax.set_axisbelow(True)
    for s in ("top", "right"):
        ax.spines[s].set_visible(False)
    for s in ("left", "bottom"):
        ax.spines[s].set_color(MUTED)
    ax.tick_params(colors=MUTED)


def save(fig, out, name):
    fig.tight_layout()
    path = out / name
    fig.savefig(path, dpi=140, facecolor="white")
    plt.close(fig)
    print(f"wrote {path}")


def load_curves():
    data = defaultdict(list)
    for f in sorted((CAP / "issue7/curve").glob("*_curve_*.csv")):
        board = f.name.split("_")[0]
        run = int(f.stem.rsplit("_", 1)[1])
        with open(f, encoding="ascii") as fh:
            for r in csv.DictReader(fh):
                data[board].append({**r, "run": run, "pct": int(r["pct"]),
                                    "erpm": int(r["erpm"]), "t_s": float(r["t_s"]),
                                    "curr_a": int(r["curr_a"]), "vbat_v": float(r["vbat_v"]),
                                    "temp_c": int(r["temp_c"]),
                                    "invalid_pct": float(r["invalid_pct"])})
    if set(data) != {"l431", "binz"}:
        sys.exit(f"curve CSVs missing a board: {sorted(data)}")
    return data


def per_rung(rows, key, direction, last_only=False):
    """Mean of `key` per (pct) over runs; the first rung's start transient
    is dropped by using each rung's LAST sample when last_only."""
    groups = defaultdict(list)
    by_run_rung = defaultdict(list)
    for r in rows:
        if r["dir"] == direction:
            by_run_rung[(r["run"], r["pct"])].append(r)
    for (run, pct), rs in by_run_rung.items():
        vals = [rs[-1][key]] if last_only else [x[key] for x in rs]
        groups[pct].extend(vals)
    xs = sorted(groups)
    return xs, [mean(groups[x]) for x in xs], [min(groups[x]) for x in xs], [max(groups[x]) for x in xs]


def fig_speed(curves, out):
    fig, ax = plt.subplots(figsize=(7.2, 4.4))
    for b in ("l431", "binz"):
        for d, ls, mk in (("up", "-", "o"), ("down", "--", "s")):
            xs, ys, _, _ = per_rung(curves[b], "erpm", d, last_only=True)
            if d == "up":  # 10 % up is the start rung: still spinning up
                xs, ys = xs[1:], ys[1:]
            ax.plot(xs, [y / 1000 for y in ys], ls, color=COLOR[b], linewidth=2,
                    marker=mk, markersize=4.5, label=f"{LABEL[b]} {d}")
    style(ax, "Commanded throttle (%)", "Speed, BF bidir eRPM (thousands)")
    ax.set_title("Speed vs throttle, 10 → 100 → 10 % (2 runs per board)", color=INK, loc="left")
    ax.text(0.99, 0.02, "up sweep from 15 %: the 10 % start rung is a spin-up transient",
            transform=ax.transAxes, ha="right", va="bottom", color=MUTED, fontsize=8)
    ax.legend(frameon=False, ncol=2)
    save(fig, out, "speed_vs_throttle.png")


def fig_gap(curves, out):
    fig, ax = plt.subplots(figsize=(7.2, 3.4))
    for d, ls, mk in (("up", "-", "o"), ("down", "--", "s")):
        xl, yl, _, _ = per_rung(curves["l431"], "erpm", d, last_only=True)
        xb, yb, _, _ = per_rung(curves["binz"], "erpm", d, last_only=True)
        common = [x for x in xl if x in xb and x >= 15]
        gap = [100 * (yb[xb.index(x)] / yl[xl.index(x)] - 1) for x in common]
        ax.plot(common, gap, ls, color=INK, linewidth=2, marker=mk, markersize=4.5, label=d)
    ax.axhline(0, color=MUTED, linewidth=1)
    style(ax, "Commanded throttle (%)", "binz faster by (%)")
    ax.set_title("Speed gap, binz vs L431 at the same command", color=INK, loc="left")
    ax.legend(frameon=False)
    save(fig, out, "speed_gap.png")


def fig_power(curves, out):
    fig, axes = plt.subplots(1, 2, figsize=(9.6, 3.6))
    for b in ("l431", "binz"):
        for ax, key, ttl in ((axes[0], "curr_a", "Current, EDT (1 A per step)"),
                             (axes[1], "vbat_v", "Supply at the ESC, EDT (0.25 V per step)")):
            xs, ys, lo, hi = per_rung(curves[b], key, "up")
            ax.fill_between(xs, lo, hi, color=COLOR[b], alpha=0.15, linewidth=0)
            ax.plot(xs, ys, "-", color=COLOR[b], linewidth=2, marker="o", markersize=4, label=LABEL[b])
            ax.set_title(ttl, color=INK, loc="left", fontsize=10)
    style(axes[0], "Commanded throttle (%)", "A")
    style(axes[1], "Commanded throttle (%)", "V")
    axes[0].legend(frameon=False)
    save(fig, out, "current_and_supply.png")


def fig_temp(curves, out):
    fig, ax = plt.subplots(figsize=(7.2, 3.2))
    for b in ("l431", "binz"):
        for run in sorted({r["run"] for r in curves[b]}):
            rs = [r for r in curves[b] if r["run"] == run]
            ax.plot([r["t_s"] for r in rs], [r["temp_c"] for r in rs], "-",
                    color=COLOR[b], linewidth=2, alpha=1.0 if run == 1 else 0.5,
                    label=f"{LABEL[b]} run {run}")
    style(ax, "Time into the curve (s)", "ESC temperature, EDT (°C)")
    ax.set_title("ESC temperature through the curve", color=INK, loc="left")
    ax.legend(frameon=False, ncol=2)
    save(fig, out, "temperature.png")


def fig_invalid(curves, out):
    fig, ax = plt.subplots(figsize=(7.2, 3.2))
    for b in ("l431", "binz"):
        xs, ys, lo, hi = per_rung(curves[b], "invalid_pct", "up")
        ax.plot(xs, ys, "-", color=COLOR[b], linewidth=2, marker="o", markersize=4, label=LABEL[b])
    style(ax, "Commanded throttle (%)", "Invalid telemetry frames (%)")
    ax.set_title("BF bidir telemetry errors (BF's rolling invalid %)", color=INK, loc="left")
    ax.legend(frameon=False)
    save(fig, out, "telemetry_invalid.png")


NAME_RE = re.compile(r"^(l431|binz)_(ladder|slam)_(\d+)\.log$")


def fig_runs(out):
    pts = defaultdict(list)
    for f in sorted((CAP / "issue7").glob("*.log")):
        m = NAME_RE.match(f.name)
        if not m:
            continue
        runs = [r for r in map(parse_run, f.read_text("ascii", "replace").splitlines()) if r]
        if not runs:
            sys.exit(f"no [run] in {f}")
        pts[(m.group(1), m.group(2))].append(runs[-1])
    if len(pts) != 4:
        sys.exit(f"issue7 logs incomplete: {sorted(pts)}")
    fig, axes = plt.subplots(1, 2, figsize=(9.6, 3.4))
    for ax, test in zip(axes, ("ladder", "slam")):
        for i, b in enumerate(("l431", "binz")):
            rs = pts[(b, test)]
            jit = [i + 0.07 * (k - (len(rs) - 1) / 2) for k in range(len(rs))]
            ax.scatter(jit, [r["erpm_pk"] / 1000 for r in rs], s=64,
                       color=COLOR[b], edgecolor="white", linewidth=2, zorder=3)
            ax.annotate(f"{len(rs)}/{len(rs)} pass\n{mean(r['comms'] for r in rs) / 1e3:,.0f}k comms",
                        (i, mean(r["erpm_pk"] for r in rs) / 1000), xytext=(14, -6),
                        textcoords="offset points", color=INK, fontsize=8.5)
        ax.set_xticks([0, 1], [LABEL["l431"], LABEL["binz"]])
        ax.set_xlim(-0.5, 1.9)
        style(ax, "", "Peak eRPM (thousands)")
        ax.set_title(f"bf_{test}.py, per run", color=INK, loc="left", fontsize=10)
    save(fig, out, "ladder_slam_runs.png")


# Start A/B variants: (label, glob prefixes). Clean = the hold's [run]
# shows exactly one start.
VARIANTS = [
    ("rm32 before fix\n(48k/24k, fwd, sine, snap)", ("k48_", "k24_", "fwd_", "sine_", "snap_", "hist_", "hist2_")),
    ("guard: skip 1 check", ("xg_",)),
    ("guard: skip 2 checks\n(f0625ca)", ("xg2_",)),
]


def fig_starts(out):
    rows = []
    for label, prefixes in VARIANTS:
        clean = total = 0
        for f in sorted((CAP / "carrier").glob("*.log")):
            if not f.name.startswith(prefixes):
                continue
            runs = [r for r in map(parse_run, f.read_text("ascii", "replace").splitlines()) if r]
            if not runs:
                continue
            total += 1
            clean += runs[-1]["starts"] == 1
        if total == 0:
            sys.exit(f"no logs for {label!r}")
        rows.append((label, clean, total))
    rows.insert(0, ("stock AM32 fd4d45f\n(oracle, Call 45)", 8, 8))
    fig, ax = plt.subplots(figsize=(7.2, 3.4))
    ys = list(range(len(rows)))[::-1]
    for y, (label, clean, total) in zip(ys, rows):
        ax.barh(y, 100 * clean / total, height=0.55, color="#2e7d4f" if clean == total else "#a86b00")
        ax.text(100 * clean / total + 1.5, y, f"{clean}/{total}", va="center", color=INK, fontsize=9)
    ax.set_yticks(ys, [r[0] for r in rows], fontsize=8.5)
    ax.set_xlim(0, 112)
    style(ax, "Clean 25 % starts (%)", "")
    ax.set_title("L431 start A/B: one start, no restart loop", color=INK, loc="left")
    save(fig, out, "start_ab.png")
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="docs/issue7")
    a = ap.parse_args()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    curves = load_curves()
    fig_speed(curves, out)
    fig_gap(curves, out)
    fig_power(curves, out)
    fig_temp(curves, out)
    fig_invalid(curves, out)
    fig_runs(out)
    for label, clean, total in fig_starts(out):
        print(f"starts: {label.splitlines()[0]}: {clean}/{total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
