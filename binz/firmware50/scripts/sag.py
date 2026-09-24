#!/usr/bin/env python3
"""Read a `sag-capture` dump and report the sharp-sag guard's own margin.

The firmware records one row per block the guard judged (`src/sagtrace.rs`):

    SAGSNAP len=<n> total=<n> frozen=<0|1> num=95 den=100 streak_to_latch=3
    SAGROW <at> <bus_mean> <vref_mean> <filt_bus> <filt_vref> <streak> <step> <duty> <since_com_us>
    ...
    SAGEND

Raw quantities only -- the firmware does no division -- so the guard's own
comparison is reproduced here exactly as it runs:

    low  <=>  bus_mean * filt_vref * den  <  filt_bus * vref_mean * num

and the margin is reported as the ratio of the two sides in per mille, where
1000 is exactly on the line and larger is safer:

    margin = 1000 * (bus_mean * filt_vref * den) / (filt_bus * vref_mean * num)

**What this is not.** `bus_min` in the run report is the minimum of a *single
raw scan* over the whole run, and the report's `filt_bus` is the filter's value
at the *end* of the run. Neither is what the guard compares, and both have been
quoted at it before (campaign 8). This script only uses the quartet the guard
itself used, as recorded at the instant it judged.

Coverage: the ring is bounded (512 blocks, about 1.2 s) and **freezes on the
trip**, so a frozen dump is the window that led to the fault and an unfrozen
one is the tail of a healthy run. `total` says how many blocks were judged;
`len` how many survived.

Usage:
    python scripts/sag.py <capture.txt> [--csv rows.csv] [--worst 20]
"""
from __future__ import annotations

import argparse
import collections
import pathlib
import statistics
import sys

Row = collections.namedtuple(
    "Row", "at bus vref filt_bus filt_vref streak step duty since_com"
)


def parse(path: pathlib.Path) -> tuple[dict, list[Row]]:
    snap, rows = {}, []
    for line in path.read_text(errors="replace").splitlines():
        line = line.strip()
        if line.startswith("SAGSNAP"):
            for kv in line.split()[1:]:
                if "=" in kv:
                    k, v = kv.split("=", 1)
                    snap[k] = int(v)
        elif line.startswith("SAGROW "):
            f = line.split()
            if len(f) == 10:
                rows.append(Row(*(int(x) for x in f[1:])))
    return snap, rows


def margin_permille(r: Row, num: int, den: int) -> float:
    """The guard's own cross-product, as a ratio; 1000.0 is exactly the line."""
    lhs = r.bus * r.filt_vref * den
    rhs = r.filt_bus * r.vref * num
    return 1000.0 * lhs / rhs if rhs else float("inf")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("capture")
    ap.add_argument("--csv")
    ap.add_argument("--worst", type=int, default=10, help="how many tightest blocks to list")
    a = ap.parse_args()
    snap, rows = parse(pathlib.Path(a.capture))
    if not rows:
        print("no SAGROW rows found", file=sys.stderr)
        return 1
    num = snap.get("num", 95)
    den = snap.get("den", 100)
    latch = snap.get("streak_to_latch", 3)
    dropped = max(snap.get("total", len(rows)) - snap.get("len", len(rows)), 0)
    print(
        f'blocks judged={snap.get("total")} kept={snap.get("len")} overwritten={dropped} '
        f'frozen={snap.get("frozen")} fraction={num}/{den} streak_to_latch={latch}'
    )

    m = [margin_permille(r, num, den) for r in rows]
    low = [r for r, x in zip(rows, m) if x < 1000.0]
    print(
        f"margin per mille (1000 = exactly on the line): min={min(m):.1f} "
        f"p01={statistics.quantiles(m, n=100)[0]:.1f} p50={statistics.median(m):.1f} "
        f"max={max(m):.1f}"
    )
    print(f"low blocks (the guard's own test): {len(low)} of {len(rows)}")
    streaks = [r.streak for r in rows]
    print(f"streak held: max={max(streaks)} (latches at {latch}); rows with streak>0: {sum(1 for s in streaks if s)}")

    # Where in the commutation cycle the tightest blocks fall: a sag that is
    # phase-locked to switching looks different from one that is not.
    if rows:
        order = sorted(range(len(rows)), key=lambda i: m[i])
        print(f"\ntightest {min(a.worst, len(rows))} blocks:")
        print(f'{"margin":>9} {"bus":>6} {"vref":>6} {"filt_bus":>9} {"filt_vref":>9} {"streak":>7} {"step":>5} {"duty":>6} {"since_com":>10}')
        for i in order[: a.worst]:
            r = rows[i]
            print(
                f"{m[i]:9.1f} {r.bus:6d} {r.vref:6d} {r.filt_bus:9d} {r.filt_vref:9d} "
                f"{r.streak:7d} {r.step:5d} {r.duty:6d} {r.since_com:10d}"
            )
        tight = [rows[i] for i in order[: max(a.worst, 20)]]
        if tight:
            sc = [r.since_com for r in tight]
            print(
                f"\nsince the last accepted crossing, in the tightest blocks: "
                f"min={min(sc)} p50={statistics.median(sc):.0f} max={max(sc)} µs"
            )
            steps = collections.Counter(r.step for r in tight)
            print(f"sector of the tightest blocks: {dict(sorted(steps.items()))}")

    if a.csv:
        with open(a.csv, "w", newline="") as fh:
            fh.write("at,bus_mean,vref_mean,filt_bus,filt_vref,streak,step,duty_tenths,since_com_us,margin_permille\n")
            for r, x in zip(rows, m):
                fh.write(f"{r.at},{r.bus},{r.vref},{r.filt_bus},{r.filt_vref},{r.streak},{r.step},{r.duty},{r.since_com},{x:.2f}\n")
        print(f"\nrows written to {a.csv}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
