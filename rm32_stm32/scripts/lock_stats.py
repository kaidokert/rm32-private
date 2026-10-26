#!/usr/bin/env python3
"""Per-segment lock summary of binz_profile captures (CSV).

    python scripts/lock_stats.py captures/l431/profile_cab-*.csv

For each run and each held throttle segment (after a settle time): mean
eHz, eHz spread (p10..p90), mean mA, the share of samples in the
open-loop startup routine (old_routine=1) and whether the duty was ever
cut below the commanded level (desync kicks). "held" = closed loop for
every settled sample with no duty cut.
"""

from __future__ import annotations

import argparse
import csv
import pathlib
import statistics




def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("csv", nargs="+")
    ap.add_argument("--settle", type=float, default=1.0, help="seconds skipped per segment")
    a = ap.parse_args()
    for f in a.csv:
        rows = list(csv.DictReader(open(f)))
        held = [r for r in rows if int(r["duty"]) > 0 or float(r["ehz"]) > 0]
        if not held:
            print(f"{pathlib.Path(f).name}: no drive")
            continue
        # Segment by commanded throttle: the max duty seen in a contiguous
        # run of nonzero-duty rows bounded by large steps up.
        out = []
        t0 = float(held[0]["t_s"])
        cmd = int(held[0]["duty"])
        segs, cur = [], []
        for r in held:
            d = int(r["duty"])
            if d > cmd * 1.08 and cur:  # throttle stepped up
                segs.append((cmd, cur))
                cur, cmd = [], d
            cur.append(r)
        if cur:
            # The scripted walk-down before the stop is not a cut: end the
            # last segment at its last sample at the commanded duty.
            last = max(i for i, r in enumerate(cur) if int(r["duty"]) >= cmd)
            segs.append((cmd, cur[: last + 1]))
        for cmd, rs in segs:
            if int(rs[0]["duty"]) == 0:
                continue
            ts = float(rs[0]["t_s"])
            st = [r for r in rs if float(r["t_s"]) >= ts + a.settle and int(r["duty"]) > 0]
            if not st:
                continue
            e = sorted(float(r["ehz"]) for r in st)
            ma = statistics.mean(float(r["ma"]) for r in st)
            orr = sum(int(r["old_routine"]) for r in st) / len(st)
            cut = sum(1 for r in st if int(r["duty"]) < cmd * 0.9)
            dsy = max(int(r.get("dsy4", 0) or 0) for r in st)
            p = lambda q: e[min(len(e) - 1, int(q * len(e)))]
            ok = "HELD" if orr == 0 and cut == 0 else "lost"
            out.append(f"duty {cmd:4d}: {statistics.mean(e):7.1f} eHz (p10 {p(.1):.0f} p90 {p(.9):.0f}) "
                       f"{ma:5.0f} mA  open-loop {orr:4.0%}  cuts {cut:2d}  dsy4 {dsy}  {ok}")
        print(pathlib.Path(f).name)
        for line in out:
            print("   " + line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
