#!/usr/bin/env python3
"""ABBA interleave: ADC-pause vs baseline at 100%, drift-cancelling.

Order A-B-B-A cancels linear pack drain within the comparison. Metric =
storm onset time (first sample with dsy delta > 100) or HELD to the cap.
Short exposures; pack floor enforced per arm.
"""
import sys
import time

from bench_lib import Bench, reset_board


def arm(port, adc_pause, cap):
    identity, _ = reset_board(port)
    if identity != "rm32":
        return ("ABORT", 0.0, "identity")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", 0.0, "engage")
        pre = b.info()
        if pre is None or pre.volts < 11.0:
            return ("ABORT", 0.0, f"pack {pre.volts if pre else '?'}V")
        if adc_pause:
            b.cmd(b"A", settle=0.3)
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        s96 = b.info()
        if s96 is None or not s96.running:
            return ("ABORT", 0.0, "no lock at 96")
        d0 = s96.dsy
        t0 = time.time()
        while time.time() - t0 < cap:
            b.hold(100, 0.8)
            inf = b.info()
            if inf is None:
                continue
            if inf.dsy - d0 > 100:
                return ("STORM", time.time() - t0, f"dsy+{inf.dsy - d0}")
        return ("HELD", cap, "capped")


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    cap = float(sys.argv[2]) if len(sys.argv) > 2 else 8.0
    seq = [("A1", True), ("B1", False), ("B2", False), ("A2", True)]
    res = {}
    for name, pause in seq:
        print(f"== {name} ({'ADC-pause' if pause else 'baseline'}) ==")
        v, t, d = arm(port, pause, cap)
        res[name] = (v, t)
        print(f"  -> {v} at t={t:.1f}s ({d})\n")
        time.sleep(4)
    print("== ABBA result (onset seconds; higher = better) ==")
    for k, (v, t) in res.items():
        print(f"  {k}: {v:6s} t={t:.1f}")
    a = [res[k][1] for k in ("A1", "A2") if res[k][0] != "ABORT"]
    bb = [res[k][1] for k in ("B1", "B2") if res[k][0] != "ABORT"]
    if a and bb:
        print(f"\n  ADC-pause mean onset: {sum(a)/len(a):.1f}s   "
              f"baseline mean onset: {sum(bb)/len(bb):.1f}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
