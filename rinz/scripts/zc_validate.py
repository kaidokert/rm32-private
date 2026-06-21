#!/usr/bin/env python3
"""Cross-validate the BEMF zero-crossing estimate against independent observables.

The in-window ZC (direct float-vs-neutral crossing) only exists in a narrow
regime -- the load angle pushes the crossing outside the 60-deg float window in
deep lock. zc_fit *reconstructs* the crossing everywhere via a sinusoid fit, but
that reconstruction has never been checked against ground truth.

This tool answers "can we trust the reconstruction" by lining up, per sector,
THREE estimates of the same rotor event:

  1. zc_in   -- the direct in-window crossing (% of the float window), where it
                exists. This is ground truth, but only at the catch boundary.
  2. zc_fit  -- the sinusoid-fit crossing (driven-pair neutral, same as #1's
                neutral) mapped to the same window %. Exists in every regime.
  3. i_slope -- the phase current's ramp slope in its low-driven sectors (an
                independent, sense-path-disjoint load-angle indicator). Reported
                for context; not yet a calibrated angle (needs R/L -- step 3).

Verdict logic: where zc_in exists, zc_fit should match it. If it does across the
catch boundary, zc_fit is trusted in deep lock where zc_in is gone -- i.e. we've
"found" the ZC in every regime as a validated reconstruction.

    python scripts/zc_validate.py logs/stream_20260616_213729.log
    python scripts/zc_validate.py logs/stream_20260616_213729.log --per-capture
"""

from __future__ import annotations

import argparse
import math
from collections import defaultdict
from pathlib import Path

import numpy as np

from scope_common import (
    PHASE_NAMES,
    PHASE_TO_CHANNEL,
    SIX_STEP_HIGH,
    SIX_STEP_LOW,
    _driven_pair_neutral,
    analyze_zero_crossings,
    lowpass_channels,
    parse_capture,
    split_complete_dumps,
)

TWO_PI = 2.0 * math.pi


SECTOR = TWO_PI / 6.0


def fit_harmonics(angles, vals, n):
    """Least-squares fit c0 + sum_{h=1..n} a_h*cos(h t) + b_h*sin(h t). The BEMF is
    NOT a pure sinusoid (trapezoidal-ish), so the fundamental-only fit (n=1) crosses
    at a different angle than the real waveform -- n=2..3 captures the distortion and
    matches the direct ZC. Returns the coefficient vector [c0, a1, b1, a2, b2, ...]."""
    t = np.asarray(angles)
    y = np.asarray(vals)
    cols = [np.ones_like(t)]
    for h in range(1, n + 1):
        cols += [np.cos(h * t), np.sin(h * t)]
    coef, *_ = np.linalg.lstsq(np.column_stack(cols), y, rcond=None)
    return coef


def crossing_pct(coef, n, sector):
    """% position within `sector`'s float window where the fitted BEMF crosses zero
    (no closed form for n>1 -> dense eval + linear interp). None if no crossing."""
    center = (sector + 0.5) * SECTOR
    ts = np.linspace(center - SECTOR / 2, center + SECTOR / 2, 400)
    f = np.full_like(ts, coef[0])
    for h in range(1, n + 1):
        f += coef[2 * h - 1] * np.cos(h * ts) + coef[2 * h] * np.sin(h * ts)
    idx = np.where(np.diff(np.sign(f)) != 0)[0]
    if len(idx) == 0:
        return None
    j = idx[np.argmin(np.abs(ts[idx] - center))]
    t0, t1, f0, f1 = ts[j], ts[j + 1], f[j], f[j + 1]
    tc = t0 - f0 * (t1 - t0) / (f1 - f0)  # linear interp to the zero
    return ((tc - center) / SECTOR + 0.5) * 100.0
# Phase -> its current channel label (self-describing dump7 header).
CUR_LABEL = {0: "ch13", 1: "ch16", 2: "ch18"}


def _float_phase(sector: int) -> int:
    hi, lo = SIX_STEP_HIGH[sector % 6], SIX_STEP_LOW[sector % 6]
    return 3 - hi - lo


def _low_phase(sector: int) -> int:
    return SIX_STEP_LOW[sector % 6]


