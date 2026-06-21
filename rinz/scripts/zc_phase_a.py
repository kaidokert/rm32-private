#!/usr/bin/env python3
"""Phase-A anomaly closeout: is it a sense-path gain error, or not?

History: early per-sector analysis flagged phase A (PA4 / ADC2 ch17) as
"anomalous" -- its fitted floating-window BEMF amplitude read about half of B/C
and it carried a large DC offset (c/R ~ +2.4), so A never produced an in-window
crossing. The standing hypothesis (OBSERVATION_LEVERS_MEMO.md) was a linear
sense-path error V_meas = g_A * V_true + o_A, to be confirmed by adding a free
gain g_A to the fit and watching the asymmetry collapse.

This tool tests that hypothesis directly, three independent ways, all from
existing captures (no hardware lead-swap needed):

  1. DRIVEN-RAIL GAIN. The high/low driven plateaus pass through the SAME
     resistor divider as the floating BEMF read. A per-channel divider gain
     error would scale BOTH. So matched driven plateaus => matched sense gain
     => no per-channel gain error to correct. (sense_spread%)

  2. AMPLITUDE vs OPERATING POINT. A fixed gain error is amplitude-INDEPENDENT.
     If R_A/R_C instead varies with commanded amplitude (= duty = load angle),
     the low amplitude is an operating-point effect, not a gain. (R-ratio table)

  3. OFFSET. The big +2.4 c/R was measured against the OLD (A+B+C)/3 neutral.
     Re-checked against the validated driven-pair neutral, the offset should be
     small for all three phases (the anomaly was a neutral-method artifact).

Only captures that classify as solidly locked AND well-sensed (low plateau
spread) are used -- the marginal/uncertain low-duty regime gives meaningless
amplitudes by design.

    python scripts/zc_phase_a.py logs/sweep_<ts> [more dirs...] --render
    python scripts/zc_phase_a.py logs/sweep_<ts> --hz-lo 250 --hz-hi 550
"""

from __future__ import annotations

import argparse
import math
import statistics
from collections import defaultdict
from pathlib import Path

import numpy as np

from scope_common import (
    PHASE_TO_CHANNEL,
    _driven_pair_neutral,
    classify_rotor_state,
    lowpass_channels,
)
from zc_validate import TWO_PI, _float_phase, fit_harmonics, load_captures


