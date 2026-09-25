"""Score a rung's raw-scan excursion RATE against the predeclared line (E296).

What this measures and why it is the count and not the run length:

* `raw*_run` -- the longest consecutive run of raw scans below a fraction --
  carries **no information** at the rates this bench produces. At p ~ 2.9e-4
  over ~886k scans the expected longest run is 0.68, so an observed 1 is the
  independent-noise prediction rounded up, and E284's predeclared ">= 10" arm
  needed a ~900x higher rate to fire (E291/E295). Withdrawn.
* `raw*_n` **as a rate**, against a **measured bridge-off floor**, does
  discriminate. Over rungs 150-375 it rose linearly with duty, corr = +0.706,
  and rung 400 then landed within **0.6%** of the extrapolation.

The floor is not assumed: `e292-idlescan` ran the same scan path for two 45 s
phases with the bridge never energised and measured **43.8 per 1e6 scans**
(44.9 at EN low, 42.7 at EN high -- so driver-amplifier bias coupling is
excluded). Roughly 18% of the lowest driving rung's rate is instrument.

Usage:
    python scripts/raw_rate.py                      # fit 150-375, score e296-*
    python scripts/raw_rate.py --fit 'e286-*' --score 'e296-*'
"""
from __future__ import annotations

import argparse
import collections
import math
import pathlib
import re
import statistics

# Measured, not assumed. `captures/2026-09-24/e292-idlescan.txt`, bridge off.
BRIDGE_OFF_FLOOR = 43.8
# E296's predeclared tolerance: inside this, a rung is the same
# load-proportional phenomenon and says nothing about a latch.
TOLERANCE_PCT = 25.0


def rates(pattern: str, root: pathlib.Path, bin_ix: int) -> dict[int, list[float]]:
    """Excursion rate per 1e6 drive scans, by rung.

    Captures that do not carry every needed field are **skipped and counted**,
    never defaulted -- a partially written capture would otherwise contribute a
    zero rate and drag a rung's mean down silently.
    """
    out: dict[int, list[float]] = collections.defaultdict(list)
    skipped = 0
    for p in sorted(root.glob(pattern)):
        t = p.read_text(encoding="utf-8", errors="replace")
        try:
            duty = int(re.search(r"\btarget_duty_tenths=(\d+)", t).group(1))
            scans = int(re.search(r"\bdrive_scans=(\d+)", t).group(1))
            n = int(re.search(rf"\braw{bin_ix}_n=(\d+)", t).group(1))
        except AttributeError:
            skipped += 1
            continue
        if scans == 0:
            skipped += 1
            continue
        out[duty].append(1e6 * n / scans)
    if skipped:
        print(f"  ({skipped} capture(s) skipped: incomplete or pre-observer)")
    return out


def fit(by_rung: dict[int, list[float]]) -> tuple[float, float, float]:
    """Least-squares rate-vs-duty line, and the correlation."""
    xs = [d for d, v in by_rung.items() for _ in v]
    ys = [y for v in by_rung.values() for y in v]
    mx, my = statistics.mean(xs), statistics.mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    sxy = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    syy = sum((y - my) ** 2 for y in ys)
    slope = sxy / sxx
    corr = sxy / math.sqrt(sxx * syy)
    return slope, my - slope * mx, corr


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", default="captures/2026-09-24")
    ap.add_argument("--fit", default="e286-*.txt", help="cohort the line is fitted from")
    ap.add_argument("--score", default="e296-*.txt", help="cohort scored against it")
    ap.add_argument("--bin", type=int, default=1, help="raw bin index (1 = the 950 line)")
    args = ap.parse_args()
    root = pathlib.Path(args.root)

    print(f"fitting from {args.fit}:")
    base = rates(args.fit, root, args.bin)
    if not base:
        print("  no captures to fit; refusing to score against nothing")
        return 2
    slope, intercept, corr = fit(base)
    n = sum(len(v) for v in base.values())
    print(f"  n={n} over {len(base)} rungs  "
          f"rate = {intercept:.1f} + {slope:.4f} x duty_tenths  corr={corr:+.3f}")
    print(f"  bridge-off floor {BRIDGE_OFF_FLOOR} /1e6 (measured, e292-idlescan)\n")

    print(f"scoring {args.score} (tolerance +-{TOLERANCE_PCT:.0f}%):")
    sc = rates(args.score, root, args.bin)
    if not sc:
        print("  no scored captures yet")
        return 0
    print(f'{"rung":>5}{"n":>3}{"measured":>10}{"predicted":>11}{"dev":>8}'
          f'{"verdict":>12}{"x floor":>9}')
    steps = []
    for duty in sorted(sc):
        v = sc[duty]
        if not v:
            continue
        m = statistics.mean(v)
        pred = intercept + slope * duty
        dev = 100 * (m - pred) / pred
        on = abs(dev) <= TOLERANCE_PCT
        if not on:
            steps.append((duty, dev))
        print(f"{duty:>5}{len(v):>3}{m:>10.1f}{pred:>11.1f}{dev:>+7.1f}%"
              f'{"ON LINE" if on else "** STEP **":>12}{m / BRIDGE_OFF_FLOOR:>8.1f}x')
    if steps:
        print("\n!! STEP(S) beyond the predeclared tolerance -- the first evidence of a")
        print("   mechanism distinct from the load-proportional rise seen from rung 150:")
        for duty, dev in steps:
            print(f"     rung {duty}: {dev:+.1f}%")
        return 1
    print("\nAll scored rungs on the line: the same load-proportional phenomenon,")
    print("carrying no information about a latch. That is a useful negative.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