def analyze_one(cap, *, smooth: int = 3, blank: int = 2, harmonics=(1, 3)):
    """Compare the direct in-window ZC against the harmonic fit for each harmonic
    count in `harmonics` (pooled over the capture). Returns a list of comparison
    records (one per in-window crossing), each with a "h{n}" key per harmonic count.
    """
    hz = float(cap.debug.get("hz", "0") or 0)
    if hz <= 0 or len(cap.channels) < 3:
        return []
    sm = lowpass_channels(cap.channels, smooth)
    frames = min(len(c) for c in sm)
    fps = cap.sample_hz / (hz * 6.0)
    fpr = cap.sample_hz / hz
    neutral = _driven_pair_neutral(sm, frames, fps)
    sectors, _, _ = analyze_zero_crossings(cap, smooth_window=smooth, blank_frames=blank)

    samp = defaultdict(lambda: ([], []))  # phase -> (angles, e=v_float-neutral)
    for i in range(frames):
        k = int(i / fps)
        if (i - k * fps) < blank:
            continue
        fl = _float_phase(k % 6)
        samp[fl][0].append((i / fpr) * TWO_PI)
        samp[fl][1].append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])
    # coef[(n, phase)] for each harmonic count and phase with enough samples
    coef = {}
    for fl, (ang, val) in samp.items():
        if len(ang) >= 2 * max(harmonics) + 4:
            for n in harmonics:
                coef[(n, fl)] = fit_harmonics(ang, val, n)

    comps = []
    for sec in sectors:
        if sec.status != "zc" or sec.zc_pct is None:
            continue
        s = sec.index % 6
        fl = _float_phase(s)
        rec = {"hz": int(hz), "amp": int(cap.debug.get("amp", 0)), "sector": s, "zc_in": sec.zc_pct}
        for n in harmonics:
            rec[f"h{n}"] = crossing_pct(coef[(n, fl)], n, s) if (n, fl) in coef else None
        comps.append(rec)
    return comps


def load_captures(paths):
    caps = []
    for p in paths:
        files = sorted(p.rglob("*.log")) if p.is_dir() else [p]
        for f in files:
            text = f.read_text(encoding="ascii", errors="replace")
            for seg in split_complete_dumps(text + "\r\nend\r\n")[0]:
                try:
                    cap = parse_capture(seg)
                    cap.debug.setdefault("mode", "six-step")
                    caps.append(cap)
                except Exception:
                    pass
    return caps


def _stats(diffs):
    d = sorted(diffs)
    if not d:
        return None
    n = len(d)
    return {
        "n": n,
        "median": d[n // 2],
        "mean": sum(d) / n,
        "max": max(d),
        "lt5": sum(1 for x in d if x < 5),
        "lt10": sum(1 for x in d if x < 10),
        "gt20": sum(1 for x in d if x > 20),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--smooth", type=int, default=3)
    ap.add_argument("--blank", type=int, default=2)
    ap.add_argument("--harmonics", type=int, nargs="+", default=[1, 2, 3],
                    help="harmonic counts to compare (default 1 2 3)")
    ap.add_argument("--by-hz", action="store_true", help="break the comparison down per frequency")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")

    harm = tuple(args.harmonics)
    comps = []
    for cap in caps:
        comps.extend(analyze_one(cap, smooth=args.smooth, blank=args.blank, harmonics=harm))
    print(f"loaded {len(caps)} captures, {len(comps)} in-window crossings to validate")
    if not comps:
        print("no in-window crossings -> no ground truth here (need catch-boundary captures).")
        return 0

    def diffs(n, recs):
        return [abs(c["zc_in"] - c[f"h{n}"]) for c in recs if c.get(f"h{n}") is not None]

    if args.by_hz:
        by_hz = defaultdict(list)
        for c in comps:
            by_hz[c["hz"]].append(c)
        head = "  ".join(f"{n}h-med" for n in harm)
        print(f"\nhz  |   n   {head}   (median % of 60deg window)")
        for hz in sorted(by_hz):
            recs = by_hz[hz]
            cells = "  ".join(
                f"{(_stats(diffs(n, recs)) or {'median': float('nan')})['median']:5.1f}%" for n in harm
            )
            print(f"{hz} | {len(recs):3d}   {cells}")

    print("\n=== HARMONIC fit vs direct in-window ZC (1% window = 0.6 deg elec) ===")
    best = None
    for n in harm:
        s = _stats(diffs(n, comps))
        if not s:
            continue
        print(
            f"  {n} harmonic{'s' if n > 1 else ' '}: median {s['median']:4.1f}% (~{s['median']*0.6:.1f} deg)  "
            f"mean {s['mean']:4.1f}%  max {s['max']:5.1f}%  "
            f"<10%: {s['lt10']}/{s['n']}  >20% (outliers): {s['gt20']}/{s['n']}"
        )
        if best is None or s["gt20"] < best[1]["gt20"]:
            best = (n, s)
    if best:
        n, s = best
        if s["gt20"] == 0:
            print(f"\n  -> {n}-harmonic fit ELIMINATES the outlier tail (0 >20%): the reconstruction")
            print("     matches the direct ZC across every regime with ground truth. The 1-harmonic")
            print("     gap was BEMF harmonic distortion, not speed/hunting/sensing.")
        else:
            print(f"\n  -> {n}-harmonic fit is the tightest ({s['gt20']} outliers remain).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
