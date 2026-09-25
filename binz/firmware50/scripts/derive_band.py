"""Derive the rate-identity band from measured within-session scatter (E290).

Written **before** the last runs of the low-rung walk landed, so the rule cannot
be adjusted after seeing which runs it moves -- which is the guard E286
predeclared and E290 narrowed.

The rule, fixed in E290:

    band = round(median) +- ceil(3.5 * sd_about_trend)

`sd_about_trend`, not the pooled sd, because E286 predeclared the band from
measured *scatter* and the gated quantity carries a real monotonic duty trend
(+25.6 permille per 1000 duty-tenths over the first four rungs). Pooled sd over
a trending variable is scatter PLUS trend. The trend-removed figure is also the
**stricter** of the two candidates, so the correction cannot buy a pass -- the
direction is stated in E290 for exactly that reason.

The quantity is `cohort.parse`'s `rate_vs_coast_permille` and nothing else.
E290 records why: the firmware also emits `zc_permille_of_6x_coast` (which runs
+2 against it) and a whole-hold `loop_ehz / coast_ehz` ratio (+7, sd 3.7), and
deriving a band from either would be a band for a quantity no gate evaluates.
This script refuses to run on any other field, and asserts every row came
through the primary estimator.
"""
from __future__ import annotations

import collections
import math
import pathlib
import re
import statistics
import sys

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import cohort  # noqa: E402

SIGMA = 3.5
PRIMARY = "matched window vs time-anchored coast"


def rows(pattern: str, root: pathlib.Path) -> list[dict]:
    out = []
    for p in sorted(root.glob(pattern)):
        d = cohort.parse(p)
        if d:
            # **The advance level is part of the cohort's identity, not a
            # detail.** The ladder switches from 20 to 22 partway up (rung 350
            # on this walk), so a band pooled over "the 30 runs" silently
            # averages two different control parameters. `cohort.parse` does not
            # carry it, so it is read here and disclosed below rather than
            # discovered afterwards.
            m = re.search(r"\badvance_level=(\d+)", p.read_text(encoding="utf-8", errors="replace"))
            d["advance_level"] = int(m.group(1)) if m else -1
            out.append(d)
    return out


def trend(xs: list[float], ys: list[float]) -> tuple[float, float]:
    """Least-squares slope and intercept of y on x."""
    mx, my = statistics.mean(xs), statistics.mean(ys)
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return 0.0, my
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    return slope, my - slope * mx


