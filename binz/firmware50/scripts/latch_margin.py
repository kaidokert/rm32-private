"""Margin-to-latch for the fast bus-sag guard, from a run's recorded fields (E349).

The guard and the 950-per-mille raw depth bin test the **same line**:

    FastBusSag::observe   bus_mean * filt_vref * 100 < filt_bus * vref_mean * 95
    BusDepth::observe[1]  bus_raw  * filt_vref * 1000 < filt_bus * vref     * 950

Same fraction, same reference. The only difference is that the guard is fed the
**8-tap moving mean** (`RAIL_MEAN_LEN = 8`) while the bin is fed the raw sample,
and `observe` runs **once per ADC scan** -- so `SAG_STREAK = 3` is three
consecutive *scans*, a 0.303 ms window at 9901 Hz, not three 100-scan blocks.

That makes the latch boundary computable. With `vref ~ filt_vref`, the line in
bus codes is `L = 0.95 * filt_bus`; ordinary samples sit `h = filt_bus - L`
above it (`filt_bus` being a 207 ms EWMA of the bus, so the run's typical mean is
`filt_bus` by construction); a dip sample sits `d` below. An 8-tap window holding
`k` dip samples is low iff

    k*d > (8 - k)*h      =>      k_min = floor(8h / (d + h)) + 1

and a contiguous run of `R` raw dip samples drives three *consecutive* low means
iff `R >= R_need`, where

    R_need = max(k_min, 2*k_min - 6)

which `self_check()` brute-forces rather than trusting the algebra.

**Margin-to-latch = R_have - R_need.** Negative is margin; >= 0 latches.

## The assumption this rests on, and why the tool says so on every line

`d` is taken from `bus_min`, which is the minimum over **every scan of the whole
run** (`src/run/states.rs:332`), while `R_have` is `raw1_run`, the longest run
below 950, accumulated separately. **Nothing in the report pairs them** -- they
may be two different events seconds apart. Using the global minimum as the depth
of every dip sample over-estimates `d`, which under-estimates `k_min` and
`R_need`, so this tool is biased **toward predicting a latch**: a computed margin
is a lower bound on the true margin. It reproduces 14 of 15 runs on image
FE927FA3 and its single miss (`q525-advref_03`: predicted latch, passed) is
exactly what the unpaired-extrema assumption predicts as a failure.

Per-scan pairing is what `sagtrace` supplies and this cannot; treat a margin at
or near 0 as "unresolved", never as a verdict.

    python scripts/latch_margin.py captures/2026-09-25/q5*-advref*.txt
    python scripts/latch_margin.py --self-check
"""
from __future__ import annotations

import argparse
import glob
import math
import re
import sys

WINDOW = 8          # RAIL_MEAN_LEN
STREAK = 3          # SAG_STREAK
NUM, DEN = 95, 100  # SAG_NUM / SAG_DEN
SCAN_HZ = 9901.0

# Below this the rail was not powered (E346); a margin computed from it is
# meaningless, so refuse the row rather than print a number.
MIN_PLAUSIBLE_FILT = 600


def r_need(k_min: int) -> int:
    """Contiguous raw dip samples needed for `STREAK` consecutive low means."""
    # count(overlap >= k) = WINDOW + 1 + R - 2k, so STREAK consecutive lows
    # need R >= 2k - (WINDOW + 1 - STREAK). Getting this constant wrong is
    # what `self_check` caught on its first run (11 instead of 6, which
    # silently shifted k_min >= 7 and changed two rows of the table).
    return max(k_min, 2 * k_min - (WINDOW + 1 - STREAK))


def _r_need_bruteforce(k_min: int) -> int | None:
    """The same quantity by simulation, for `self_check`."""
    for r in range(1, 4 * WINDOW):
        best = cur = 0
        for i in range(-1, r + WINDOW + 2):
            overlap = len(set(range(i - WINDOW + 1, i + 1)) & set(range(0, r)))
            cur = cur + 1 if overlap >= k_min else 0
            best = max(best, cur)
        if best >= STREAK:
            return r
    return None


