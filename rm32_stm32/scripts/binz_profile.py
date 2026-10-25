#!/usr/bin/env python3
"""Recorded throttle profile on either bench board: the qualification
report's data source.

    python scripts/binz_profile.py --name step-10-60 --rec fast --profile 10:3,60:4,10:4
    python scripts/binz_profile.py --board l431 --name step-10-60 --rec fast --profile 10:3,60:4,10:4
    python scripts/binz_profile.py --name ladder --rec slow --profile 10:5,20:5,...,10:5
    python scripts/binz_profile.py --name start1 --rec fast --from-arm --profile 10:4

Arm (zero throttle 6 s); on binz the sine-start ramp 1-4 % and the
standard walk-in 5..10 % at 1 s each (as every qualification run; --no-walk
skips it; the L431 has no sine start and steps straight to the first
segment, as every L431 run has); then
each `throttle%:seconds` segment as an INSTANT setpoint (the firmware's own ramp shapes the response),
then stop. The firmware's time-series recorder (rm32::bench_rec; `R` = 10 ms,
`Y` = 50 ms per sample) is armed at the first segment, or at arming with
--from-arm, and dumped after the stop with `X`. Nothing is sent while the
bridge drives except setpoints.

Outputs (captures/<board>/):
  profile_<name>_<ts>.txt   every line received (the raw capture)
  profile_<name>_<ts>.csv   t_s, duty, ehz, ma, mv, old_routine, dsy4
  profile_<name>_<ts>.json  segments with host start times relative to the
                            recorder arm, the `r` hold snapshot, kill line
Throttle % is the bench command (values <= 100 are percent).
`w` on every exit path; the firmware guard latches independently.
"""

from __future__ import annotations

import argparse
import collections
import importlib.util
import json
import pathlib
import re
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("binz_spin", HERE / "binz_spin.py")
spin = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(spin)

TICK_S = 50e-6  # control tick
ROW = re.compile(r"rd \d+ \d+ -?\d+ \d+")

# Per-board link and start preamble. binz: COM41 115200, sine start, `i`
# answers with i/b lines. L431: COM18 2 Mbaud (benchuart), no sine start,
# `i` answers with the i line only.
# `walkdown`: throttle steps (0.35 s each, only those below the last segment)
# before the stop. The L431 prop at 60 % stopped from full drive pumped the
# shared PSU bus 11.5 -> 17 V in 100 ms (both boards' OV guards latched,
# 2026-10-25); its bench scripts always walked down (bench_lib
# rampdown_kill). binz keeps the abrupt stop every qualification used.
BOARDS = {
    "binz": {"port": spin.PORT, "baud": spin.BAUD, "need": ("i", "b"), "sine": True,
             "walkdown": ()},
    "l431": {"port": "COM18", "baud": 2_000_000, "need": ("i",), "sine": False,
             "walkdown": (60, 40, 25, 15)},
}