def main() -> int:
    root = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else pathlib.Path("captures/2026-09-24")
    pattern = sys.argv[2] if len(sys.argv) > 2 else "e286-*.txt"
    rs = rows(pattern, root)
    if len(rs) < 3:
        print(f"REFUSED: {len(rs)} scored captures matching {pattern}; need at least 3")
        return 2

    # **Every row must come through the primary estimator.** A `rate_source` of
    # "coast fit unphysical" is a non-result (E273) and a "legacy" row is a
    # different estimator; pooling either into a scatter figure would make the
    # band describe a mixture. Refuse rather than silently average.
    bad = [(r["file"], r["rate_source"]) for r in rs if r["rate_source"] != PRIMARY]
    if bad:
        print(f"REFUSED: {len(bad)} of {len(rs)} rows are not the primary estimator:")
        for f, s in bad:
            print(f"   {f}: {s}")
        return 2

    v = [r["rate_vs_coast_permille"] for r in rs]
    duty = [r["duty"] for r in rs]
    by = collections.defaultdict(list)
    for r in rs:
        by[r["duty"]].append(r["rate_vs_coast_permille"])

    print(f"n = {len(rs)} runs over {len(by)} rungs, all via: {PRIMARY}\n")

    # Disclose cohort heterogeneity BEFORE any pooled statistic is printed, so
    # the reader sees the mixture before the number derived from it. The ladder
    # switches advance level partway up, and a band pooled over "the 30 runs"
    # would silently average two different control parameters.
    adv = collections.defaultdict(list)
    for r in rs:
        adv[r["advance_level"]].append(r["rate_vs_coast_permille"])
    if len(adv) > 1:
        print("!! MIXED COHORT: this pool spans more than one advance level.")
        for a in sorted(adv):
            g = adv[a]
            sd = f"{statistics.stdev(g):.3f}" if len(g) > 1 else "n/a"
            print(f"   advance {a}: n={len(g):>2} median={statistics.median(g)} sd={sd}")
        print("   The band below pools them, as E290 predeclared over 30 runs;")
        print("   the per-level figures above are what that pooling averages.\n")
    else:
        print(f"cohort is homogeneous: advance level {next(iter(adv))} throughout\n")
    print(f'{"rung":>5} {"n":>2} {"values":>22} {"mean":>8} {"sd":>6}')
    within = []
    for d in sorted(by):
        g = by[d]
        sd = statistics.stdev(g) if len(g) > 1 else float("nan")
        if len(g) > 1:
            within.append(statistics.variance(g))
        print(f"{d:>5} {len(g):>2} {str(g):>22} {statistics.mean(g):>8.2f} {sd:>6.3f}")

    slope, intercept = trend(list(map(float, duty)), list(map(float, v)))
    resid = [y - (intercept + slope * x) for x, y in zip(duty, v)]

    pooled = statistics.stdev(v)
    sd_within = math.sqrt(sum(within) / len(within)) if within else float("nan")
    sd_trend = statistics.stdev(resid)
    median = statistics.median(v)

    print(f"\ntrend           : {slope * 1000:+.2f} permille per 1000 duty-tenths")
    print(f"                  ({slope * (max(duty) - min(duty)):+.2f} across {min(duty)}..{max(duty)})")
    print(f"pooled sd       : {pooled:.3f}  -> +-{math.ceil(SIGMA * pooled)}")
    print(f"within-rung sd  : {sd_within:.3f}  -> +-{math.ceil(SIGMA * sd_within)}")
    print(f"sd about trend  : {sd_trend:.3f}  -> +-{math.ceil(SIGMA * sd_trend)}   <-- E290's rule")

    # Python's round() is banker's rounding, so round(1002.5) is 1002, not 1003.
    # E290's rule says `round(median)`; a .5 median is a genuine tie and the
    # choice must not be made by an accident of the language. Stated loudly.
    centre = round(median)
    if median % 1 == 0.5:
        print(f"\n!! median is a tie at {median}: round() gives {centre} "
              f"(banker's rounding). Reported both ways below.")
    half = math.ceil(SIGMA * sd_trend)
    print(f"\nDERIVED BAND    : {centre - half}..{centre + half}   "
          f"(median {median}, +-{half})")

    print(f"\n{'candidate band':>22} {'outside':>8}")
    cands = [("old 990..1010", 990, 1010),
             (f"derived {centre-half}..{centre+half}", centre - half, centre + half)]
    for extra in (4, 5, 6):
        cands.append((f"+-{extra} ({centre-extra}..{centre+extra})", centre - extra, centre + extra))
    for name, lo, hi in cands:
        out = [r for r in rs if not lo <= r["rate_vs_coast_permille"] <= hi]
        print(f"{name:>22} {len(out):>5} of {len(rs)}"
              + ("   " + ", ".join(f'{r["file"]}={r["rate_vs_coast_permille"]}' for r in out) if out else ""))

    # **E286's stop condition.** If the old and the derived band disagree about
    # any rung, the walk order was not legitimate after all and the circularity
    # is real -- the answer is to say so, not to keep the band that passes.
    verdicts = {}
    for name, lo, hi in (("old", 990, 1010), ("derived", centre - half, centre + half)):
        for d in sorted(by):
            ok = all(lo <= x <= hi for x in by[d])
            verdicts.setdefault(d, {})[name] = ok
    differ = [d for d, s in verdicts.items() if s["old"] != s["derived"]]
    if differ:
        print(f"\n!! STOP (E286's own guard): rungs {differ} have different verdicts "
              f"under the old and derived bands. The circularity is real; do not "
              f"pick the band that passes.")
        return 1
    print("\nOK: every rung's verdict is identical under the old and the derived "
          "band, which is the assumption that made the low-rung walk order legitimate.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
