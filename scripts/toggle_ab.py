#!/usr/bin/env python3
"""Generic ABBA toggle test at 100%: --keys "T" (or "TA", "OA"...) vs
baseline. Order A-B-B-A cancels linear pack drift. Metric: storm onset
(dsy delta > 100) or HELD to cap.

Usage: toggle_ab.py --keys T [--cap 10] [--port COM41]
"""
import argparse
import sys
import time

from bench_lib import Bench, reset_board


def arm(port, keys, cap):
    identity, _ = reset_board(port)
    if identity != "rm32":
        return ("ABORT", 0.0, "identity")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", 0.0, "engage")
        pre = b.info()
        if pre is None or pre.volts < 11.0:
            return ("ABORT", 0.0, f"pack {pre.volts if pre else '?'}V")
        for k in keys:
            b.cmd(k.encode(), settle=0.25)
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        s96 = b.info()
        print(f"  96%: {s96}")
        if s96 is None or not s96.running:
            return ("ABORT", 0.0, "no lock at 96")
        d0 = s96.dsy
        t0 = time.time()
        last = None
        while time.time() - t0 < cap:
            b.hold(100, 0.8)
            inf = b.info()
            if inf is None:
                continue
            last = inf
            if inf.dsy - d0 > 100:
                print(f"  [storm] {inf}")
                return ("STORM", time.time() - t0, f"dsy+{inf.dsy - d0}")
        print(f"  [end] {last}")
        held = last is not None and last.running and last.zc > 9000
        return ("HELD" if held else "FELL", cap, repr(last))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--keys", required=True,
                    help="toggle keys for the ON arms, e.g. T or TA")
    ap.add_argument("--cap", type=float, default=10.0)
    ap.add_argument("--port", default="COM41")
    a = ap.parse_args()
    seq = [("ON1", a.keys), ("OFF1", ""), ("OFF2", ""), ("ON2", a.keys)]
    res = {}
    for name, keys in seq:
        print(f"== {name} ({'keys=' + keys if keys else 'baseline'}) ==")
        v, t, d = arm(a.port, keys, a.cap)
        res[name] = (v, t)
        print(f"  -> {v} t={t:.1f}s ({d})\n")
        time.sleep(4)
    print(f"== ABBA keys={a.keys!r} (onset s; HELD = cap) ==")
    for k, (v, t) in res.items():
        print(f"  {k}: {v:6s} t={t:.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
