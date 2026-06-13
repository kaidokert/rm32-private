#!/usr/bin/env python3
"""True per-phase BEMF zero-crossing angle by sinusoid fit.

The per-sector ZC analysis (analyze_zero_crossings / zc_sector_stats) can only
*measure* a crossing that actually falls inside the 60-degree float window. For
the other sectors it linearly extrapolates from the window edges -- and a linear
extrapolation of a sine from more than a window away is unreliable (it produced
"shift = -8 logical sectors" nonsense).

This script instead fits each phase's visible BEMF to a sinusoid. A phase floats
in two 60-degree arcs 180 degrees apart per electrical revolution; together that
is enough of the waveform to fit  v_float - neutral = a*cos(t) + b*sin(t) + c
by least squares (the capture is zero-aligned, so frame -> electrical angle is
exact). Solving for the zero gives the TRUE crossing angle whether or not it
lands in an observation window.

Then it asks the question the extrapolated table could not answer honestly:
are the six per-sector offsets all equal (=> a single global commutation-phase
lag, i.e. load angle, fixable by shifting the commutation phase) or genuinely
per-sector (=> something structural)?

Example:
    python scripts/zc_fit.py logs/chase_20260612_200550
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
    SIX_STEP_HIGH_REV,
    SIX_STEP_LOW,
    SIX_STEP_LOW_REV,
    lowpass_channels,
    parse_capture,
)

TWO_PI = 2.0 * math.pi
SECTOR_RAD = TWO_PI / 6.0
ELEC_STEPS_PER_REV = 48  # mirrors scope1.rs
STEPS_PER_PHYS_SECTOR = ELEC_STEPS_PER_REV // 6  # 8


def phys_float(phys: int, reversed_drive: bool) -> int:
    """Float phase (0/1/2) of physical sector 0..5. The scope_common tables are
    6-entry (one per physical sector), unlike the firmware's 24-entry tables."""
    hi = (SIX_STEP_HIGH_REV if reversed_drive else SIX_STEP_HIGH)[phys % 6]
    lo = (SIX_STEP_LOW_REV if reversed_drive else SIX_STEP_LOW)[phys % 6]
    return 3 - hi - lo


