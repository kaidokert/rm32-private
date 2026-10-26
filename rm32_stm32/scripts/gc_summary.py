#!/usr/bin/env python3
"""Per-segment summary of binz_profile.py captures (gc refactor S1 bands).

    python scripts/gc_summary.py captures/l431/profile_s00-r1-ladder_*.json
    python scripts/gc_summary.py --bands out.json captures/*/profile_s00-*.json

For each capture and each segment: duty, eHz and mA averaged over the
segment's settled tail (last `--tail` seconds, default 2 s, never more than
half the segment), plus whole-run bus min/max mV, desync counts, old_routine
samples while driven, guard kill. Two desync counts: `dsy` is the recorder's
dsy4 field, which carries only the fast-rotor class (`dsy_fast`); `dsy_fw`
is the firmware's full `desync_events` (i-line `dsy=`), last i-line minus
first in the run's .txt log (it accumulates from the last flash on binz). Segment times come
from the host's segment starts relative to the recorder arm (JSON), mapped
onto the recorder's sample clock.

--bands groups captures by (board, test name without the run timestamp,
segment index) and writes min/max per quantity across runs: the S1
acceptance bands are min..max widened by --widen (default 5 %), never by
less than the board's --ma-floor on mA; segments shorter than
--min-settled seconds get no eHz/mA band (dsy/kill still gate them).
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
            "secs": end - start,
            "duty": statistics.mean(int(r["duty"]) for r in sel),
            "ehz": statistics.mean(float(r["ehz"]) for r in sel),
            "ma": statistics.mean(int(r["ma"]) for r in sel),
        })
    driven = [r for r in rows if int(r["duty"]) > 0]
    dsy = [int(r["dsy4"]) for r in rows]
    # dsy4 is a 4-bit wrapping counter of desync events.
    dsy_n = sum((b - a) % 16 for a, b in zip(dsy, dsy[1:]))
    mv = [int(r["mv"]) for r in rows if int(r["mv"]) > 0]
    dsy_fw = None
    txt = js.with_suffix(".txt")
    if txt.exists():
        il = [re.search(r" dsy=(\d+)", ln) for ln in txt.open(errors="replace")
              if ln.startswith("i step")]
        il = [int(m.group(1)) for m in il if m]
        if len(il) >= 2:
            dsy_fw = il[-1] - il[0]
    r = meta.get("r") or ""
    if isinstance(r, dict):
        r = " ".join(f"{k}={v}" for k, v in r.items())
    m = re.search(r"\bi50max=(-?\d+)", r)
    return {
        "board": meta.get("board", "binz"),
        "test": re.sub(r"_\d{8}_\d{6}$", "", js.stem.removeprefix("profile_")),
        "file": js.name,
        "kill": meta.get("kill") or meta.get("kill_line"),
        "segments": segs,
        "dsy": dsy_n,
        # Full firmware desync count over the run; None without a usable log.
        "dsy_fw": dsy_fw,
        "old_routine": sum(int(r["old_routine"]) for r in driven),
        "mv_min": min(mv) if mv else 0,
        "mv_max": max(mv) if mv else 0,
        # Peak of shared.actual_current() (the 1 kHz path); None if absent.
        "i50max": int(m.group(1)) if m else None,
    }


def check(out, bands) -> int:
    """A capture passes when it has no kill, no desync, the band's segment
    count, and every segment's duty/eHz/mA inside [lo, hi]. A capture whose
    test has no band is a failure (never silently skipped).

    binz also checks the 1 kHz current path against the guard's: the r-line
    `i50max` (peak of `actual_current()`) must be at least the highest
    settled segment `ma` (the guard's own ADC feed). The bands read only
    the guard feed, so an `actual_current` defect (S4.4) passed them; a
    binz run without an r-line `i50max` fails."""
    fails = 0
    for s in out:
        key = f"{s['board']}/{re.sub(r'^s[\d.]+[a-z0-9]*-', 's00-', s['test'])}"
        b = bands.get(key)
        why = []
        if b is None:
            why.append(f"no band {key}")
        else:
            if s["kill"]:
                why.append(f"kill: {s['kill']}")
            if s["dsy"] > b["dsy_max"]:
                why.append(f"dsy={s['dsy']}")
            if s["dsy_fw"] is None:
                why.append("no firmware desync count (.txt missing or < 2 i-lines)")
            elif s["dsy_fw"] > b["dsy_max"]:
                why.append(f"firmware desyncs={s['dsy_fw']} (all classes)")
            if len(s["segments"]) != len(b["segments"]):
                why.append(f"{len(s['segments'])} segments, band has {len(b['segments'])}")
            else:
                for g, bg in zip(s["segments"], b["segments"]):
                    for q in ("duty", "ehz", "ma"):
                        if bg[q] is None:  # not banded (transient segment)
                            continue
                        lo, hi = bg[q]
                        if not lo <= g[q] <= hi:
                            why.append(f"{g['throttle']}% {q}={g[q]:.1f} outside {lo}..{hi}")
            if s["board"] == "binz" and not s["kill"]:
                top = max(g["ma"] for g in s["segments"])
                if s["i50max"] is None:
                    why.append("no r-line i50max (actual_current unchecked)")
                elif s["i50max"] < top:
                    why.append(f"i50max={s['i50max']} < settled ma {top:.0f}"
                               " (actual_current diverges from the guard feed)")
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
    ap.add_argument("--ma-floor", action="append", default=[], metavar="BOARD=MA",
                    help="minimum mA half-widening for a board (its ADC count, e.g. l431=27)")
    ap.add_argument("--min-settled", type=float, default=0.0,
                    help="segments shorter than this (s) get no eHz/mA band (never settle)")
    ap.add_argument("--check", type=pathlib.Path,
                    help="verify captures against a saved bands JSON; exit 1 on any miss")
    a = ap.parse_args()
    out = [summarize(j, a.tail) for j in a.json]
    if a.check:
        return check(out, json.loads(a.check.read_text()))
    for s in out:
        print(f"{s['board']:5} {s['file']}: dsy={s['dsy']} dsy_fw={s['dsy_fw']} old={s['old_routine']} "
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
        floor = {k: float(v) for k, v in (f.split("=") for f in a.ma_floor)}
        bands = {}
        for (board, test), runs in sorted(groups.items()):
            nseg = {len(r["segments"]) for r in runs}
            if len(nseg) != 1:
                raise SystemExit(f"{board}/{test}: runs disagree on segment count {nseg}")
            segb = []
            for k in range(nseg.pop()):
                b = {"throttle": runs[0]["segments"][k]["throttle"],
                     "secs": round(runs[0]["segments"][k]["secs"], 2)}
                transient = b["secs"] < a.min_settled
                for q in ("duty", "ehz", "ma"):
                    v = [r["segments"][k][q] for r in runs]
                    b[q + "_runs"] = [round(x, 1) for x in v]
                    if transient and q != "duty":
                        b[q] = None
                        continue
                    lo, hi = min(v), max(v)
                    f = floor.get(board, 0.0) if q == "ma" else 0.0
                    b[q] = [round(lo - max(a.widen * abs(lo), f), 1),
                            round(hi + max(a.widen * abs(hi), f), 1)]
                segb.append(b)
            bands[f"{board}/{test}"] = {
                "runs": [r["file"] for r in runs],
                "segments": segb,
                "dsy_max": 0, "kills": 0,
                "mv_max_seen": max(r["mv_max"] for r in runs),
            }
        a.bands.write_text(json.dumps(bands, indent=1) + "\n", newline="\n")
        print(f"bands -> {a.bands} ({len(bands)} tests)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
