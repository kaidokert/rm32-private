#!/usr/bin/env python3
"""Streaming (running, decaying-accumulator) harmonic ZC detector -- the firmware-feasible
form of the validated batch oracle (zc_fit).

The batch oracle fits a*cos(t)+b*sin(t)+c to ALL of a phase's pooled float-arc samples; the
firmware cannot pool the whole capture, but it CAN keep running normal-equation sums updated
every tick (uniform per-tick cost -- the constant-cost ISR constraint) with an exponential
decay so the fit reflects ~1-2 recent electrical revs. This module is the host reference for
that design + the validation that the streaming fit converges to the batch crossing.

Per phase, 9 running sums for the 3-param least squares of e = a*cos(t)+b*sin(t)+c:
    [Scc Scs Sc] [a]   [Svc]
    [Scs Sss Ss] [b] = [Svs]
    [Sc  Ss  S1] [c]   [Sv ]
push(phase, theta, e): decay all 9 by `lam`, then add the new sample's contributions.
solve(phase): 3x3 solve -> (a,b,c); cross(phase): the fall/rise crossing angle (deg).

  python scripts/harmonic_stream.py            # validate streaming vs batch on saved captures
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import scope_common as sc

sc.SIX_STEP_HIGH_REV = sc.SIX_STEP_HIGH
sc.SIX_STEP_LOW_REV = sc.SIX_STEP_LOW
import zc_fit as zf  # noqa: E402
from scope_common import parse_capture  # noqa: E402

TWO_PI = 2.0 * math.pi


class StreamingHarmonic:
    """Running per-phase a*cos+b*sin+c via decaying normal-equation sums. `lam` is the
    per-sample decay (0.92 ~= 1/e over ~12 samples ~= 1 rev/phase at our sampling)."""

    def __init__(self, lam: float = 0.92):
        self.lam = lam
        # [Scc, Scs, Sc, Sss, Ss, S1, Svc, Svs, Sv] per phase
        self.s = [[0.0] * 9 for _ in range(3)]

    def push(self, phase: int, theta: float, e: float) -> None:
        c, sn = math.cos(theta), math.sin(theta)
        s = self.s[phase]
        lam = self.lam
        for i in range(9):
            s[i] *= lam
        s[0] += c * c
        s[1] += c * sn
        s[2] += c
        s[3] += sn * sn
        s[4] += sn
        s[5] += 1.0
        s[6] += e * c
        s[7] += e * sn
        s[8] += e

    def solve(self, phase: int):
        s = self.s[phase]
        if s[5] < 6.0:  # need enough effective samples
            return None
        m = np.array([[s[0], s[1], s[2]], [s[1], s[3], s[4]], [s[2], s[4], s[5]]])
        rhs = np.array([s[6], s[7], s[8]])
        try:
            a, b, c = np.linalg.solve(m, rhs)
        except np.linalg.LinAlgError:
            return None
        return float(a), float(b), float(c)

    def cross(self, phase: int, want: str = "fall"):
        ab = self.solve(phase)
        if ab is None:
            return None
        a, b, c = ab
        for t, d in zf.zero_crossings(a, b, c):
            if d == want:
                return math.degrees(t)
        return None


def _replay(cap, lam, blank=3, smooth=5):
    """Feed a capture's float samples frame-by-frame into a StreamingHarmonic, recording the
    per-phase fall crossing as it converges. Returns (final per-phase fall, history)."""
    hz = float(cap.debug.get("hz", "0") or 0)
    sm = zf.lowpass_channels(cap.channels, smooth)
    frames = min(len(c) for c in sm)
    neutral = [(sm[0][i] + sm[1][i] + sm[2][i]) / 3.0 for i in range(frames)]
    fpr = cap.sample_hz / hz
    fps = fpr / 6.0
    sh = StreamingHarmonic(lam)
    hist = {0: [], 1: [], 2: []}
    for i in range(frames):
        k = int(i / fps)
        if k < 6 or (i - k * fps) < blank:
            continue
        step = int(i / fpr * zf.ELEC_STEPS_PER_REV) % zf.ELEC_STEPS_PER_REV
        theta = (i / fpr) * TWO_PI
        for fl in zf.floating_phases(step, False, 0):
            sh.push(fl, theta, sm[sc.PHASE_TO_CHANNEL[fl]][i] - neutral[i])
        for p in range(3):
            cr = sh.cross(p)
            if cr is not None:
                hist[p].append(cr)
    final = {p: (hist[p][-1] if hist[p] else None) for p in range(3)}
    return final, hist


def _batch(cap):
    samples = zf.collect_phase_samples([cap], smooth_window=5, blank_frames=3, skip_first_rev=True)
    out = {}
    for (_h, _r, ph), (ang, val) in samples.items():
        if len(ang) >= 8:
            a, b, c, _ = zf.fit_sinusoid(ang, val)
            zcs = [math.degrees(t) for t, d in zf.zero_crossings(a, b, c) if d == "fall"]
            out[ph] = zcs[0] if zcs else None
    return out


def main() -> int:
    d = Path("logs/zc_20260627_164752")
    caps = sorted(d.glob("zc_*.log"))
    lam = 0.92
    print(f"streaming harmonic (lam={lam}) fall-crossing vs batch oracle, |diff| deg per phase:")
    print(f"{'capture':>14} | A_batch A_strm d | B_batch B_strm d | C_batch C_strm d | maxdiff")
    worst = 0.0
    for f in caps:
        if not any(s in f.name for s in ("250_", "300_", "350_", "400_")):
            continue
        cap = parse_capture(f.read_text(errors="replace"))
        b = _batch(cap)
        strm, _ = _replay(cap, lam)
        cells, diffs = [], []
        for p in range(3):
            bb, ss = b.get(p), strm.get(p)
            if bb is not None and ss is not None:
                dd = abs(((bb - ss + 180) % 360) - 180)
                diffs.append(dd)
                cells.append(f"{bb:6.0f} {ss:6.0f} {dd:4.1f}")
            else:
                cells.append("  --     --   -- ")
        md = max(diffs) if diffs else float("nan")
        worst = max(worst, md if md == md else 0)
        print(f"  {f.stem:>12} | {cells[0]} | {cells[1]} | {cells[2]} | {md:5.1f}")
    print(f"\nworst |diff| streaming-vs-batch across all = {worst:.1f} deg "
          f"({'GOOD: streaming tracks the oracle' if worst < 10 else 'streaming diverges -- tune lam'})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