def capture_metrics(cap, *, smooth: int = 3, blank: int = 2, harmonics: int = 3):
    """Per-capture: float-window R and c/R per phase (driven-pair neutral,
    N-harmonic fit) plus driven-rail high plateau per channel. None if unusable."""
    hz = float(cap.debug.get("hz", "0") or 0)
    if hz <= 0 or len(cap.channels) < 3:
        return None
    sm = lowpass_channels(cap.channels, smooth)
    frames = min(len(c) for c in sm)
    fps = cap.sample_hz / (hz * 6.0)
    fpr = cap.sample_hz / hz
    neutral = _driven_pair_neutral(sm, frames, fps)

    samp = defaultdict(lambda: ([], []))
    for i in range(frames):
        k = int(i / fps)
        if (i - k * fps) < blank:
            continue
        fl = _float_phase(k % 6)
        samp[fl][0].append((i / fpr) * TWO_PI)
        samp[fl][1].append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])

    R, coff = {}, {}
    for ph in range(3):
        ang, val = samp[ph]
        if len(ang) < 2 * harmonics + 6:
            return None
        coef = fit_harmonics(ang, val, harmonics)
        r = math.hypot(coef[1], coef[2])
        if r <= 0:
            return None
        R[ph] = r
        coff[ph] = coef[0] / r
    # driven-rail high plateau per channel (the sense-gain proxy at Vbus)
    high = [float(np.percentile(np.array(cap.channels[ch]), 92)) for ch in range(3)]
    try:
        amp = int(cap.debug.get("amp", 0))
    except (TypeError, ValueError):
        amp = None
    return {"hz": hz, "amp": amp, "R": R, "c_over_R": coff, "high": high}


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--smooth", type=int, default=3)
    ap.add_argument("--blank", type=int, default=2)
    ap.add_argument("--harmonics", type=int, default=3)
    ap.add_argument("--hz-lo", type=float, default=250.0)
    ap.add_argument("--hz-hi", type=float, default=550.0)
    ap.add_argument("--plateau-spread-max", type=float, default=2.0,
                    help="keep only captures sensed at least this tightly (%%)")
    ap.add_argument("--all-states", action="store_true",
                    help="do NOT require solid lock (debug; pollutes with marginal data)")
    ap.add_argument("--render", action="store_true")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")

    kept = []
    for cap in caps:
        hz = float(cap.debug.get("hz", "0") or 0)
        if not (args.hz_lo <= hz <= args.hz_hi):
            continue
        rs = classify_rotor_state(cap)
        if not args.all_states and rs.get("state") != "locked":
            continue
        ps = rs.get("plateau_spread")
        if ps is not None and ps > args.plateau_spread_max:
            continue
        m = capture_metrics(cap, smooth=args.smooth, blank=args.blank, harmonics=args.harmonics)
        if m and m["amp"] is not None:
            kept.append(m)

    print(f"loaded {len(caps)} captures; {len(kept)} solidly-locked, well-sensed "
          f"in {args.hz_lo:.0f}-{args.hz_hi:.0f} Hz")
    if len(kept) < 20:
        raise SystemExit("too few clean captures to conclude")

    # --- evidence 1: driven-rail sense gain ---
    hi = [statistics.median(m["high"][ch] for m in kept) for ch in range(3)]
    sense_spread = (max(hi) - min(hi)) / (sum(hi) / 3) * 100.0
    print("\n[1] DRIVEN-RAIL GAIN (same divider as the float read):")
    print(f"    high plateau  A={hi[0]:.0f}  B={hi[1]:.0f}  C={hi[2]:.0f}  "
          f"spread {sense_spread:.1f}%")

    # --- evidence 2: amplitude dependence of R_A/R_C ---
    byamp = defaultdict(list)
    for m in kept:
        byamp[m["amp"] // 10].append(m)
    print("\n[2] FLOAT-WINDOW AMPLITUDE vs COMMANDED AMP (a fixed gain is flat):")
    print(f"    {'amp%':>4} {'n':>4}  RA/RC  RB/RC | c/R: A      B      C")
    ratios_a, bin_med_c = [], []
    for amp in sorted(byamp):
        v = byamp[amp]
        if len(v) < 5:
            continue
        med = lambda f: statistics.median(f(m) for m in v)
        ra = med(lambda m: m["R"][0] / m["R"][2])
        rb = med(lambda m: m["R"][1] / m["R"][2])
        ca = med(lambda m: m["c_over_R"][0])
        cb = med(lambda m: m["c_over_R"][1])
        cc = med(lambda m: m["c_over_R"][2])
        ratios_a.append(ra)
        bin_med_c += [abs(ca), abs(cb), abs(cc)]
        print(f"    {amp:>4} {len(v):>4}  {ra:5.2f}  {rb:5.2f} | {ca:+5.2f}  {cb:+5.2f}  {cc:+5.2f}")
    ra_range = max(ratios_a) - min(ratios_a) if ratios_a else 0.0

    # --- evidence 3: offset magnitude vs old (A+B+C)/3 ---
    # robust: worst per-amp-bin MEDIAN |c/R| (systematic offset, not capture outliers)
    max_c = max(bin_med_c) if bin_med_c else float("nan")
    print(f"\n[3] OFFSET (driven-pair neutral): worst per-bin median |c/R| = {max_c:.2f}")
    print("    (old (A+B+C)/3 neutral reported c/R ~ +2.4 on A)")

    # --- verdict ---
    print("\n=== VERDICT ===")
    gain_ok = sense_spread < 5.0
    not_fixed = ra_range > 0.20
    offset_fixed = max_c < 0.5
    print(f"  sense divider matched (<5%):        {gain_ok}  ({sense_spread:.1f}%)")
    print(f"  R_A/R_C varies with amp (>0.20):    {not_fixed}  (range {ra_range:.2f})")
    print(f"  offset resolved by driven neutral:  {offset_fixed}  (max |c/R| {max_c:.2f})")
    if gain_ok and not_fixed and offset_fixed:
        print("\n  -> Phase-A 'sense-path gain error' is REFUTED. The divider gain is")
        print("     matched (driven rails), the low float amplitude is operating-point")
        print("     (load-angle) dependent rather than a fixed gain, and the offset half")
        print("     of the original anomaly was a (A+B+C)/3 neutral artifact, already")
        print("     fixed. A host-side g_A would mask a real load-angle effect, not")
        print("     correct a calibration error. The validated detector (B/C + harmonic")
        print("     crossing) is right to not depend on A's amplitude.")
    else:
        print("\n  -> inconclusive / gain hypothesis not cleanly refuted; inspect the table.")

    if args.render:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        amps = sorted(a for a in byamp if len(byamp[a]) >= 5)
        ra = [statistics.median(m["R"][0] / m["R"][2] for m in byamp[a]) for a in amps]
        rb = [statistics.median(m["R"][1] / m["R"][2] for m in byamp[a]) for a in amps]
        ca = [statistics.median(m["c_over_R"][0] for m in byamp[a]) for a in amps]
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 4.5))
        ax1.plot(amps, ra, "o-", color="crimson", label="R_A / R_C")
        ax1.plot(amps, rb, "s-", color="steelblue", label="R_B / R_C")
        ax1.axhline(hi[0] / hi[2], color="crimson", ls="--", alpha=0.6,
                    label=f"divider gain A/C = {hi[0]/hi[2]:.2f} (fixed-gain prediction)")
        ax1.axhline(1.0, color="gray", ls=":", alpha=0.5)
        ax1.set_xlabel("commanded amp %")
        ax1.set_ylabel("float-window fundamental ratio")
        ax1.set_title("Float amplitude is NOT a fixed gain\n(swings with load angle; "
                      "driven-rail gain is flat)", fontsize=9)
        ax1.legend(fontsize=8)
        ax1.grid(alpha=0.3)
        ax2.plot(amps, ca, "o-", color="crimson", label="A")
        ax2.axhspan(-0.5, 0.5, color="green", alpha=0.08)
        ax2.axhline(2.4, color="black", ls="--", alpha=0.5, label="old (A+B+C)/3 c/R ~ +2.4")
        ax2.set_xlabel("commanded amp %")
        ax2.set_ylabel("c / R  (offset / amplitude)")
        ax2.set_title("Offset anomaly gone with driven-pair neutral", fontsize=9)
        ax2.legend(fontsize=8)
        ax2.grid(alpha=0.3)
        fig.suptitle(f"Phase-A closeout: {args.paths[0].name} "
                     f"({len(kept)} locked captures, {args.hz_lo:.0f}-{args.hz_hi:.0f} Hz)")
        fig.tight_layout()
        out = (args.paths[0] if args.paths[0].is_dir() else args.paths[0].parent) / "phase_a_closeout.png"
        fig.savefig(out, dpi=110)
        print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
