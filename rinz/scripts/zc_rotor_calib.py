#!/usr/bin/env python3
"""Compare rotor-state discriminators across GROUND-TRUTH labeled captures.

Feed it raw scope1 dump logs you have labeled by hand (you watched the rotor),
and it computes a battery of candidate metrics so we can pick the one that
cleanly separates "barely spinning" from "stalled" -- the hard case where the
motor is energized and frozen, so the float windows still show a demag transient
that masquerades as motion.

    python scripts/zc_rotor_calib.py \
        spinning:logs/captures/cap012_six-step_hz60_amp45_raw.log \
        stalled:logs/captures/cap013_six-step_hz60_amp44_raw.log \
        spinning:logs/captures/cap014_...raw.log

Candidate metrics (median over phases B,C -- A skipped for its sense anomaly):
  R          fitted BEMF amplitude at the drive freq (a*cos+b*sin+c). KNOWN to
             false-positive on energized stall (static per-sector offsets project
             onto the fundamental) -- shown so we can watch it fail.
  swing      mean per-window line-fit excursion, blank=2 (current classifier).
  swing_late same, but only the LAST third of each window (demag excluded) --
             the hypothesis: ~0 when stalled, persists when spinning.
  *_per_hz   speed-normalized (BEMF ~ proportional to rotor speed).
  demag_frac 1 - swing_late/swing : how front-loaded the window slope is.
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import numpy as np

from scope_common import (
    PHASE_NAMES,
    PHASE_TO_CHANNEL,
    SIX_STEP_HIGH,
    SIX_STEP_LOW,
    lowpass_channels,
    parse_capture,
)


def _windows(capture, ph, smooth_window):
    """Yield (frame_indices, e_values) for each window where phase ph floats."""
    hz = float(capture.debug.get("hz", "0") or 0)
    sm = lowpass_channels(capture.channels, smooth_window)
    frames = min(len(c) for c in sm)
    neutral = [(sm[0][i] + sm[1][i] + sm[2][i]) / 3.0 for i in range(frames)]
    ch = PHASE_TO_CHANNEL[ph]
    fps = capture.sample_hz / (hz * 6.0)
    k = 0
    while (k + 1) * fps <= frames:
        s = k % 6
        if 3 - SIX_STEP_HIGH[s] - SIX_STEP_LOW[s] == ph:
            i0 = int(round(k * fps))
            i1 = int(round((k + 1) * fps))
            idx = list(range(i0, min(i1, frames)))
            yield idx, [sm[ch][i] - neutral[i] for i in idx]
        k += 1


def _swing(idx, ev, lo_frac, hi_frac=1.0):
    """Line-fit excursion over the [lo_frac, hi_frac] slice of a window."""
    n = len(idx)
    a, b = int(n * lo_frac), int(n * hi_frac)
    if b - a < 3:
        return None
    xs = np.array(idx[a:b], dtype=float)
    ys = np.array(ev[a:b])
    slope = np.polyfit(xs - xs.mean(), ys, 1)[0]
    return abs(slope) * (xs[-1] - xs[0])


def phase_metrics(capture, ph, smooth_window, blank_frames=2):
    hz = float(capture.debug.get("hz", "0") or 0)
    fpr = capture.sample_hz / hz
    thetas, evals, swings_full, swings_late = [], [], [], []
    for idx, ev in _windows(capture, ph, smooth_window):
        # blank=2 absolute for the "full" swing (matches classifier)
        bf = min(blank_frames, max(0, len(idx) - 3))
        sf = _swing(idx[bf:], ev[bf:], 0.0)
        sl = _swing(idx, ev, 2.0 / 3.0)  # last third, demag excluded
        if sf is not None:
            swings_full.append(sf)
        if sl is not None:
            swings_late.append(sl)
        for i, v in zip(idx[bf:], ev[bf:]):
            thetas.append(2.0 * np.pi * (i / fpr))
            evals.append(v)
    if len(evals) < 4:
        return None
    th = np.array(thetas)
    e = np.array(evals)
    design = np.column_stack([np.cos(th), np.sin(th), np.ones_like(th)])
    a, b, c = np.linalg.lstsq(design, e, rcond=None)[0]
    return {
        "R": float(np.hypot(a, b)),
        "swing": float(np.mean(swings_full)) if swings_full else 0.0,
        "swing_late": float(np.mean(swings_late)) if swings_late else 0.0,
    }


def capture_metrics(capture, smooth_window):
    hz = float(capture.debug.get("hz", "0") or 0)
    per = {PHASE_NAMES[ph]: phase_metrics(capture, ph, smooth_window) for ph in range(3)}
    healthy = [per[p] for p in ("B", "C") if per[p]]
    if not healthy:
        healthy = [v for v in per.values() if v]

    def med(key):
        vals = sorted(r[key] for r in healthy)
        n = len(vals)
        return (vals[n // 2] if n % 2 else (vals[n // 2 - 1] + vals[n // 2]) / 2) if vals else 0.0

    R = med("R")
    swing = med("swing")
    swing_late = med("swing_late")
    return {
        "hz": hz,
        "amp": capture.debug.get("amp", "?"),
        "R": R,
        "R_per_hz": R / hz if hz else 0.0,
        "swing": swing,
        "swing_late": swing_late,
        "swing_late_per_hz": swing_late / hz if hz else 0.0,
        "demag_frac": (1.0 - swing_late / swing) if swing else 0.0,
        "per": per,
    }


def main() -> int:
    pairs = []
    sw = 5
    for arg in sys.argv[1:]:
        if arg.startswith("--zc-window="):
            sw = int(arg.split("=", 1)[1])
            continue
        if ":" not in arg:
            print(f"skip (need label:path): {arg}")
            continue
        label, path = arg.split(":", 1)
        pairs.append((label, Path(path)))
    if not pairs:
        print(__doc__)
        return 2

    cols = ["R", "R_per_hz", "swing", "swing_late", "swing_late_per_hz", "demag_frac"]
    print(f"{'label':<10} {'hz':>4} {'amp':>4}  " + "  ".join(f"{c:>10}" for c in cols) + "   file")
    by_label: dict[str, list[dict]] = {}
    for label, path in pairs:
        try:
            cap = parse_capture(path.read_text())
        except Exception as exc:
            print(f"{label:<10} parse failed: {exc} ({path})")
            continue
        m = capture_metrics(cap, sw)
        by_label.setdefault(label, []).append(m)
        print(
            f"{label:<10} {int(m['hz']):>4} {str(m['amp']):>4}  "
            + "  ".join(f"{m[c]:>10.2f}" for c in cols)
            + f"   {path.name}"
        )

    if len(by_label) >= 2:
        print("\nper-label range (min..max) and separation:")
        for c in cols:
            line = f"  {c:<18}"
            ranges = {}
            for label, ms in by_label.items():
                vals = [m[c] for m in ms]
                ranges[label] = (min(vals), max(vals))
                line += f" {label}=[{min(vals):.2f}..{max(vals):.2f}]"
            labels = list(ranges)
            if len(labels) == 2:
                a, b = ranges[labels[0]], ranges[labels[1]]
                gap = max(a[0], b[0]) - min(a[1], b[1])  # >0 means non-overlapping
                line += f"  -> {'CLEAN gap=%.2f' % gap if gap > 0 else 'OVERLAP'}"
            print(line)
        print("\nLook for the metric with a CLEAN (non-overlapping) gap -- that's the discriminator.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
