#!/usr/bin/env python3
"""binz recorded throttle profile: the qualification report's data source.

    python scripts/binz_profile.py --name step-10-60 --rec fast --profile 10:3,60:4,10:4
    python scripts/binz_profile.py --name ladder --rec slow --profile 10:5,20:5,...,10:5
    python scripts/binz_profile.py --name start1 --rec fast --from-arm --profile 10:4

Arm (zero throttle 6 s), sine-start ramp 1-4 %, the standard walk-in
5..10 % at 1 s each (as every qualification run; --no-walk skips it), then
each `throttle%:seconds` segment as an INSTANT setpoint (the firmware's own ramp shapes the response),
then stop. The firmware's time-series recorder (rm32::bench_rec; `R` = 10 ms,
`Y` = 50 ms per sample) is armed at the first segment, or at arming with
--from-arm, and dumped after the stop with `X`. Nothing is sent while the
bridge drives except setpoints.

Outputs (captures/binz/):
  profile_<name>_<ts>.txt   every line received (the raw capture)
  profile_<name>_<ts>.csv   t_s, duty, ehz, ma, mv, old_routine, dsy4
  profile_<name>_<ts>.json  segments with host start times relative to the
                            recorder arm, the `r` hold snapshot, kill line
Throttle % is the bench command (values <= 100 are percent).
`w` on every exit path; the firmware guard latches independently.
"""

from __future__ import annotations

import argparse
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
    ap.add_argument("--port", default=spin.PORT)
    a = ap.parse_args()
    segs = parse_profile(a.profile)
    period_s = (200 if a.rec == "fast" else 1000) * TICK_S
    span = 2048 * period_s
    walk = [] if a.no_walk else [(p, 1.0) for p in range(5, 11)]
    total = sum(s for _, s in segs) + ((3.5 + len(walk)) if a.from_arm else 0.0)
    if total > span * 0.98:
        raise SystemExit(f"profile {total:.1f} s exceeds the recorder span {span:.1f} s")
    ts = time.strftime("%Y%m%d_%H%M%S")
    base = pathlib.Path("captures/binz") / f"profile_{a.name}_{ts}"
    base.parent.mkdir(parents=True, exist_ok=True)
    bench = spin.Bench(a.port, base.with_suffix(".txt"))
    key = "R" if a.rec == "fast" else "Y"
    meta = {"name": a.name, "profile": segs, "period_s": period_s, "segments": [], "kill": None}
    ok = False
    try:
        bench.pump(0.2)
        if bench.query()["b"]["killed"]:
            raise RuntimeError("guard already latched — reset first")
        t0 = None
        bench.phase = "arm"
        bench.stream("0", 6.0)
        if a.from_arm:
            bench.send(key)
            t0 = time.time()
        for pct, secs in list(spin.SINE_RAMP) + walk:
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
    # Read back (also after an abort: the recorder froze at the kill).
    try:
        bench.lines.clear()
        bench.send("X")
        end = time.time() + 12
        while time.time() < end and not any(ln.startswith("rec end") for ln in bench.lines):
            bench.pump(0.2)
        rows = [ln for ln in bench.lines if ln.startswith("rd ")]
        head = [ln for ln in bench.lines if ln.startswith("rec n=")]
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
    n = int(re.search(r"n=(\d+)", head[-1]).group(1))
    if len(rows) != n:
        print(f"WARNING: dump has {len(rows)} rows, header says {n}")
    with base.with_suffix(".csv").open("w") as f:
        f.write("t_s,duty,ehz,ma,mv,old_routine,dsy4\n")
        for k, ln in enumerate(rows):
            df, ci, ma, mv = (int(x) for x in ln.split()[1:5])
            duty = df & 0x7FF
            ehz = round(2e6 / (6 * ci), 1) if ci and duty else 0.0
            f.write(f"{k * period_s:.3f},{duty},{ehz},{ma},{mv},{(df >> 11) & 1},{df >> 12}\n")
    meta["rows"] = len(rows)
    base.with_suffix(".json").write_text(json.dumps(meta, indent=1))
    print(f"{a.name}: {'OK' if ok else 'ABORTED'}; {len(rows)} samples at {period_s * 1000:.0f} ms; "
          f"{base.with_suffix('.csv')}")
    if meta.get("r"):
        r = meta["r"]
        print(f"  r: duty={r.get('duty')} imean_ma={r.get('imean_ma')} dsy={r.get('dsy')}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
