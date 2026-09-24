#!/usr/bin/env python3
"""Powered speed against the coast that follows it, on matched windows.

Campaign 8 step 3. Two speeds, both in electrical Hz, each with its origin
stated rather than assumed:

**Powered.** `BEMFTAIL accepts span_us start_before_stop_us end_before_stop_us`
is a window that ends at the last accepted crossing before the stop and is one
to two `TAIL_WINDOW_US` long. Its speed is `accepts / span_us` sixths of a
revolution per µs, so

    ehz_powered = accepts * 1e6 / (6 * span_us)

and the window's midpoint sits `(start_before_stop + end_before_stop) / 2` µs
before the stop stamp. Nothing is rounded to whole µs: `BEMFRATE`'s
`mean_sector_us` is, which is why its `ehz_from_sector` is not used here.

**Coast.** `COASTTIMING offset_us first_us iv_us=...` gives the coast window's
own origin (`offset_us` after the stop stamp), the delay to the first debounced
comparator transition, and the first eight spacings. **Consecutive spacings are
paired** before anything is fitted: one phase's rising and falling half-cycles
are not equal here, and the asymmetry is rung-dependent -- about 4% peak to
peak between neighbouring spacings at 25% (409/425) and about 11% at 15%
(748/673) -- so a fit through single half-periods is dominated by which phase
of the alternation it starts on. It gave slopes of -8000 eHz/s and one of
+1500 eHz/s on five runs of one image at one rung, which is how this was
caught. A pair is one full electrical cycle:

    ehz_k = 1e6 / (iv[2k] + iv[2k+1])   at  t_k = offset + first + Σ(earlier) + (iv[2k] + iv[2k+1]) / 2

measured from the stop stamp. A least-squares line through those four points,
evaluated at t = 0, is the rotor's speed at the stop. The lever arm
(`offset_us + first_us`) is short next to the coast's own span but it is not
bounded by anything: 237-455 µs over five 25% runs, 645 µs in a 15% one, so it
is printed per run rather than characterised.

`--drop-first` discards `iv[0]` before pairing. `cohort.py` drops it as a
possible demagnetisation transient; this estimator keeps it by default, and the
switch is there so the difference can be measured rather than argued (the
independent review of E155 asked for exactly this).

**What is compared.** The powered window sits inside the governed hold, where
the duty is held and the speed is steady, so it is *not* slope-corrected: doing
that carried a coast slope across a 1.4 s lever arm and produced nonsense
(first version of this script). The coast estimate is brought back to the stop
instead, over a lever arm three orders of magnitude shorter. The residual is
reported in per mille. No tolerance is applied: calibrating a band is a
separate job, done from repeat runs of one image at one rung (`--calibrate`).

Usage:
    python scripts/speed.py captures/2026-09-23/*.txt
    python scripts/speed.py --calibrate captures/chain/oracle-375-*.txt
"""
from __future__ import annotations

import argparse
import glob
import pathlib
import re
import statistics
import sys


def fields(text: str, prefix: str) -> dict[str, str]:
    for line in text.splitlines():
        if line.startswith(prefix + " "):
            out = {}
            for tok in line.split()[1:]:
                if "=" in tok:
                    k, v = tok.split("=", 1)
                    out[k] = v
            return out
    return {}


def coast_fit(offset_us: int, first_us: int, iv: list[int]) -> tuple[float, float, float] | None:
    """(ehz at the stop, slope in eHz per µs, lever arm µs) from full cycles."""
    xs, ys, t = [], [], float(offset_us + first_us)
    lever = t
    k = 0
    while k + 1 < len(iv):
        cycle = iv[k] + iv[k + 1]
        if iv[k] <= 0 or iv[k + 1] <= 0:
            break
        xs.append(t + cycle / 2.0)
        ys.append(1e6 / cycle)
        t += cycle
        k += 2
    if len(xs) < 3:
        return None
    n = len(xs)
    mx, my = sum(xs) / n, sum(ys) / n
    sxx = sum((x - mx) ** 2 for x in xs)
    if sxx == 0:
        return None
    slope = sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / sxx
    return my + slope * (0.0 - mx), slope, lever


def elf_sha(text: str) -> str:
    """The image a capture was taken on, from the fixture's header line."""
    m = re.search(r"^# elf_sha256 ([0-9A-Fa-f]+)", text, re.MULTILINE)
    return m.group(1)[:8].upper() if m else "unknown"


