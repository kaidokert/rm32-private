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
prev_tk = None
with open(args.csv_path, newline="") as fh:
    r = csv.DictReader(fh)
    for row in r:
        # BATCH-DECIMATION gap detect (clone 50-on/50-off trace mode):
        # a >1 ms jump in the 20 kHz tenkhz counter marks a skipped
        # batch — insert a sentinel so the rolling-mean history resets
        # instead of referencing the previous batch's tail.
        tk = row.get("tenkhz")
        if tk is not None:
            tk = int(float(tk))
            if prev_tk is not None and ((tk - prev_tk) & 0xFFFF) > 20:
                for lst in by_rung.values():
                    if lst and lst[-1] is not None:
                        lst.append(None)
            prev_tk = tk
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
            # zctrace_capture CSVs carry zt_us (already converted);
            # the sweep-am32 CSVs carry zt_ticks (0.5 us each).
            if "zt_us" in row:
                per = float(row["zt_us"])
            else:
                per = float(row["zt_ticks"]) * 0.5
            rung = int(row.get(args.rung_col, 0) or 0)
        if 20 <= per <= 20000:
            by_rung[rung].append(per)

print(f"{'rung':>4} {'n':>7} {'med_us':>7} {'f_Hz':>6} "
      f"{'>12.5%/1k':>10} {'>25%/1k':>8} {'worst':>7}")
for rung in sorted(by_rung):
    ps = by_rung[rung]
    vals = [p for p in ps if p is not None]
    if len(vals) < 100:
        print(f"{rung:>4} {len(vals):>7}  (too few)")
        continue
    med = sorted(vals)[len(vals) // 2]
    hist = []
    e125 = e250 = 0
    worst = 0.0
    for p in ps:
        if p is None:
            hist.clear()  # batch gap: don't span the skipped half
            continue
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
    n = len(vals)
    print(f"{rung:>4} {n:>7} {med:>7.0f} {1e6 / (6 * med):>6.0f} "
          f"{1e3 * e125 / n:>10.2f} {1e3 * e250 / n:>8.2f} "
          f"+{100 * worst:>5.0f}%")
