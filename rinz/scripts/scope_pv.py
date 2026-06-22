#!/usr/bin/env python3
"""Dual peak+valley (scope2) capture analyzer + validator.

scope2 samples the ADC twice per PWM period -- a valley scan (ON window, neutral
~Vbus/2, full bipolar BEMF) and a peak scan (OFF window, both driven phases at GND).

This tool (a) renders the per-channel valley+peak ENVELOPE figure, and (b) tests
whether the peak scan adds a usable second BEMF view, by building the float-window
harmonic fit valley-only vs valley+peak and comparing both to the direct in-window
ZC (ground truth from the bipolar valley signal), plus per-phase peak-vs-valley
amplitude/offset diagnostics.

VALIDATION FINDING (300 Hz / 15%, logs/dual.log): the valley stream is validated
(median 1.5% vs ground truth -- scope2's valley path == scope1 quality). The PEAK
stream is NOT a clean second BEMF view on this topology:
  * peak BEMF reads only ~0.36x the valley amplitude (robust p10-p90 span, all three
    phases) with a spurious positive offset -- forcing both driven terminals to GND
    in the OFF window destroys the star reference the valley's {Vbus,GND} pair gives,
    and the OFF window sits in post-commutation demag/freewheel;
  * the peak clamp-edge sits ~75% of the window regardless of the true ZC (which
    swings 34-97% with load angle), so it is not a usable ZC indicator either.
So naive dual-pooling DEGRADES the fit (9% vs 1.5%) -- the "2x density" / comparator
cross-check wins do not materialize here. The usable path is the valley sub-stream:
deinterleave(cap)[0] is a clean 20 kHz scope1-equivalent the whole toolchain accepts.
(One operating point so far; the valley validation is solid regardless, but confirm
the peak verdict across duty/RPM before discarding the OFF-window scan for good.)

    python scripts/scope_pv.py logs/dual.log --render
"""

from __future__ import annotations

import argparse
import statistics
from collections import defaultdict
from pathlib import Path

from scope_common import (
    SIX_STEP_HIGH,
    SIX_STEP_LOW,
    classify_rotor_state,
    deinterleave,
    frame_is_valley,
    plot_envelope_snapshot,
)
from zc_validate import TWO_PI, crossing_pct, fit_harmonics, load_captures

CLAMP_FLOOR = 20  # raw float counts below this on a peak frame = clamped neg half -> drop


def _float_phase(s: int) -> int:
    return 3 - SIX_STEP_HIGH[s % 6] - SIX_STEP_LOW[s % 6]


