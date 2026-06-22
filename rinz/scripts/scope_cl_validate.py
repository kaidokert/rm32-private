#!/usr/bin/env python3
"""Stage-1 gate for the closed-loop transition: does the firmware's REAL-TIME ZC
detector match the OFFLINE multi-harmonic oracle on the SAME captured frames?

scope_cl.rs (observe-only) logs, per commutation, where its in-ISR detector thinks
the BEMF zero-crossing is (`cl i=.. phys=.. zc=.. per=..` lines, zc in % of the
60-deg float window, -1 = no in-window crossing). This tool recomputes the oracle
(the validated 3-harmonic fit, driven-pair neutral) on the dumped valley frames and
overlays the two, per physical sector.

Per CLOSED_LOOP_PLAN.md, this is the ONLY thing that certifies the real-time detector
before any feedback is engaged. Checkpoint D1: real-time vs oracle median <= 5% window
(~3 deg) with NO stable systematic bias. A large-but-consistent bias is flagged
separately -- that is a fixable tick-vs-frame alignment offset, not detector error;
the SPREAD is the detector quality.

    python scripts/scope_cl_validate.py logs/cl_capture.log [--render]
"""

from __future__ import annotations

import argparse
import re
import statistics
from collections import defaultdict
from pathlib import Path

from scope_common import PHASE_TO_CHANNEL, _driven_pair_neutral, lowpass_channels
from zc_validate import TWO_PI, _float_phase, crossing_pct, fit_harmonics, load_captures

_CL_RE = re.compile(r"\bcl i=(\d+)\s+phys=(\d+)\s+zc=(-?\d+)\s+per=(\d+)")


def parse_cl(text: str):
    """[(i, phys, zc_pct, period_ticks)] from the `cl ...` lines; zc_pct -1 = none."""
    return [
        (int(m[1]), int(m[2]), int(m[3]), int(m[4]))
        for m in (_CL_RE.search(ln) for ln in text.splitlines())
        if m
    ]


def oracle_crossings(cap, *, harmonics=3, smooth=3, blank=2):
    """{phys sector -> oracle ZC pct} from the 3-harmonic fit on the valley frames."""
    try:
        hz = float(cap.debug.get("hz", "0") or 0)
    except ValueError:
        return {}
    if hz <= 0 or len(cap.channels) < 3 or cap.interleaved:
        return {}  # scope_cl is valley-only (not interleaved); guard anyway
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
    out = {}
    for s in range(6):
        fl = _float_phase(s)
        a, v = samp[fl]
        if len(a) >= 2 * harmonics + 6:
            out[s] = crossing_pct(fit_harmonics(a, v, harmonics), harmonics, s)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--harmonics", type=int, default=3)
    ap.add_argument("--d1", type=float, default=5.0, help="D1 threshold: median |rt-oracle| %% window")
    ap.add_argument("--render", action="store_true")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")

    pairs = []  # (phys, rt_zc, oracle_zc, diff)
    n_cl = n_rt_inwin = n_caps_with_cl = 0
    for cap in caps:
        cl = parse_cl(cap.text)
        if not cl:
            continue
        n_caps_with_cl += 1
        orc = oracle_crossings(cap, harmonics=args.harmonics)
        for _, phys, zc, _per in cl:
            n_cl += 1
            if zc < 0:
                continue
            n_rt_inwin += 1
            o = orc.get(phys % 6)
            if o is None:
                continue
            pairs.append((phys % 6, float(zc), float(o), float(zc) - float(o)))

    if n_caps_with_cl == 0:
        raise SystemExit("no `cl ...` lines found -- is this a scope_cl (observe-only) capture?")
    print(f"{n_caps_with_cl} captures with a cl log; {n_cl} commutations "
          f"({n_rt_inwin} with an in-window real-time ZC)")
    if not pairs:
        print("no overlapping in-window ZCs (real-time AND oracle) to compare.")
        print("-> likely deep lock (ZC out of window). Pick a zc_map cell with in-window ground truth.")
        return 0

    diffs = [abs(d) for *_ , d in pairs]
    signed = [d for *_, d in pairs]
    med = statistics.median(diffs)
    bias = statistics.median(signed)
    print(f"\nreal-time vs oracle (1% window = 0.6 deg elec), n={len(pairs)}:")
    print(f"  median |d|: {med:.1f}%   max: {max(diffs):.1f}%   "
          f"<5%: {sum(1 for d in diffs if d < 5)}/{len(diffs)}")
    print(f"  median signed d (bias): {bias:+.1f}%  (consistent bias = fixable tick/frame offset)")

    print("\n  per phys sector:  rt_zc  oracle  d   (n)")
    bys = defaultdict(list)
    for phys, zc, o, d in pairs:
        bys[phys].append((zc, o, d))
    for s in sorted(bys):
        v = bys[s]
        print(f"    s{s}: {statistics.median(x[0] for x in v):5.0f}  "
              f"{statistics.median(x[1] for x in v):5.0f}  "
              f"{statistics.median(x[2] for x in v):+5.0f}   ({len(v)})")

    # de-biased spread: the real detector-quality metric (remove a constant offset)
    debiased = statistics.median(abs(d - bias) for d in signed)
    print("\n=== STAGE-1 GATE (D1) ===")
    print(f"  median |d|              : {med:.1f}%   (threshold {args.d1:.0f}%)")
    print(f"  de-biased median |d|   : {debiased:.1f}%   (detector quality, alignment removed)")
    if med <= args.d1:
        print("  -> PASS: real-time detector matches the oracle. Stage-1 gate met.")
    elif debiased <= args.d1:
        print(f"  -> NEAR: spread is fine ({debiased:.1f}%) but there is a {bias:+.0f}% systematic")
        print("     offset -> a tick-vs-frame alignment fix in scope_cl, NOT a detector error.")
        print("     Correct the alignment, re-capture, then re-gate. Do NOT proceed to Stage 2 yet.")
    else:
        print("  -> FAIL (D1): real-time detector does not match the oracle. Do NOT engage feedback.")
        print("     Diagnose (CL_BLANK/CL_CONFIRM, sign-change logic) before anything else.")

    if args.render:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        fig, ax = plt.subplots(figsize=(7, 7))
        xs = [o for _, _, o, _ in pairs]
        ys = [zc for _, zc, _, _ in pairs]
        ax.scatter(xs, ys, c=[p for p, *_ in pairs], cmap="tab10", s=30, alpha=0.8)
        ax.plot([0, 100], [0, 100], "k--", alpha=0.5, label="agreement")
        ax.set_xlabel("offline oracle ZC (% window)")
        ax.set_ylabel("firmware real-time ZC (% window)")
        ax.set_title(f"Stage-1: real-time detector vs oracle  (median |d|={med:.1f}%, bias={bias:+.0f}%)")
        ax.set_xlim(0, 100)
        ax.set_ylim(0, 100)
        ax.legend()
        ax.grid(alpha=0.3)
        out = (args.paths[0] if args.paths[0].is_dir() else args.paths[0].parent) / "cl_oracle_overlay.png"
        fig.tight_layout()
        fig.savefig(out, dpi=110)
        print(f"\nwrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
