#!/usr/bin/env python3
"""L431 recovery spin: settle at 0, gentle engage ramp up to the target,
hold it (info polled every few seconds), then rampdown + kill (bench_lib
guarantees the kill on every exit path). A quick "does the board still
spin" check after a re-image.

    python scripts/l431_recovery_spin.py --port COM18                # 10, 15 %
    python scripts/l431_recovery_spin.py --target 30 --hold 15
"""

import argparse
import atexit
import pathlib
import sys
import time

from bench_lib import ENGAGE_RAMP, STAGE_SECS, Bench

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM18")
ap.add_argument("--target", type=int, default=15)
ap.add_argument("--hold", type=float, default=5.0, help="seconds at the target")
ap.add_argument("--poll", type=float, default=3.0, help="info poll period at the target")
ap.add_argument("--amp-abort", type=float, default=None,
                help="kill above this current (default 0.8 A up to 15 %%, else 2.0 A)")
a = ap.parse_args()
amp_abort = a.amp_abort or (0.8 if a.target <= 15 else 2.0)


def check(b, pct, inf):
    print(f"  {pct}%: {inf}")
    if b.reboot_events or b.kill_line_seen:
        sys.exit("reboot or bench kill seen - aborting")
    if inf is None:
        sys.exit("no info line - aborting")
    if inf.amps > amp_abort:
        sys.exit(f"current {inf.amps:.2f} A > {amp_abort} A - stall? killing")


CAPTURE = pathlib.Path(__file__).resolve().parent / "captures" / time.strftime(
    "l431_spin_%Y%m%d_%H%M%S.txt")


def save(b):
    out = CAPTURE
    out.parent.mkdir(exist_ok=True)
    out.write_bytes(bytes(b.buf))
    print(f"  log: {out}")


b = Bench(a.port)
atexit.register(save, b)
with b:
    b.hold(0, 3.0)
    for pct in [s for s in ENGAGE_RAMP if s < a.target]:
        b.hold(pct, STAGE_SECS)
        check(b, pct, b.info())
    t0 = time.time()
    while (left := a.hold - (time.time() - t0)) > 0.5:
        b.hold(a.target, min(a.poll, left))
        check(b, a.target, b.info())
print("done, motor killed")