def parse_profile(text: str) -> list[tuple[int, float]]:
    out = []
    for part in text.split(","):
        t, s = part.split(":")
        t, s = int(t), float(s)
        if not 1 <= t <= 100:
            raise SystemExit(f"throttle {t}% outside 1..100")
        out.append((t, s))
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--name", required=True)
    ap.add_argument("--profile", required=True, help="throttle%%:seconds,...")
    ap.add_argument("--rec", choices=["fast", "slow"], default="fast")
    ap.add_argument("--from-arm", action="store_true",
                    help="record from the start of the sine ramp (starts)")
    ap.add_argument("--no-walk", action="store_true",
                    help="skip the standard walk-in (5..10 %% at 1 s each) after the sine ramp: "
                         "the first segment is then a direct step from the sine band")
    ap.add_argument("--board", choices=sorted(BOARDS), default="binz")
    ap.add_argument("--port", help="override the board's default port")
    a = ap.parse_args()
    board = BOARDS[a.board]
    segs = parse_profile(a.profile)
    period_s = (200 if a.rec == "fast" else 1000) * TICK_S
    span = 2048 * period_s
    ramp = list(spin.SINE_RAMP) if board["sine"] else []
    walk = [] if a.no_walk or not board["sine"] else [(p, 1.0) for p in range(5, 11)]
    total = sum(s for _, s in segs) + (sum(s for _, s in ramp + walk) if a.from_arm else 0.0)
    if total > span * 0.98:
        raise SystemExit(f"profile {total:.1f} s exceeds the recorder span {span:.1f} s")
    ts = time.strftime("%Y%m%d_%H%M%S")
    base = pathlib.Path("captures") / a.board / f"profile_{a.name}_{ts}"
    base.parent.mkdir(parents=True, exist_ok=True)

    def out(ext: str) -> pathlib.Path:
        # Not with_suffix: a step name like "s02.1-r1" contains a dot.
        return base.parent / f"{base.name}{ext}"
    bench = spin.Bench(a.port or board["port"], out(".txt"),
                       board["baud"], board["need"])
    key = "R" if a.rec == "fast" else "Y"
    meta = {"board": a.board, "name": a.name, "profile": segs, "period_s": period_s, "segments": [], "kill": None}
    ok = False
    try:
        bench.pump(0.2)
        q0 = bench.query()
        if q0.get("b", q0["i"])["killed"]:
            raise RuntimeError("guard already latched — reset first")
        t0 = None
        bench.phase = "arm"
        bench.stream("0", 6.0)
        if a.from_arm:
            bench.send(key)
            t0 = time.time()
        for pct, secs in ramp + walk:
            bench.phase, bench.t_phase = f"sine {pct}", time.time()
            bench.stream(str(pct), secs)
        for i, (thr, secs) in enumerate(segs):
            if i == 0:
                # Silent aggregate reset: the post-stop counters (lost
                # ticks, COMP load, loop gap, sector histogram) then cover
                # the recorded segments only.
                bench.send("H")
                if t0 is None:
                    bench.send(key)
                    t0 = time.time()
            meta["segments"].append({"throttle": thr, "secs": secs, "t_start": time.time() - t0})
            bench.phase, bench.t_phase = f"seg {thr}%", time.time()
            bench.stream(str(thr), secs)
        meta["t_stop"] = time.time() - t0
        for pct in board["walkdown"]:
            if pct < segs[-1][0]:
                bench.phase, bench.t_phase = f"walkdown {pct}%", time.time()
                bench.stream(str(pct), 0.35)
        bench.send("0")
        bench.send("0")
        bench.pump(1.0)
        ok = True
    except KeyboardInterrupt:
        print("interrupted")
    except Exception as e:  # noqa: BLE001 — every failure ends in the kill path
        print(f"ABORT: {e}")
        meta["kill"] = str(e)
    finally:
        bench.kill()
        bench.pump(0.3)
    # Read back (also after an abort: the recorder froze at the kill). The
    # record stays frozen, so it is read repeatedly and voted row by row:
    # the L431's CH340 at 2 Mbaud corrupts bytes, and a byte lost inside a
    # number (11821 -> 1182) still looks like a valid row. Only dumps with
    # the header's row count and every row well-formed take part (so rows
    # align by position); a row is accepted when >= 2 dumps agree on it and
    # no other value ties. Any row left unresolved -> no CSV.
    try:
        dumps: list[list[str]] = []
        whole = False
        rows: list[str] = []
        n = -1
        for attempt in range(12):
            bench.lines.clear()
            bench.send("X")
            end = time.time() + 12
            while time.time() < end and not any(ln.startswith("rec end") for ln in bench.lines):
                bench.pump(0.2)
            got = [ln for ln in bench.lines if ln.startswith("rd ")]
            head = [ln for ln in bench.lines if ln.startswith("rec n=")]
            if not head:
                break
            n = int(re.search(r"n=(\d+)", head[-1]).group(1))
            if len(got) == n and all(ROW.fullmatch(ln) for ln in got):
                dumps.append(got)
            if len(dumps) >= 2:
                voted, unresolved = [], 0
                for k in range(n):
                    (v, c), *rest = collections.Counter(d[k] for d in dumps).most_common()
                    if c >= 2 and (not rest or rest[0][1] < c):
                        voted.append(v)
                    else:
                        unresolved += 1
                if unresolved == 0:
                    rows, whole = voted, True
                    break
            print(f"  dump {attempt + 1}: {len(dumps)} whole read(s) so far; re-reading")
        rows = rows or (dumps[-1] if dumps else [])
        q = bench.query()
        meta["r"] = q.get("r")
        meta["b"] = q.get("b")
        kills = [ln for ln in bench.lines if "BENCH KILL" in ln]
        if kills:
            meta["kill_line"] = kills[0]
    finally:
        bench.kill()
        bench.close()
    if not head:
        print("no recorder dump (firmware without the recorder?)")
        return 1
    if not whole:
        print(f"recorder dump never arrived whole ({len(rows)}/{n} rows) — no CSV written")
        return 1
    with out(".csv").open("w") as f:
        f.write("t_s,duty,ehz,ma,mv,old_routine,dsy4\n")
        for k, ln in enumerate(rows):
            df, ci, ma, mv = (int(x) for x in ln.split()[1:5])
            duty = df & 0x7FF
            ehz = round(2e6 / (6 * ci), 1) if ci and duty else 0.0
            f.write(f"{k * period_s:.3f},{duty},{ehz},{ma},{mv},{(df >> 11) & 1},{df >> 12}\n")
    meta["rows"] = len(rows)
    out(".json").write_text(json.dumps(meta, indent=1))
    print(f"{a.name}: {'OK' if ok else 'ABORTED'}; {len(rows)} samples at {period_s * 1000:.0f} ms; "
          f"{out('.csv')}")
    if meta.get("r"):
        r = meta["r"]
        print(f"  r: duty={r.get('duty')} imean_ma={r.get('imean_ma')} dsy={r.get('dsy')}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
