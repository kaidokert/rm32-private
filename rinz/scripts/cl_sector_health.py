#!/usr/bin/env python3
"""Per-physical-sector lock health from saved cl_alpha_tune dumps (offline, no hardware).

The aggregate lockS% hides WHERE the misses live. This reads the `cl i=.. phys=.. zc=..
coast=..` logs from a logs/alpha_<ts>/ dir and breaks down, per physical sector and per
alpha level: how often it coasted (missed the ZC) and how much the in-window ZC% jitters.
A sector that coasts or scatters far more than the others is the one causing the audible
clips -- e.g. the phase-A float sectors (s2, s5) per the known phase-A sense anomaly.

    python scripts/cl_sector_health.py logs/alpha_20260623_083413
    python scripts/cl_sector_health.py logs/alpha_20260623_083413 --alpha 0.7 0.8
"""

from __future__ import annotations

import argparse
import re
import statistics
from collections import defaultdict
from pathlib import Path

_CL_RE = re.compile(r"\bcl i=(\d+)\s+phys=(\d+)\s+zc=(-?\d+)\s+\w+=(\d+)")
_ALPHA_RE = re.compile(r"\balpha=(\d+)")


def analyze_file(text: str):
    """-> (alpha_x1000, {phys: [(zc, coast)]})."""
    am = _ALPHA_RE.search(text)
    alpha = int(am.group(1)) if am else None
    by = defaultdict(list)
    for m in _CL_RE.finditer(text):
        phys, zc, coast = int(m[2]), int(m[3]), int(m[4])
        by[phys].append((zc, coast))
    return alpha, by


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir", type=Path)
    ap.add_argument("--alpha", type=float, nargs="*", help="only these alpha levels (e.g. 0.7 0.8)")
    args = ap.parse_args()

    want = {int(round(a * 1000)) for a in args.alpha} if args.alpha else None
    # alpha_x1000 -> phys -> list[(zc,coast)]
    agg: dict[int, dict[int, list]] = defaultdict(lambda: defaultdict(list))
    for f in sorted(args.dir.glob("*.log")):
        alpha, by = analyze_file(f.read_text(encoding="ascii", errors="replace"))
        if alpha is None:
            continue
        if want and alpha not in want:
            continue
        for phys, rows in by.items():
            agg[alpha][phys].extend(rows)

    if not agg:
        raise SystemExit("no cl logs found (or none matched --alpha)")

    for alpha in sorted(agg):
        print(f"\n=== alpha {alpha / 1000:.2f} ===")
        print(f"  {'sec':>4} {'n':>4} {'coast%':>7} {'zc%_med':>8} {'zc%_iqr':>8}  note")
        tot_n = tot_coast = 0
        for phys in range(6):
            rows = agg[alpha].get(phys, [])
            if not rows:
                continue
            n = len(rows)
            coast = sum(1 for _zc, c in rows if c == 1)
            tot_n += n
            tot_coast += coast
            zcs = [zc for zc, c in rows if c == 0 and 0 <= zc <= 100]  # in-window only
            zc_med = statistics.median(zcs) if zcs else float("nan")
            if len(zcs) >= 4:
                q = statistics.quantiles(zcs, n=4)
                zc_iqr = q[2] - q[0]
            else:
                zc_iqr = float("nan")
            note = ""
            if phys in (2, 5):
                note = "phase-A float"
            print(f"  s{phys:>3} {n:>4} {coast / n * 100:6.0f}% "
                  f"{zc_med:8.0f} {zc_iqr:8.0f}  {note}")
        if tot_n:
            print(f"  {'all':>4} {tot_n:>4} {tot_coast / tot_n * 100:6.0f}%  "
                  f"(lock {100 - tot_coast / tot_n * 100:.0f}%)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
