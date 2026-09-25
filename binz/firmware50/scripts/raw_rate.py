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
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import provenance  # noqa: E402  -- sibling script, path set above

# Measured, not assumed. `captures/2026-09-24/e292-idlescan.txt`, bridge off.
BRIDGE_OFF_FLOOR = 43.8

# E296's original tolerance on the RAW rate against a duty line.
# **RETIRED AS CONFOUNDED (E297)** and kept only so the historical verdict can
# be reproduced: it fired at rung 425 (+43.4%) because the excursion rate is
# essentially proportional to *current* and duty is what changes current. The
# measured within-rung CV of the rate is 16.5% (max 19.6%), so +-25% on a 3-run
# mean was only ~2.6 sigma to begin with.
TOLERANCE_PCT = 25.0

# E297's replacement, predeclared before rungs 450+ ran: the rate **per unit
# current** against the mean of rungs 350-425, with a tolerance taken from the
# measured CV rather than from a round number.
RATE_PER_MA_LO = 0.317
RATE_PER_MA_HI = 0.589
SCORE_FROM_TENTHS = 450
# Below this many 930-bin counts per run the deep-bin ratio is Poisson noise
# (sqrt(5) ~ 2.2 at the counts rungs 375-400 produce) and is not read. Stated
# before the data, because rungs 375 and 400 turned out to be the anomalously
# LOW ones -- the apparent 6.7x "jump" at 425 was measured against them.
DEEP_BIN_FLOOR = 25.0

# The window above describes ONE regime and is predeclared for rungs at or above
# this duty. `rate/mA` is not constant across the ladder: it falls from 0.814 at
# rung 200 to ~0.40-0.50 at 350-450, i.e. the excursion rate grows SUB-
# proportionally to current (a power law with exponent ~0.6, not 1.0). So
# "proportional to current" overstates it and the window cannot be applied
# outside the band it was measured in.


def per_run(pattern: str, root: pathlib.Path) -> dict[int, list[tuple]]:
    """Per run: (rate per 1e6 scans, raw1_n, raw2_n, hold_ma), keyed by rung.

    Carries `hold_ma` because the raw excursion rate tracks current rather than
    duty (E297), so the only non-confounded reading is per unit current. Note it
    is **sub**-proportional -- `rate/mA` falls from 0.814 at rung 200 to
    ~0.40-0.50 at 350-450, a power law with exponent ~0.6 -- so "proportional to
    current" is a convenient shorthand and not the measured relationship.
    """
    out: dict[int, list[tuple]] = collections.defaultdict(list)
    # **Provenance is enforced here, not advisory.** `480263F1` measured every
    # `raw*` bin against the wrong reference, and E295-E297 built three entries
    # of analysis on those counts after the defect had already been disclosed.
    # A table lookup now refuses what two reviews had to disassemble an ELF to
    # establish. Field-scoped, so that image's timing fields stay usable.
    usable, refused = provenance.filter_usable(
        sorted(root.glob(pattern)), ("raw1_n", "raw2_n"), strict=True
    )
    if refused:
        print(f"  REFUSED {len(refused)} capture(s) on provenance:")
        for _c, reason in refused[:3]:
            print(f"    {reason}")
        if len(refused) > 3:
            print(f"    ... and {len(refused) - 3} more")
    for p in usable:
        t = p.read_text(encoding="utf-8", errors="replace")
        try:
            duty = int(re.search(r"\btarget_duty_tenths=(\d+)", t).group(1))
            scans = int(re.search(r"\bdrive_scans=(\d+)", t).group(1))
            r1 = int(re.search(r"\braw1_n=(\d+)", t).group(1))
            r2 = int(re.search(r"\braw2_n=(\d+)", t).group(1))
            ma = int(re.search(r"BEMFCURRENT .*?\bhold_ma=(-?\d+)", t).group(1))
        except AttributeError:
            continue
        if scans == 0 or ma == 0:
            continue
        out[duty].append((1e6 * r1 / scans, r1, r2, ma))
    return out


def report_normalised(root: pathlib.Path, patterns: list[str]) -> int:
    """E297's predeclared discriminator, applied mechanically rather than by eye."""
    merged: dict[int, list[tuple]] = collections.defaultdict(list)
    for pat in patterns:
        for duty, rows in per_run(pat, root).items():
            merged[duty].extend(rows)
    if not merged:
        print("  no captures")
        return 0
    print(f'{"rung":>5}{"n":>3}{"rate/1e6":>10}{"hold_ma":>8}{"rate/mA":>9}'
          f'{"verdict":>10}{"raw2_n":>8}{"raw2/raw1":>12}')
    steps = []
    for duty in sorted(merged):
        v = merged[duty]
        rate = statistics.mean(x[0] for x in v)
        ma = statistics.mean(x[3] for x in v)
        rpm = rate / ma
        r1 = statistics.mean(x[1] for x in v)
        r2 = statistics.mean(x[2] for x in v)
        # **Scope.** E297 predeclared this window for rungs 450 and above, from
        # the mean of 350-425. `rate/mA` is strongly duty-dependent below that
        # (5.556 at rung 150 against ~0.45 at 400), so applying the window to
        # the low rungs flags them all and says nothing -- the window describes
        # one regime, not the whole ladder. Lower rungs print as context.
        scored = duty >= SCORE_FROM_TENTHS
        on = RATE_PER_MA_LO <= rpm <= RATE_PER_MA_HI
        if scored and not on:
            steps.append((duty, rpm))
        verdict = ("STEP" if not on else "on line") if scored else "(context)"
        deep = f"{r2 / r1:.4f}" if r2 >= DEEP_BIN_FLOOR else "below floor"
        print(f"{duty:>5}{len(v):>3}{rate:>10.1f}{ma:>8.0f}{rpm:>9.3f}"
              f"{verdict:>10}{r2:>8.1f}{deep:>12}")
    print(f"\n  predeclared (E297): rate/mA in {RATE_PER_MA_LO}..{RATE_PER_MA_HI}; "
          f"deep-bin ratio read only at raw2_n >= {DEEP_BIN_FLOOR:.0f}")
    if steps:
        print("  !! STEP(S) outside the predeclared window:")
        for duty, rpm in steps:
            print(f"     rung {duty}: rate/mA = {rpm:.3f}")
        return 1
    print("  every rung inside the window: excursions track current (sub-proportionally),")
    print("  carrying no information about a latch.")
    return 0


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
    # Defaults point at the **post-fix** cohort. The old e286/e296 captures are
    # refused by the provenance gate for every `raw*` field, so defaulting to
    # them would make the tool print a wall of refusals and no data.
    ap.add_argument("--root", default="captures/2026-09-25")
    ap.add_argument("--fit", default="e301-*.txt", help="cohort the line is fitted from")
    ap.add_argument("--score", default="e301-*.txt", help="cohort scored against it")
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

    print("E297 discriminator -- rate per unit current, the non-confounded reading:")
    rc = report_normalised(root, [args.fit, args.score])

    print(f"\nE296 discriminator, RETIRED AS CONFOUNDED, shown only so its "
          f"historical verdict reproduces (tolerance +-{TOLERANCE_PCT:.0f}%):")
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
        print("  (retired test flags: "
              + ", ".join(f"rung {d} {v:+.1f}%" for d, v in steps) + ")")
    print("\nThe retired test's verdict is not load-bearing; E297's is.")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