def floating_phases(step_in_rev: int, reversed_drive: bool, guard: int) -> set[int]:
    """Which phases are floating at electrical step `step_in_rev` (0..47),
    mirroring firmware set_six_step. A guard band is a FULL COAST (all six FETs
    off) so all three phases float; otherwise the single nominal float phase.
    ELEC_STEPS_PER_REV = 48, 8 steps/physical sector."""
    phys = (step_in_rev // 8) % 6
    pos = step_in_rev % 8
    if guard > 0 and (pos < guard or pos >= 8 - guard):
        return {0, 1, 2}
    return {phys_float(phys, reversed_drive)}


def collect_phase_samples(
    captures: list,
    *,
    smooth_window: int,
    blank_frames: int,
    skip_first_rev: bool,
) -> dict[tuple[int, bool, int], tuple[list[float], list[float]]]:
    """(hz, reversed, phase) -> (angles, v_minus_neutral) over every frame in
    which that phase is floating (nominal window AND guard bands)."""
    out: dict[tuple[int, bool, int], tuple[list[float], list[float]]] = defaultdict(
        lambda: ([], [])
    )
    for cap in captures:
        hz = float(cap.debug.get("hz", "0") or 0)
        if hz <= 0 or len(cap.channels) < 3:
            continue
        rev = cap.debug.get("dir") == "rev"
        guard = int(cap.debug.get("guard", "0") or 0)
        sm = lowpass_channels(cap.channels, smooth_window)
        frames = min(len(c) for c in sm)
        neutral = [(sm[0][i] + sm[1][i] + sm[2][i]) / 3.0 for i in range(frames)]
        fpr = cap.sample_hz / hz  # frames per electrical rev
        fps = fpr / 6.0
        # Blank a few frames after each sector boundary (commutation/demag).
        for i in range(frames):
            k = int(i / fps)  # absolute sector index from capture start
            if skip_first_rev and k < 6:
                continue
            if (i - k * fps) < blank_frames:
                continue
            step = int(i / fpr * ELEC_STEPS_PER_REV) % ELEC_STEPS_PER_REV
            theta = (i / fpr) * TWO_PI
            for fl in floating_phases(step, rev, guard):
                ang, val = out[(int(hz), rev, fl)]
                ang.append(theta)
                val.append(sm[PHASE_TO_CHANNEL[fl]][i] - neutral[i])
    return out


def fit_sinusoid(angles: list[float], vals: list[float]) -> tuple[float, float, float, float]:
    """Least-squares a*cos+b*sin+c; returns (a, b, c, rms_residual)."""
    t = np.asarray(angles)
    y = np.asarray(vals)
    A = np.column_stack([np.cos(t), np.sin(t), np.ones_like(t)])
    coef, *_ = np.linalg.lstsq(A, y, rcond=None)
    a, b, c = coef
    resid = y - A @ coef
    rms = float(np.sqrt(np.mean(resid**2))) if len(resid) else float("nan")
    return float(a), float(b), float(c), rms


def zero_crossings(a: float, b: float, c: float) -> list[tuple[float, str]]:
    """Angles in [0, 2pi) where a*cos t + b*sin t + c = 0, with direction."""
    r = math.hypot(a, b)
    if r < 1e-9 or abs(c) > r:
        return []
    psi = math.atan2(a, b)  # a cos t + b sin t = r sin(t + psi)
    base = math.asin(-c / r)
    out = []
    for theta_shift in (base, math.pi - base):
        t = (theta_shift - psi) % TWO_PI
        deriv = -a * math.sin(t) + b * math.cos(t)
        out.append((t, "rise" if deriv > 0 else "fall"))
    return sorted(out)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("paths", nargs="+", type=Path, help="capture dirs or *_raw.log files")
    parser.add_argument("--smooth", type=int, default=5)
    parser.add_argument("--blank", type=int, default=3, help="frames skipped after each sector start (demag)")
    parser.add_argument("--keep-first-rev", action="store_true", help="include rev 0 (startup transient)")
    args = parser.parse_args()

    logs: list[Path] = []
    for p in args.paths:
        if p.is_dir():
            logs += sorted(p.rglob("*_raw.log"))
        elif p.is_file():
            logs.append(p)
    captures = []
    for log in logs:
        try:
            captures.append(parse_capture(log.read_text(encoding="ascii", errors="replace")))
        except Exception as exc:
            print(f"skip {log}: {exc}")
    if not captures:
        raise SystemExit("no parsable captures")

    samples = collect_phase_samples(
        captures,
        smooth_window=args.smooth,
        blank_frames=args.blank,
        skip_first_rev=not args.keep_first_rev,
    )

    # Fit per (hz, dir, phase), then walk the 6 sectors and place each ZC.
    by_group: dict[tuple[int, bool], dict[int, tuple]] = defaultdict(dict)
    for (hz, rev, phase), (ang, val) in samples.items():
        if len(ang) >= 8:
            by_group[(hz, rev)][phase] = fit_sinusoid(ang, val)

    for (hz, rev), fits in sorted(by_group.items()):
        tag = "rev" if rev else "fwd"
        print(f"\nhz={hz} dir={tag}  ({len([c for c in captures if int(float(c.debug.get('hz',0)))==hz])} captures)")
        print("phase  amp(R)  neutralBias(c)  c/R     rms    | true ZC angles (deg, dir)")
        zc_by_phase: dict[int, list[tuple[float, str]]] = {}
        for phase in range(3):
            if phase not in fits:
                continue
            a, b, c, rms = fits[phase]
            r = math.hypot(a, b)
            zcs = zero_crossings(a, b, c)
            zc_by_phase[phase] = zcs
            zstr = "  ".join(f"{math.degrees(t):6.1f} {d}" for t, d in zcs)
            print(
                f"  {PHASE_NAMES[phase]}   {r:6.0f}   {c:+8.1f}     "
                f"{c / r if r else 0:+5.2f}  {rms:5.0f}   | {zstr}"
            )

        # Per-sector offset: crossing nearest the sector's float window, in window units.
        print("\nsector phase  win-center(deg)  ZC(deg)  dir   offset(win)  comment")
        offsets = []
        for k in range(6):
            fl = phys_float(k, rev)
            if fl not in zc_by_phase or not zc_by_phase[fl]:
                continue
            center = (k + 0.5) * SECTOR_RAD
            # choose the crossing closest to the window center (circular)
            best = min(
                zc_by_phase[fl],
                key=lambda td: abs(((td[0] - center + math.pi) % TWO_PI) - math.pi),
            )
            t, d = best
            delta = ((t - center + math.pi) % TWO_PI) - math.pi
            off = delta / SECTOR_RAD
            offsets.append(off)
            tag2 = "IN-WINDOW" if abs(off) <= 0.5 else "outside"
            print(
                f"s{k}    {PHASE_NAMES[fl]}      {math.degrees(center):6.1f}      "
                f"{math.degrees(t):6.1f}  {d:4s}  {off:+6.2f}      {tag2}"
            )

        if offsets:
            mean = sum(offsets) / len(offsets)
            spread = max(offsets) - min(offsets)
            sd = (sum((o - mean) ** 2 for o in offsets) / len(offsets)) ** 0.5
            print(
                f"\n  offsets: mean {mean:+.2f} win  sd {sd:.2f}  spread {spread:.2f} win"
                f"  ({mean * 4:+.1f} logical sectors mean lag)"
            )
            if spread <= 0.5:
                print(
                    "  -> GLOBAL offset: all six crossings share one lag. This IS load"
                    " angle. Shift commutation phase by the mean and all six center."
                )
            else:
                print(
                    "  -> per-sector offsets genuinely differ even with honest fits"
                    " -- not a single global lag."
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
