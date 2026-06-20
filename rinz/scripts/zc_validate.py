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


def fit_sinusoid(angles, vals):
    """Least-squares a*cos+b*sin+c; returns (a, b, c, rms_residual). (from zc_fit)"""
    t = np.asarray(angles)
    y = np.asarray(vals)
    A = np.column_stack([np.cos(t), np.sin(t), np.ones_like(t)])
    coef, *_ = np.linalg.lstsq(A, y, rcond=None)
    a, b, c = coef
    resid = y - A @ coef
    rms = float(np.sqrt(np.mean(resid**2))) if len(resid) else float("nan")
    return float(a), float(b), float(c), rms


def zero_crossings(a, b, c):
    """Angles in [0, 2pi) where a*cos t + b*sin t + c = 0, with direction."""
    r = math.hypot(a, b)
    if r < 1e-9 or abs(c) > r:
        return []
    psi = math.atan2(a, b)
    base = math.asin(-c / r)
    out = []
    for theta_shift in (base, math.pi - base):
        t = (theta_shift - psi) % TWO_PI
        deriv = -a * math.sin(t) + b * math.cos(t)
        out.append((t, "rise" if deriv > 0 else "fall"))
    return sorted(out)

SECTOR = TWO_PI / 6.0
# Phase -> its current channel label (self-describing dump7 header).
CUR_LABEL = {0: "ch13", 1: "ch16", 2: "ch18"}


def _float_phase(sector: int) -> int:
    hi, lo = SIX_STEP_HIGH[sector % 6], SIX_STEP_LOW[sector % 6]
    return 3 - hi - lo


def _low_phase(sector: int) -> int:
    return SIX_STEP_LOW[sector % 6]