def self_check() -> int:
    """Refuse to be trusted unless the closed form matches simulation."""
    bad = []
    for k in range(1, WINDOW + 1):
        f, b = r_need(k), _r_need_bruteforce(k)
        print(f"  k_min={k}: closed form {f}, simulated {b}"
              + ("   <-- MISMATCH" if f != b else ""))
        if f != b:
            bad.append(k)
    # And the arithmetic must reproduce the one latch we have.
    m = margin(filt_bus=1196, bus_min=1058, raw_run=6)
    if m is None or m["margin"] < 0:
        print(f"  q550-advref_02 (the observed latch) computes margin "
              f"{m and m['margin']} -- must be >= 0")
        bad.append("latch")
    if bad:
        print(f"SELF-CHECK FAILED: {bad}")
        return 1
    print("self-check OK: closed form == simulation, and the observed latch "
          "computes as a latch")
    return 0


def margin(filt_bus: int, bus_min: int, raw_run: int) -> dict | None:
    if filt_bus < MIN_PLAUSIBLE_FILT:
        return None
    line = NUM * filt_bus / DEN
    head = filt_bus - line
    depth = line - bus_min
    if depth <= 0 or head <= 0:
        return None
    k_min = math.floor(WINDOW * head / (depth + head)) + 1
    need = r_need(k_min)
    return {"line": line, "depth": depth, "head": head, "k_min": k_min,
            "need": need, "have": raw_run, "margin": raw_run - need}


FIELDS = ("target_duty_tenths", "reason", "filt_bus", "bus_min", "raw1_run")


def parse(path: str) -> dict | None:
    text = open(path, errors="replace").read()
    out = {}
    for k in FIELDS:
        m = re.search(rf"\b{k}=(-?\d+)", text)
        if not m:
            return None
        out[k] = int(m.group(1))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("captures", nargs="*")
    ap.add_argument("--self-check", action="store_true")
    args = ap.parse_args()
    if args.self_check or not args.captures:
        return self_check()

    paths = sorted({p for pat in args.captures for p in glob.glob(pat)})
    print(f"latch window = {STREAK}/{SCAN_HZ:.0f} Hz = "
          f"{1000 * STREAK / SCAN_HZ:.3f} ms; {WINDOW}-tap mean\n")
    print(f"{'capture':26s}{'rung':>5}{'rsn':>5}{'depth':>7}{'k_min':>6}"
          f"{'need':>6}{'have':>6}{'margin':>8}  reading")
    refused = 0
    for p in paths:
        r = parse(p)
        name = p.replace("\\", "/").split("/")[-1]
        if r is None:
            print(f"{name[:26]:26s}  REFUSED: fields missing")
            refused += 1
            continue
        m = margin(r["filt_bus"], r["bus_min"], r["raw1_run"])
        if m is None:
            print(f"{name[:26]:26s}{r['target_duty_tenths']:>5}"
                  f"{r['reason']:>5}  REFUSED: rail out of band "
                  f"(filt_bus={r['filt_bus']}) -- not a powered run")
            refused += 1
            continue
        # The tool's own resolution limit, stated rather than hidden: its only
        # wrong call (`q525-advref_03`) computed exactly +0 and passed, so a
        # margin within one scan of the boundary is UNRESOLVED, not a verdict.
        if abs(m["margin"]) <= 1:
            reading = "UNRESOLVED (within one scan -- needs per-scan pairing)"
        elif m["margin"] > 1:
            reading = "LATCHES"
        else:
            reading = "margin"
        print(f"{name[:26]:26s}{r['target_duty_tenths']:>5}{r['reason']:>5}"
              f"{m['depth']:>7.1f}{m['k_min']:>6}{m['need']:>6}{m['have']:>6}"
              f"{m['margin']:>+8}  {reading}")
    if refused:
        print(f"\n{refused} capture(s) refused rather than scored.")
    print("\nA margin here is a LOWER BOUND: depth comes from the run's global "
          "`bus_min`, which need not belong to the longest run, and that bias "
          "favours predicting a latch. Near 0, read it as unresolved.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