def analyze(cap, *, harmonics: int = 3, blank: int = 2):
    """Per-capture dual analysis. Returns a dict or None."""
    try:
        hz = float(cap.debug.get("hz", "0") or 0)
    except ValueError:
        return None
    if hz <= 0 or len(cap.channels) < 3 or not cap.interleaved:
        return None
    n = cap.frames
    fps = cap.sample_hz / (hz * 6.0)
    fpr = cap.sample_hz / hz
    ch = cap.channels
    valley = frame_is_valley(cap)

    vpool = defaultdict(lambda: ([], []))  # phase -> (angles, e)  valley only
    ppool = defaultdict(lambda: ([], []))  # phase -> (angles, e)  unclamped peak only
    dpool = defaultdict(lambda: ([], []))  # phase -> (angles, e)  valley + unclamped peak
    occ = defaultdict(list)  # sector occurrence k -> [(pos%, e)] valley, for ground truth

    nV = nP = nP_used = 0
    for i in range(n):
        k = int(i / fps)
        pos = i - k * fps
        if pos < blank:  # commutation/demag transient
            continue
        s = k % 6
        fl = _float_phase(s)
        neutral = (ch[SIX_STEP_HIGH[s]][i] + ch[SIX_STEP_LOW[s]][i]) / 2.0
        vfloat = ch[fl][i]
        e = vfloat - neutral
        ang = (i / fpr) * TWO_PI
        if valley[i]:
            nV += 1
            vpool[fl][0].append(ang)
            vpool[fl][1].append(e)
            dpool[fl][0].append(ang)
            dpool[fl][1].append(e)
            occ[k].append(((i - k * fps) / fps * 100.0, e))
        else:
            nP += 1
            if vfloat >= CLAMP_FLOOR:  # keep the unclamped positive-half peak points
                nP_used += 1
                ppool[fl][0].append(ang)
                ppool[fl][1].append(e)
                dpool[fl][0].append(ang)
                dpool[fl][1].append(e)

    need = 2 * harmonics + 4
    vcoef = {fl: fit_harmonics(a, e, harmonics) for fl, (a, e) in vpool.items() if len(a) >= need}
    dcoef = {fl: fit_harmonics(a, e, harmonics) for fl, (a, e) in dpool.items() if len(a) >= need}
    # per-phase valley-vs-peak amplitude/offset mismatch (why peak doesn't pool).
    # Robust span (p10..p90) + median offset, so a few demag spikes in the OFF window
    # don't blow up the estimate (a 3-harmonic R does -- the OFF window sits in demag).
    import numpy as np

    def span(e):
        a = np.asarray(e)
        return (float(np.percentile(a, 90) - np.percentile(a, 10)) / 2.0, float(np.median(a)))

    pdiag = {}
    for fl in range(3):
        if len(vpool[fl][1]) >= need and len(ppool[fl][1]) >= need:
            Rv, ov = span(vpool[fl][1])
            Rp, op = span(ppool[fl][1])
            pdiag[fl] = (Rv, Rp, Rp / Rv if Rv else 0.0, ov, op)

    # ground truth: in-window bipolar crossing per sector occurrence (valley signal)
    res = []  # (zc_in, |zc_in - valley_fit|, |zc_in - dual_fit|)
    for k, pts in occ.items():
        pts.sort()
        zc_in = None
        for j in range(1, len(pts)):
            (p0, e0), (p1, e1) = pts[j - 1], pts[j]
            if e0 == 0:
                continue
            if (e0 < 0) != (e1 < 0):
                zc_in = p0 - e0 * (p1 - p0) / (e1 - e0)
                break
        if zc_in is None:
            continue
        s = k % 6
        fl = _float_phase(s)
        vc = crossing_pct(vcoef[fl], harmonics, s) if fl in vcoef else None
        dc = crossing_pct(dcoef[fl], harmonics, s) if fl in dcoef else None
        res.append((zc_in, None if vc is None else abs(zc_in - vc),
                    None if dc is None else abs(zc_in - dc)))

    return {
        "hz": hz,
        "amp": cap.debug.get("amp", "?"),
        "fps": fps,
        "frames": n,
        "nV": nV,
        "nP": nP,
        "nP_used": nP_used,
        "v_pts": sum(len(a) for a, _ in vpool.values()),
        "d_pts": sum(len(a) for a, _ in dpool.values()),
        "res": res,
        "pdiag": pdiag,
        "rotor": classify_rotor_state(deinterleave(cap)[0]),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("--harmonics", type=int, default=3)
    ap.add_argument("--blank", type=int, default=2)
    ap.add_argument("--render", action="store_true", help="write the envelope PNG per capture")
    args = ap.parse_args()

    caps = load_captures(args.paths)
    if not caps:
        raise SystemExit("no parsable captures")
    inter = [c for c in caps if c.interleaved]
    print(f"loaded {len(caps)} captures; {len(inter)} interleaved (dual peak+valley)")
    if not inter:
        raise SystemExit("no interleaved captures -- is this scope2 data? (header needs 'interleaved')")

    all_v, all_d = [], []
    summary = []
    for cap in inter:
        r = analyze(cap, harmonics=args.harmonics, blank=args.blank)
        if r is None:
            continue
        rot = r["rotor"]
        print(f"\n{int(r['hz'])} Hz  amp={r['amp']}  frames={r['frames']}  "
              f"rotor={rot.get('state')} (plateau_spread={rot.get('plateau_spread')}%)")
        print(f"  density: fps={r['fps']:.1f}/sector total ; "
              f"valley {r['nV']} frames, peak {r['nP']} ({r['nP_used']} unclamped used)")
        print(f"  fit input points (all phases): valley-only {r['v_pts']}  |  "
              f"dual {r['d_pts']}  (x{r['d_pts']/max(r['v_pts'],1):.2f})")
        vr = [a for _, a, _ in r["res"] if a is not None]
        dr = [b for _, _, b in r["res"] if b is not None]
        if r["res"]:
            print(f"  in-window ground-truth crossings: {len(r['res'])}")
            if vr:
                print(f"    |in-window - VALLEY-only fit| (the validated path): "
                      f"median {statistics.median(vr):.1f}%  max {max(vr):.1f}%  (n={len(vr)})")
            if dr:
                print(f"    |in-window - +peak pooled fit|: "
                      f"median {statistics.median(dr):.1f}%  max {max(dr):.1f}%  (n={len(dr)})")
            all_v += vr
            all_d += dr
        else:
            print("  no in-window ground truth in this capture (deep lock) -- fit-only.")

        if r["pdiag"]:
            print("  peak-vs-valley BEMF mismatch (why peak does not pool):")
            print("    phase  valley_R  peak_R  peak/valley  valley_c0  peak_c0")
            for fl, (Rv, Rp, ratio, c0v, c0p) in sorted(r["pdiag"].items()):
                flag = "" if 0.85 <= ratio <= 1.15 and abs(c0p) < 0.3 * Rp else "  <-- mismatch"
                print(f"    {'ABC'[fl]}     {Rv:7.0f}  {Rp:6.0f}    {ratio:5.2f}     "
                      f"{c0v:+8.0f}  {c0p:+7.0f}{flag}")

        try:
            amp_t = int(r["amp"])
        except (TypeError, ValueError):
            amp_t = 0
        pv = [ratio for (_, _, ratio, _, _) in r["pdiag"].values()]
        summary.append({
            "hz": int(r["hz"]),
            "amp": amp_t / 10.0,
            "duty": amp_t / 10.0 * 2 / 3,  # actual PWM duty ~= amp * 2/3
            "state": rot.get("state"),
            "spread": rot.get("plateau_spread"),
            "pv": statistics.median(pv) if pv else None,
            "vres": statistics.median(vr) if vr else None,
            "dres": statistics.median(dr) if dr else None,
            "ninwin": len(r["res"]),
        })

        if args.render:
            out = (cap_path := args.paths[0])
            out = (out if out.is_dir() else out.parent) / f"envelope_{int(r['hz'])}hz_{amp_t}.png"
            plot_envelope_snapshot(cap, out)
            print(f"  wrote {out}")

    if len(summary) > 1:
        print("\n=== DUTY SWEEP SUMMARY (the low-duty rescue test) ===")
        print("  as amp drops, the valley pinches (sensing 'spread' rises, rotor -> uncertain).")
        print("  RESCUE would show: peak/valley ratio -> ~1 and a usable peak exactly there.\n")
        print(f"  {'hz':>4} {'amp%':>5} {'duty%':>5}  {'rotor':>9} {'v_spread%':>9}  "
              f"{'peak/val':>8}  {'valley_res':>10} {'+peak_res':>9}  inwin")
        for s in sorted(summary, key=lambda x: (x["hz"], -x["amp"])):
            sp = f"{s['spread']:.0f}" if s["spread"] is not None else "-"
            pv = f"{s['pv']:.2f}" if s["pv"] is not None else "-"
            vr = f"{s['vres']:.1f}%" if s["vres"] is not None else "-"
            dr = f"{s['dres']:.1f}%" if s["dres"] is not None else "-"
            print(f"  {s['hz']:>4} {s['amp']:>5.1f} {s['duty']:>5.1f}  {str(s['state']):>9} "
                  f"{sp:>9}  {pv:>8}  {vr:>10} {dr:>9}  {s['ninwin']}")

    if all_v and all_d:
        mv, md = statistics.median(all_v), statistics.median(all_d)
        print(f"\n=== VERDICT (1% window = 0.6 deg elec) ===")
        print(f"  VALLEY-only fit vs in-window ZC: median {mv:.1f}%  (n={len(all_v)})  <- validated")
        print(f"  +peak pooled  fit vs in-window ZC: median {md:.1f}%  (n={len(all_d)})")
        if md <= mv + 1.0:
            print("  -> peak pooling helps (or is neutral): the second view is usable here.")
        else:
            print("  -> peak pooling DEGRADES the fit. On this topology the OFF-window peak")
            print("     scan is not a clean second BEMF (scale/offset mismatch + demag bleed).")
            print("     Use the valley sub-stream: deinterleave(cap)[0] feeds the whole toolchain.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
