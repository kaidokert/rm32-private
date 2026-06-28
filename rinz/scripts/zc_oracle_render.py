#!/usr/bin/env python3
"""Render the HONEST BEMF zero-crossing detector for review.

Supersedes the whole-window LS-linfit overlay (zc_<hz>_<amp>.png from cl_wave_sweep),
which fabricated crossings by fitting demag/commutation transients in out-of-window
sectors (see BEMF_ZC_DETECTOR.md §4 retraction). This shows, per phase, what is actually
measurable:

  * the pooled float-arc BEMF samples folded into one electrical period (angle 0-360),
  * the validated harmonic-oracle sinusoid fit (a*cos+b*sin+c, transients blanked),
  * its TRUE zero crossing (where the sinusoid passes through neutral) -- drawn whether it
    lands inside or OUTSIDE the 60-degree float window,
  * the two float windows for that phase, shaded -- so "crossing outside the window" is
    visible at a glance,
  * any directly-OBSERVED in-window sign-change crossing (the only ground-truth marker).

Usage:
  python scripts/zc_oracle_render.py logs/zc_20260627_164752            # whole dir
  python scripts/zc_oracle_render.py logs/zc_20260627_164752/zc_250_12.log
  python scripts/zc_oracle_render.py <dir> --out logs/zc_oracle_review
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

from scope_common import (PHASE_NAMES, PHASE_TO_CHANNEL, SIX_STEP_HIGH, SIX_STEP_LOW,  # noqa: E402
                          analyze_zero_crossings, lowpass_channels, parse_capture,
                          parse_cl_bounds)

TWO_PI = 2.0 * math.pi
# Float phase (0=A,1=B,2=C) of each physical sector, and thus the two 60-deg angular
# windows in which each phase floats (forward drive).
SECTOR_FLOAT = [3 - SIX_STEP_HIGH[k] - SIX_STEP_LOW[k] for k in range(6)]
PHASE_WINDOWS = {p: [k for k in range(6) if SECTOR_FLOAT[k] == p] for p in range(3)}


def fit_sinusoid(angles, vals):
    """Least-squares a*cos+b*sin+c; returns (a, b, c, rms)."""
    t = np.asarray(angles)
    y = np.asarray(vals)
    A = np.column_stack([np.cos(t), np.sin(t), np.ones_like(t)])
    coef, *_ = np.linalg.lstsq(A, y, rcond=None)
    a, b, c = coef
    rms = float(np.sqrt(np.mean((y - A @ coef) ** 2))) if len(y) else float("nan")
    return float(a), float(b), float(c), rms


def zero_crossings(a, b, c):
    """Angles in [0,2pi) where a*cos t + b*sin t + c = 0, with direction."""
    r = math.hypot(a, b)
    if r < 1e-9 or abs(c) > r:
        return []
    psi = math.atan2(a, b)
    base = math.asin(-c / r)
    out = []
    for shift in (base, math.pi - base):
        t = (shift - psi) % TWO_PI
        deriv = -a * math.sin(t) + b * math.cos(t)
        out.append((t, "rise" if deriv > 0 else "fall"))
    return sorted(out)


def collect_phase(cap, smooth_window=5, blank_frames=3):
    """phase -> (angles[rad], v_minus_neutral) over the frames where it floats."""
    hz = float(cap.debug.get("hz", "0") or 0)
    sm = lowpass_channels(cap.channels, smooth_window)
    frames = min(len(c) for c in sm)
    neutral = [(sm[0][i] + sm[1][i] + sm[2][i]) / 3.0 for i in range(frames)]
    fpr = cap.sample_hz / hz       # frames per electrical rev
    fps = fpr / 6.0
    out = {0: ([], []), 1: ([], []), 2: ([], [])}
    for i in range(frames):
        k = int(i / fps)
        if k < 6:                  # skip rev 0 (startup transient)
            continue
        if (i - k * fps) < blank_frames:   # demag blank after each sector start
            continue
        fl = SECTOR_FLOAT[k % 6]
        theta = (i / fpr) * TWO_PI
        out[fl][0].append(theta % TWO_PI)
        out[fl][1].append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])
    return out, hz


def render(cap, out_png):
    samples, hz = collect_phase(cap)
    bounds, _ = parse_cl_bounds(cap.text)
    secs, _, _ = analyze_zero_crossings(cap, smooth_window=3, sector_bounds=bounds)
    # Observed in-window crossings: phase -> list of (sector, pct, dir)
    obs = {0: [], 1: [], 2: []}
    for s in secs:
        if s.status == "zc" and s.zc_pct is not None:
            obs[PHASE_NAMES.index(s.phase)].append((s.index % 6, s.zc_pct, s.direction))

    fig, axes = plt.subplots(3, 1, figsize=(13, 9), sharex=True)
    colors = {0: "tab:green", 1: "tab:blue", 2: "tab:red"}
    deg = np.linspace(0, 360, 721)
    rad = np.radians(deg)
    amp_t = cap.debug.get("amp", "?")
    for p, ax in enumerate(axes):
        ang, val = samples[p]
        ax.axhline(0, color="0.5", lw=0.8, ls="--")
        # shade this phase's two 60-deg float windows
        for k in PHASE_WINDOWS[p]:
            ax.axvspan(k * 60, (k + 1) * 60, color=colors[p], alpha=0.07)
            ax.text((k + 0.5) * 60, ax.get_ylim()[1], f"s{k}", ha="center",
                    va="top", fontsize=8, color="0.4")
        if len(ang) >= 8:
            a, b, c, rms = fit_sinusoid(ang, val)
            r = math.hypot(a, b)
            ax.scatter(np.degrees(ang), val, s=10, color=colors[p], alpha=0.5,
                       label="float-arc BEMF samples")
            ax.plot(deg, a * np.cos(rad) + b * np.sin(rad) + c, color=colors[p], lw=2,
                    label=f"oracle sinusoid (R={r:.0f}, |c/R|={abs(c/r) if r else 0:.2f})")
            for t, d in zero_crossings(a, b, c):
                td = math.degrees(t)
                inwin = any(k * 60 <= td < (k + 1) * 60 for k in PHASE_WINDOWS[p])
                ax.axvline(td, color="k", lw=1.4, ls="-" if inwin else ":")
                ax.text(td, ax.get_ylim()[0], f" {td:.0f} {d}\n {'IN-WIN' if inwin else 'out'}",
                        fontsize=7, va="bottom",
                        color="k" if inwin else "0.5")
        # observed ground-truth crossings (rare): convert sector pct -> absolute angle
        for k, pct, d in obs[p]:
            adeg = (k + pct / 100.0) * 60
            ax.plot([adeg], [0], marker="o", ms=11, mfc="none", mec="k", mew=2)
            ax.text(adeg, 0, f"  observed {d}", fontsize=7, va="bottom")
        ax.set_ylabel(f"{PHASE_NAMES[p]}  (V_float - neutral)")
        ax.legend(loc="upper right", fontsize=7)
        ax.grid(alpha=0.25)
    axes[-1].set_xlabel("electrical angle (deg) -- float-arc samples folded into one period")
    axes[-1].set_xlim(0, 360)
    axes[-1].set_xticks(range(0, 361, 30))
    fig.suptitle(f"HONEST ZC detector: oracle sinusoid + true crossing + observed marks   "
                 f"({cap.debug.get('hz','?')} Hz, amp={amp_t})   "
                 f"solid vline=crossing IN float window, dotted=outside (real, just not observable in-window)")
    fig.tight_layout()
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_png, dpi=110)
    plt.close(fig)


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path, help="a zc_<hz>_<amp>.log or a dir of them")
    ap.add_argument("--out", type=Path, default=None, help="output dir (default <indir>_oracle)")
    args = ap.parse_args()

    logs = []
    for p in args.paths:
        if p.is_dir():
            logs += sorted(p.glob("zc_*.log"))
        elif p.is_file():
            logs.append(p)
    if not logs:
        raise SystemExit("no zc_*.log files found")
    outdir = args.out or (logs[0].parent.parent / (logs[0].parent.name + "_oracle"))
    n = 0
    for log in logs:
        try:
            cap = parse_capture(log.read_text(errors="replace"))
            render(cap, outdir / (log.stem + ".png"))
            n += 1
            print(f"  {log.name} -> {outdir / (log.stem + '.png')}")
        except Exception as exc:
            print(f"  skip {log.name}: {exc}")
    print(f"rendered {n} -> {outdir}")


if __name__ == "__main__":
    raise SystemExit(main())
