#!/usr/bin/env python3
"""binz coast-down speed check: is the loop's speed reading the rotor's?

    python scripts/binz_coast.py --rung 800 --hold 10

Arm, sine ramp, walk to the rung (applied duty, tenths), `H`, quiet hold;
then `C` instead of a stop: the firmware zeroes the throttle, cuts the
bridge, and times phase A's free BEMF crossings while the rotor coasts
(mcu_g071/coast.rs). Nothing is sent while the bridge drives.

Reported:
  loop_ehz   1e7 / ecom10: the hold-mean commutation interval (the `r`
             snapshot, frozen at the stop edge the `C` causes)
  coast_ehz  1e6 / (2 * median of half-periods 2..9), in us. The first
             crossing is discarded (comparator settling after its input
             switch: one crossing ~30 us after the cut even on a stationary
             rotor), so iv[0] is never used.
  decel      the last recorded half-period against the first used one
A loop reading above the coast reading by more than the rotor can lose in
the first half-periods means the loop's estimate is biased (firmware50
reads 0.6-1.8 % above its coast at <= 37.5 %, 2.5-3.5 % at 80-90 %).

Every line goes to captures/binz/coast_<ts>.txt; `w` on every exit.
"""

from __future__ import annotations

import argparse
import importlib.util
import pathlib
import re
import statistics
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("binz_spin", HERE / "binz_spin.py")
spin = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(spin)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rung", type=int, required=True, help="applied duty, tenths")
    ap.add_argument("--hold", type=float, default=10.0)
    ap.add_argument("--walk-step", type=int, default=5)
    ap.add_argument("--port", default=spin.PORT)
    a = ap.parse_args()
    if a.rung > spin.MAX_RUNG_TENTHS:
        raise SystemExit(f"rung {a.rung} refused (climb cap {spin.MAX_RUNG_TENTHS})")
    cmd = str(spin.rung_command(a.rung))
    out = pathlib.Path("captures/binz") / time.strftime("coast_%Y%m%d_%H%M%S.txt")
    out.parent.mkdir(parents=True, exist_ok=True)
    bench = spin.Bench(a.port, out)
    verdict = 1
    try:
        bench.pump(0.2)
        if bench.query()["b"]["killed"]:
            raise RuntimeError("guard already latched — reset first")
        bench.phase = "arm"
        bench.stream("0", 6.0)
        for pct, secs in spin.SINE_RAMP:
            bench.phase, bench.t_phase = f"sine {pct}", time.time()
            bench.stream(str(pct), secs)
        for pct in spin.walk_commands(cmd, a.walk_step):
            bench.phase, bench.t_phase = f"walk {pct}", time.time()
            bench.stream(pct, 1.0)
        bench.send("H")
        bench.pump(0.05)
        bench.phase, bench.t_phase = f"hold {cmd}", time.time()
        bench.stream(cmd, a.hold)
        bench.send("C")  # zero throttle + bridge off + coast timing
        bench.pump(0.4)
        bench.send("0")
        bench.send("0")
        bench.pump(0.5)
        # query() clears bench.lines: take the coast lines first.
        head = [ln for ln in bench.lines if ln.startswith("coast trans=")]
        rows = [ln for ln in bench.lines if ln.startswith("coast iv_cyc[")]
        q = bench.query()
        r = q.get("r")
        if not r:
            raise RuntimeError("no hold snapshot (r line) after the coast")
        if not head or len(rows) < 4:
            raise RuntimeError("no coast timing lines (firmware without the C key?)")
        m = re.search(r"trans=(\d+) first_cyc=(\d+) cpu_mhz=(\d+)", head[-1])
        trans, first_cyc, mhz = (int(x) for x in m.groups())
        iv = []
        for ln in rows[-4:]:
            iv += [int(x) for x in ln.split("]")[1].split()]
        used = [c / mhz for c in iv[1:9] if c > 0]
        if len(used) < 8:
            raise RuntimeError(f"only {trans} crossings recorded: rotor not coasting? iv={iv[:10]}")
        coast_ehz = 1e6 / (2 * statistics.median(used))
        loop_ehz = 1e7 / r["ecom10"] if r["ecom10"] else 0.0
        last = [c / mhz for c in iv if c > 0][-1]
        print(f"rung {a.rung}: loop_ehz={loop_ehz:.0f} coast_ehz={coast_ehz:.0f} "
              f"(loop {100 * (loop_ehz / coast_ehz - 1):+.1f} % vs coast); "
              f"imean_ma={r['imean_ma']} dsy={r['dsy']}; trans={trans} "
              f"first_us={first_cyc / mhz:.0f}; half-periods us {used[0]:.1f} .. {last:.1f} "
              f"(decel {100 * (last / used[0] - 1):+.1f} % over the record)")
        verdict = 0
    except KeyboardInterrupt:
        print("interrupted")
    except Exception as e:  # noqa: BLE001 — every failure ends in the kill path
        print(f"ABORT: {e}")
    finally:
        bench.kill()
        bench.pump(0.3)
        bench.close()
        print(f"log: {out}")
    return verdict


if __name__ == "__main__":
    sys.exit(main())
