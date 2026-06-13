#!/usr/bin/env python3
"""Per-sector-type ZC statistics across many captures.

Walks raw capture logs (e.g. from zc_scan / zc_chase output dirs), re-runs the
ZC analysis, and aggregates per (hz, sector type s0..s5):

  - status counts (zc / early / none)
  - measured in-window crossing position (pct of window)
  - estimated crossing position for ALL sectors, in window units, by linear
    extrapolation of (d_start, d_end): est = d_start / (d_start - d_end).
    0..1 is in-window; <0 crossed before the window opened; >1 crosses after
    it closes. Rough (the BEMF is a sine, not a line) but good to ~half a
    window within +/-1 window of the boundary.

From the per-type mean offsets it prints the global commutation-phase shift
(in 15-degree logical sectors) that would center each type, and the median —
if the per-type shifts agree, ONE table rotation fixes all six; if they
disagree, the offset is not global and a rotation alone cannot align them.

Example:
    python scripts/zc_sector_stats.py logs/chase_20260612_073151 logs/chase_20260612_073315
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path
import statistics
import sys

from scope_common import analyze_zero_crossings, parse_capture


def estimate_offset(d_start: float | None, d_end: float | None) -> float | None:
    """Linear-extrapolated crossing position in window units (0..1 in-window)."""
    if d_start is None or d_end is None or d_start == d_end:
        return None
    return d_start / (d_start - d_end)


def collect(paths: list[Path]) -> list[tuple[Path, object]]:
    logs: list[Path] = []
    for p in paths:
        if p.is_dir():
            logs += sorted(p.rglob("*_raw.log"))
        elif p.is_file():
            logs.append(p)
    out = []
    for log in logs:
        try:
            cap = parse_capture(log.read_text(encoding="ascii", errors="replace"))
            sectors, _, _ = analyze_zero_crossings(cap)
        except Exception as exc:
            print(f"skip {log}: {type(exc).__name__}: {exc}", file=sys.stderr)
            continue
        out.append((log, (cap, sectors)))
    return out


def fmt(v: float | None, spec: str = "6.2f") -> str:
    return f"{v:{spec}}" if v is not None else "     -"


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("paths", nargs="+", type=Path, help="capture dirs or *_raw.log files")
    parser.add_argument(
        "--est-limit",
        type=float,
        default=3.0,
        help="discard extrapolated offsets beyond +/- this many windows (default 3)",
    )
    parser.add_argument(
        "--per-rev",
        action="store_true",
        help="also print a rev-by-rev matrix of median offsets (drift check: "
        "an electrical-angle-locked pattern repeats identically every row; "
        "a slow rotor oscillation walks across rows)",
    )
    args = parser.parse_args()

    captures = collect(args.paths)
    if not captures:
        raise SystemExit("no parsable captures found")

    # (hz, sector_type) -> aggregates
    counts: dict[tuple[int, int], dict[str, int]] = defaultdict(lambda: {"zc": 0, "early": 0, "none": 0})
    in_window_pcts: dict[tuple[int, int], list[float]] = defaultdict(list)
    est_offsets: dict[tuple[int, int], list[float]] = defaultdict(list)
    # (hz, rev, sector_type) -> est offsets, for the --per-rev drift matrix
    per_rev: dict[tuple[int, int, int], list[float]] = defaultdict(list)
    phase_of: dict[int, str] = {}
    n_caps: dict[int, int] = defaultdict(int)

    for _log, (cap, sectors) in captures:
        hz = int(float(cap.debug.get("hz", "0") or 0))
        n_caps[hz] += 1
        for sec in sectors:
            st = sec.index % 6
            rev = sec.index // 6
            key = (hz, st)
            phase_of[st] = sec.phase
            counts[key][sec.status] += 1
            if sec.status == "zc" and sec.zc_pct is not None:
                in_window_pcts[key].append(sec.zc_pct)
            est = (
                sec.zc_pct / 100.0
                if sec.status == "zc" and sec.zc_pct is not None
                else estimate_offset(sec.d_start, sec.d_end)
            )
            if est is not None and abs(est) <= args.est_limit:
                est_offsets[key].append(est)
                per_rev[(hz, rev, st)].append(est)

    for hz in sorted(n_caps):
        print(f"\nhz={hz}  ({n_caps[hz]} captures)")
        print("type phase    n    zc early  none | in-window pct      | est crossing (window units)   | shift to center")
        print("                                  | mean   sd    n     | mean    sd     median   n     | (15-deg logical sectors)")
        shifts: list[float] = []
        for st in range(6):
            key = (hz, st)
            c = counts[key]
            n = c["zc"] + c["early"] + c["none"]
            if n == 0:
                continue
            pcts = in_window_pcts[key]
            p_mean = statistics.mean(pcts) if pcts else None
            p_sd = statistics.stdev(pcts) if len(pcts) > 1 else None
            ests = est_offsets[key]
            e_mean = statistics.mean(ests) if ests else None
            e_sd = statistics.stdev(ests) if len(ests) > 1 else None
            e_med = statistics.median(ests) if ests else None
            # Shift (in physical-sector units) that puts the median crossing at
            # window center; 1 physical sector = 4 logical sectors of 15 deg.
            shift = (e_med - 0.5) * 4 if e_med is not None else None
            if shift is not None:
                shifts.append(shift)
            print(
                f"s{st}   {phase_of.get(st, '?')}    {n:4d} {c['zc']:5d} {c['early']:5d} {c['none']:5d} | "
                f"{fmt(p_mean, '5.1f')} {fmt(p_sd, '5.1f')} {len(pcts):4d}  | "
                f"{fmt(e_mean)} {fmt(e_sd)} {fmt(e_med)} {len(ests):4d}  | "
                f"{fmt(shift, '+5.1f')}"
            )
        if shifts:
            med = statistics.median(shifts)
            spread = max(shifts) - min(shifts)
            print(f"\n  shift per type: median {med:+.1f} logical sectors, spread {spread:.1f}")
            if spread <= 1.5:
                print(
                    f"  -> offsets are consistent: rotating the drive tables by "
                    f"{round(med):+d} logical sectors should center all types."
                )
            else:
                print(
                    "  -> offsets differ structurally between sector types; a global"
                    " rotation cannot align all of them (check per-type rows)."
                )

        if args.per_rev:
            revs = sorted({r for (h, r, _st) in per_rev if h == hz})
            if revs:
                print("\n  per-rev median est (rows = revolution within capture):")
                print("  rev   " + "".join(f"   s{st}   " for st in range(6)))
                for r in revs:
                    cells = []
                    for st in range(6):
                        vals = per_rev.get((hz, r, st), [])
                        cells.append(
                            f" {statistics.median(vals):+6.2f}" if vals else "      -"
                        )
                    n_min = min(
                        (len(per_rev.get((hz, r, st), [])) for st in range(6)),
                        default=0,
                    )
                    print(f"  {r:3d}  " + " ".join(cells) + f"   (n>={n_min})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