def analyze_one(cap, *, smooth: int = 3, blank: int = 2):
    """Return per-physical-sector estimates for one capture, or None if unusable.

    Each entry: sector -> dict(float_phase, zc_in (list of %), zc_fit_pct,
    i_slope (current ramp slope of the low-driven phase, counts/frame)).
    """
    hz = float(cap.debug.get("hz", "0") or 0)
    if hz <= 0 or len(cap.channels) < 3:
        return None
    sm = lowpass_channels(cap.channels, smooth)
    frames = min(len(c) for c in sm)
    fps = cap.sample_hz / (hz * 6.0)
    fpr = cap.sample_hz / hz
    neutral = _driven_pair_neutral(sm, frames, fps)

    # 1. in-window ZC (analyze_zero_crossings already uses the driven-pair neutral)
    sectors, _, _ = analyze_zero_crossings(cap, smooth_window=smooth, blank_frames=blank)
    zc_in = defaultdict(list)
    for sec in sectors:
        if sec.status == "zc" and sec.zc_pct is not None:
            zc_in[sec.index % 6].append(sec.zc_pct)

    # 2. zc_fit: pool this capture's float-window samples per phase, fit sinusoid.
    samp = {p: ([], []) for p in range(3)}
    cur_by_sec = defaultdict(list)  # physical sector -> [(pos_in_sector, current)]
    labels = cap.labels
    for i in range(frames):
        k = int(i / fps)
        frac = (i - k * fps) / fps  # 0..1 within the sector
        if frac < blank / fps:
            continue
        s = k % 6
        fl = _float_phase(s)
        theta = (i / fpr) * TWO_PI
        samp[fl][0].append(theta)
        samp[fl][1].append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])
        # current of the LOW-driven phase (the one carrying shunt current here)
        lp = _low_phase(s)
        clab = CUR_LABEL[lp]
        if clab in labels:
            cur_by_sec[s].append((frac, sm[labels.index(clab)][i]))

    fits = {p: fit_sinusoid(*samp[p]) for p in range(3) if len(samp[p][0]) >= 8}
    zc_fit_cross = {p: zero_crossings(*fits[p][:3]) for p in fits}

    out = {}
    for k in range(6):
        fl = _float_phase(k)
        entry = {"float_phase": fl, "zc_in": zc_in.get(k, [])}
        # zc_fit: crossing nearest this window's centre -> % of window
        if fl in zc_fit_cross and zc_fit_cross[fl]:
            center = (k + 0.5) * SECTOR
            t, _d = min(
                zc_fit_cross[fl],
                key=lambda td: abs(((td[0] - center + math.pi) % TWO_PI) - math.pi),
            )
            off = (((t - center + math.pi) % TWO_PI) - math.pi) / SECTOR  # -0.5..+0.5 in window
            entry["zc_fit_pct"] = (off + 0.5) * 100.0
            entry["zc_fit_inwin"] = abs(off) <= 0.5
        else:
            entry["zc_fit_pct"] = None
            entry["zc_fit_inwin"] = False
        # current ramp slope of the low-driven phase (counts per fractional sector)
        pts = cur_by_sec.get(k, [])
        if len(pts) >= 4:
            xs = [p[0] for p in pts]
            ys = [p[1] for p in pts]
            n = len(xs)
            mx = sum(xs) / n
            my = sum(ys) / n
            denom = sum((x - mx) ** 2 for x in xs)
            entry["i_slope"] = (
                sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / denom if denom else 0.0
            )
        else:
            entry["i_slope"] = None
        out[k] = entry
    return out


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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--smooth", type=int, default=3)
    ap.add_argument("--blank", type=int, default=2)
    ap.add_argument("--per-capture", action="store_true", help="dump every capture, not the aggregate")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")
    print(f"loaded {len(caps)} captures")

    # Aggregate per (hz, amp) -> per sector lists.
    agg = defaultdict(lambda: defaultdict(lambda: {"zc_in": [], "zc_fit": [], "i_slope": [], "inwin": 0}))
    pooled = {"diff": [], "n_inwin": 0}
    for cap in caps:
        res = analyze_one(cap, smooth=args.smooth, blank=args.blank)
        if res is None:
            continue
        hz = int(float(cap.debug.get("hz", 0)))
        amp = int(cap.debug.get("amp", 0))
        for k, e in res.items():
            a = agg[(hz, amp)][k]
            a["zc_in"].extend(e["zc_in"])
            if e["zc_fit_pct"] is not None:
                a["zc_fit"].append(e["zc_fit_pct"])
            if e["i_slope"] is not None:
                a["i_slope"].append(e["i_slope"])
            # the validation: where an in-window ZC exists, compare to zc_fit
            if e["zc_in"] and e["zc_fit_pct"] is not None:
                a["inwin"] += len(e["zc_in"])
                for z in e["zc_in"]:
                    pooled["diff"].append(abs(z - e["zc_fit_pct"]))
                    pooled["n_inwin"] += 1

    mean = lambda xs: sum(xs) / len(xs) if xs else float("nan")
    for (hz, amp), secs in sorted(agg.items()):
        print(f"\n=== hz={hz} amp={amp/10:.1f}% ===")
        print("sec float | zc_in%   zc_fit%  |diff|  inwin | i_slope(low-drv)")
        for k in range(6):
            e = secs[k]
            zi = mean(e["zc_in"]) if e["zc_in"] else None
            zf = mean(e["zc_fit"]) if e["zc_fit"] else None
            islope = mean(e["i_slope"]) if e["i_slope"] else None
            diff = abs(zi - zf) if (zi is not None and zf is not None) else None
            zi_s = f"{zi:6.1f}" if zi is not None else "   -- "
            zf_s = f"{zf:6.1f}" if zf is not None else "   -- "
            df_s = f"{diff:5.1f}" if diff is not None else "  -- "
            is_s = f"{islope:+7.1f}" if islope is not None else "   -- "
            print(
                f" {k}   {PHASE_NAMES[_float_phase(k)]}   "
                f"{zi_s}  {zf_s}  {df_s}  {e['inwin']:4d}  | {is_s}"
            )

    print("\n=== VALIDATION (where in-window ZC exists, does zc_fit match?) ===")
    if pooled["n_inwin"]:
        d = sorted(pooled["diff"])
        med = d[len(d) // 2]
        # A window is 60 deg electrical, so 1% of window = 0.6 deg.
        print(f"  {pooled['n_inwin']} in-window crossings compared to zc_fit (1% window = 0.6 deg elec)")
        print(f"  |zc_in - zc_fit|: median {med:.1f}% (~{med*0.6:.1f} deg)  mean {mean(d):.1f}%  max {max(d):.1f}%")
        print(f"  agreement <5%: {sum(1 for x in d if x < 5)}/{len(d)}   "
              f"<10%: {sum(1 for x in d if x < 10)}/{len(d)}   "
              f">20% (outliers): {sum(1 for x in d if x > 20)}/{len(d)}")
        outlier_frac = sum(1 for x in d if x > 20) / len(d)
        if med < 10 and outlier_frac < 0.15:
            print("  -> zc_fit TRACKS the direct ZC (tight median, few outliers) -> trust it in deep lock.")
        elif med < 10:
            print(f"  -> zc_fit tracks the BULK (median {med:.1f}%) but has a {outlier_frac:.0%} outlier tail")
            print("     (likely wrong-crossing association or noisy/marginal fits) -> diagnose the tail.")
        else:
            print("  -> zc_fit does NOT track the direct ZC -> reconstruction suspect even in the bulk.")
    else:
        print("  no in-window crossings in this set -> no ground truth to validate against here.")
        print("  (need captures near the catch boundary where the ZC lands in-window.)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