def one(path: pathlib.Path, drop_first: bool = False) -> dict | None:
    text = path.read_text(errors="replace")
    tail = fields(text, "BEMFTAIL")
    coast = fields(text, "COASTTIMING")
    run = fields(text, "BEMFRUN")
    if not tail or not coast:
        return None
    accepts, span = int(tail.get("accepts", 0)), int(tail.get("span_us", 0))
    if accepts == 0 or span == 0:
        return None
    powered = accepts * 1e6 / (6.0 * span)
    mid_before_stop = (int(tail["start_before_stop_us"]) + int(tail["end_before_stop_us"])) / 2.0
    iv = [int(x) for x in coast.get("iv_us", "0").split(",") if x]
    if drop_first and len(iv) > 2:
        # Dropping iv[0] moves the origin forward by it, so the lever arm grows.
        first = int(coast.get("first_us", 0)) + iv[0]
        iv = iv[1:]
    else:
        first = int(coast.get("first_us", 0))
    fit = coast_fit(int(coast.get("offset_us", 0)), first, iv)
    if fit is None:
        return None
    at_stop, slope, lever = fit
    return {
        "file": path.name,
        "elf": elf_sha(text),
        "duty": int(run.get("target_duty_tenths", 0)),
        "powered": powered,
        "coast_at_stop": at_stop,
        "slope_ehz_per_s": slope * 1e6,
        "lever_us": lever,
        "mid_before_stop_us": mid_before_stop,
        "offset_us": int(coast.get("offset_us", 0)),
        "span_us": span,
        "residual_permille": (powered - at_stop) * 1000.0 / at_stop,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("captures", nargs="+")
    ap.add_argument(
        "--calibrate",
        action="store_true",
        help="summarise the scatter of the residual across the given captures; "
        "the band is whatever the scatter says, and needs >= 5 runs of one "
        "image at one rung to mean anything",
    )
    ap.add_argument(
        "--drop-first",
        action="store_true",
        help="discard iv[0] (the possible demagnetisation transient) before pairing",
    )
    a = ap.parse_args()
    paths = [pathlib.Path(p) for pat in a.captures for p in sorted(glob.glob(pat))]
    rows = [r for r in (one(p, a.drop_first) for p in paths) if r]
    if not rows:
        print("no capture carried both BEMFTAIL and COASTTIMING", file=sys.stderr)
        print("(BEMFTAIL exists only in images from E155 onward)", file=sys.stderr)
        return 1
    print(
        f'{"capture":26s} {"duty":>5s} {"powered":>9s} {"coast@stop":>10s} '
        f'{"resid":>11s} {"decel":>10s} {"lever":>7s} {"span":>8s}'
    )
    for r in rows:
        print(
            f'{r["file"][:26]:26s} {r["duty"] / 10:5.1f} {r["powered"]:9.1f} {r["coast_at_stop"]:10.1f} '
            f'{r["residual_permille"]:+8.1f}o/oo {r["slope_ehz_per_s"]:10.1f} {r["lever_us"]:7.0f} '
            f'{r["span_us"]:8d}'
        )
    if a.calibrate:
        res = [r["residual_permille"] for r in rows]
        duties = {r["duty"] for r in rows}
        elfs = {r["elf"] for r in rows}
        print(f'\nn={len(res)} runs, duties={sorted(d / 10 for d in duties)}, images={sorted(elfs)}')
        if len(duties) > 1:
            print("NOTE: more than one rung in this set -- a band calibrated across rungs means nothing")
        # The image is checked, not promised: a band pooled across images is not
        # one instrument's repeatability (found by the independent review of E155).
        if len(elfs) > 1 or "unknown" in elfs:
            print("NOTE: more than one image, or an unidentified one -- this is not one instrument's scatter")
        pw = [r["powered"] for r in rows]
        cs = [r["coast_at_stop"] for r in rows]
        print(f'powered eHz:  mean={statistics.fmean(pw):.2f}  spread={max(pw) - min(pw):.2f}')
        print(f'coast@stop:   mean={statistics.fmean(cs):.2f}  spread={max(cs) - min(cs):.2f}')
        print(f'residual per mille: mean={statistics.fmean(res):+.2f}  min={min(res):+.2f}  max={max(res):+.2f}')
        if len(res) > 1:
            print(f'                    sd={statistics.stdev(res):.2f}  range={max(res) - min(res):.2f}')
        if len(res) < 5:
            print("fewer than 5 runs: this is not yet a calibration")
    return 0


if __name__ == "__main__":
    sys.exit(main())
