#!/usr/bin/env python3
"""Per-segment summary of binz_profile.py captures (gc refactor S1 bands).

    python scripts/gc_summary.py captures/l431/profile_s00-r1-ladder_*.json
    python scripts/gc_summary.py --bands out.json captures/*/profile_s00-*.json

For each capture and each segment: duty, eHz and mA averaged over the
segment's settled tail (last `--tail` seconds, default 2 s, never more than
half the segment), plus whole-run bus min/max mV, desync count (dsy4 field
advances), old_routine samples while driven, guard kill. Segment times come
from the host's segment starts relative to the recorder arm (JSON), mapped
onto the recorder's sample clock.

--bands groups captures by (board, test name without the run timestamp,
segment index) and writes min/max per quantity across runs: the S1
acceptance bands are min..max widened by --widen (default 5 %).
Refuses captures with no CSV, a kill, or a segment with no samples.
"""

from __future__ import annotations

import argparse
import csv
import json
import pathlib
import re
import statistics
import sys


def load(js: pathlib.Path):
    meta = json.loads(js.read_text())
    csvp = js.with_suffix(".csv")
    if not csvp.exists():
        raise SystemExit(f"{js}: no CSV")
    rows = list(csv.DictReader(csvp.open()))
    return meta, rows


def summarize(js: pathlib.Path, tail: float):
    meta, rows = load(js)
    t = [float(r["t_s"]) for r in rows]
    segs = []
    for i, s in enumerate(meta["segments"]):
        start = s["t_start"]
        end = meta["segments"][i + 1]["t_start"] if i + 1 < len(meta["segments"]) else meta["t_stop"]
        lo = max(start, end - min(tail, (end - start) / 2))
        sel = [r for r, ti in zip(rows, t) if lo <= ti < end]
        if not sel:
            raise SystemExit(f"{js}: segment {i} ({s['throttle']} %) has no samples")
        segs.append({
            "throttle": s["throttle"],
            "duty": statistics.mean(int(r["duty"]) for r in sel),
            "ehz": statistics.mean(float(r["ehz"]) for r in sel),
            "ma": statistics.mean(int(r["ma"]) for r in sel),
        })
    driven = [r for r in rows if int(r["duty"]) > 0]
    dsy = [int(r["dsy4"]) for r in rows]
    # dsy4 is a 4-bit wrapping counter of desync events.
    dsy_n = sum((b - a) % 16 for a, b in zip(dsy, dsy[1:]))
    mv = [int(r["mv"]) for r in rows if int(r["mv"]) > 0]
    return {
        "board": meta.get("board", "binz"),
        "test": re.sub(r"_\d{8}_\d{6}$", "", js.stem.removeprefix("profile_")),
        "file": js.name,
        "kill": meta.get("kill") or meta.get("kill_line"),
        "segments": segs,
        "dsy": dsy_n,
        "old_routine": sum(int(r["old_routine"]) for r in driven),
        "mv_min": min(mv) if mv else 0,
        "mv_max": max(mv) if mv else 0,
    }


def check(out, bands) -> int:
    """A capture passes when it has no kill, no desync, the band's segment
    count, and every segment's duty/eHz/mA inside [lo, hi]. A capture whose
    test has no band is a failure (never silently skipped)."""
    fails = 0
    for s in out:
        key = f"{s['board']}/{re.sub(r'^s\d+-', 's00-', s['test'])}"
        b = bands.get(key)
        why = []
        if b is None:
            why.append(f"no band {key}")
        else:
            if s["kill"]:
                why.append(f"kill: {s['kill']}")
            if s["dsy"] > b["dsy_max"]:
                why.append(f"dsy={s['dsy']}")
            if len(s["segments"]) != len(b["segments"]):
                why.append(f"{len(s['segments'])} segments, band has {len(b['segments'])}")
            else:
                for g, bg in zip(s["segments"], b["segments"]):
                    for q in ("duty", "ehz", "ma"):
                        lo, hi = bg[q]
                        if not lo <= g[q] <= hi:
                            why.append(f"{g['throttle']}% {q}={g[q]:.1f} outside {lo}..{hi}")
        print(f"{'PASS' if not why else 'FAIL'}  {s['board']:5} {s['file']}"
              + "".join(f"\n        {w}" for w in why))
        fails += bool(why)
    print(f"band check: {len(out) - fails}/{len(out)} pass")
    return 1 if fails else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("json", nargs="+", type=pathlib.Path)
    ap.add_argument("--tail", type=float, default=2.0)
    ap.add_argument("--bands", type=pathlib.Path)
    ap.add_argument("--widen", type=float, default=0.05)
    ap.add_argument("--check", type=pathlib.Path,
                    help="verify captures against a saved bands JSON; exit 1 on any miss")
    a = ap.parse_args()
    out = [summarize(j, a.tail) for j in a.json]
    if a.check:
        return check(out, json.loads(a.check.read_text()))
    for s in out:
        print(f"{s['board']:5} {s['file']}: dsy={s['dsy']} old={s['old_routine']} "
              f"bus {s['mv_min']}..{s['mv_max']} mV kill={s['kill'] or 0}")
        for g in s["segments"]:
            print(f"      {g['throttle']:3d}%  duty {g['duty']:7.1f}  {g['ehz']:7.1f} eHz  {g['ma']:6.0f} mA")
    if a.bands:
        bad = [s["file"] for s in out if s["kill"]]
        if bad:
            raise SystemExit(f"refusing bands: killed runs {bad}")
        groups: dict[tuple, list] = {}
        for s in out:
            groups.setdefault((s["board"], s["test"]), []).append(s)
        bands = {}
        for (board, test), runs in sorted(groups.items()):
            nseg = {len(r["segments"]) for r in runs}
            if len(nseg) != 1:
                raise SystemExit(f"{board}/{test}: runs disagree on segment count {nseg}")
            segb = []
            for k in range(nseg.pop()):
                b = {"throttle": runs[0]["segments"][k]["throttle"]}
                for q in ("duty", "ehz", "ma"):
                    v = [r["segments"][k][q] for r in runs]
                    lo, hi = min(v), max(v)
                    b[q] = [round(lo - a.widen * abs(lo), 1), round(hi + a.widen * abs(hi), 1)]
                    b[q + "_runs"] = [round(x, 1) for x in v]
                segb.append(b)
            bands[f"{board}/{test}"] = {
                "runs": [r["file"] for r in runs],
                "segments": segb,
                "dsy_max": 0, "kills": 0,
                "mv_max_seen": max(r["mv_max"] for r in runs),
            }
        a.bands.write_text(json.dumps(bands, indent=1) + "
", newline="
")
        print(f"bands -> {a.bands} ({len(bands)} tests)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
