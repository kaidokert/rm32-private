#!/usr/bin/env python3
"""Per-rung rotor-period tail report from a zctsweep CSV (AM32 side)
or an MZT capture CSV (minz side) — the apples-to-apples metric:

Per rung: n, median period (us), >12.5% and >25% excursion rates
vs the 6-deep rolling mean, and the worst excursion — the EXACT
criterion owl_report --excursions / plot_excursions use.

AM32 zt is in 0.5 us ticks (INTERVAL_TIMER); old==1 records
(old_routine / polling mode) are excluded — running mode only.

Usage:
    python scripts/zct_sweep_report.py captures/zctsweep_am32full.csv
    python scripts/zct_sweep_report.py captures/mzt_full.csv --minz
"""

import argparse
import csv
import sys
from collections import defaultdict

ap = argparse.ArgumentParser()
ap.add_argument("csv_path")
ap.add_argument("--minz", action="store_true",
                help="MZT csv (period_us column, amp column from duty)")
ap.add_argument("--rung-col", default="rung_pct")
args = ap.parse_args()

sys.stdout.reconfigure(errors="replace")

by_rung = defaultdict(list)
with open(args.csv_path, newline="") as fh:
    r = csv.DictReader(fh)
    for row in r:
        if args.minz:
            # ALIGNED QUANTITY: raw_iv_us = ZC-stamp to ZC-stamp
            # (AM32's zt twin). period_us is commutation-paced and
            # the kick/rescue steppers hold it on schedule even when
            # the rotor stretches (the EXC_COUNT=0 blindness).
            per = float(row.get("raw_iv_us", 0) or 0)
            if per == 0:
                continue
            rung = int(row.get(args.rung_col, 0) or 0)
        else:
            if int(row["old"]):
                continue  # old_routine / polling mode excluded
            per = float(row["zt_ticks"]) * 0.5  # 0.5 us ticks -> us
            rung = int(row[args.rung_col])
        if 20 <= per <= 20000:
            by_rung[rung].append(per)

print(f"{'rung':>4} {'n':>7} {'med_us':>7} {'f_Hz':>6} "
      f"{'>12.5%/1k':>10} {'>25%/1k':>8} {'worst':>7}")
for rung in sorted(by_rung):
    ps = by_rung[rung]
    if len(ps) < 100:
        print(f"{rung:>4} {len(ps):>7}  (too few)")
        continue
    med = sorted(ps)[len(ps) // 2]
    hist = []
    e125 = e250 = 0
    worst = 0.0
    for p in ps:
        if len(hist) == 6:
            ref = sum(hist) / 6
            if ref > 0 and p > ref:
                exc = p / ref - 1.0
                worst = max(worst, exc)
                if exc > 0.125:
                    e125 += 1
                if exc > 0.25:
                    e250 += 1
        hist.append(p)
        if len(hist) > 6:
            hist.pop(0)
    n = len(ps)
    print(f"{rung:>4} {n:>7} {med:>7.0f} {1e6 / (6 * med):>6.0f} "
          f"{1e3 * e125 / n:>10.2f} {1e3 * e250 / n:>8.2f} "
          f"+{100 * worst:>5.0f}%")
