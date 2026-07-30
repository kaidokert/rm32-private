#!/usr/bin/env python3
"""ABBA: HW-TIMED ADC (mode 2, phase-locked injected scan, live
readings) vs baseline (mode 0, software-random scan) at 100%.

The shippable-fix test. Mode 2 = 'A' pressed twice (0->1->2). Unlike
mode 1 (paused), readings stay LIVE so the amps/volts checks are real.
Metric: storm onset time (dsy delta > 100) or HELD to cap.
"""
import sys
import time

from bench_lib import Bench, reset_board


def arm(port, hw, cap):
    identity, _ = reset_board(port)
    if identity != "rm32":
        return ("ABORT", 0.0, "identity")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", 0.0, "engage")
        pre = b.info()
        if pre is None or pre.volts < 11.0:
            return ("ABORT", 0.0, f"pack {pre.volts if pre else '?'}V")
        if hw:
            b.cmd(b"A", settle=0.25)
            b.cmd(b"A", settle=0.25)   # mode 2
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
                print(f"  [storm sample] {inf}")
                return ("STORM", time.time() - t0, f"dsy+{inf.dsy - d0}")
        print(f"  [end] {last}")
        return ("HELD", cap, repr(last))


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    cap = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    seq = [("HW1", True), ("B1", False), ("B2", False), ("HW2", True)]
    res = {}
    for name, hw in seq:
        print(f"== {name} ({'HW-TIMED' if hw else 'baseline'}) ==")
        v, t, d = arm(port, hw, cap)
        res[name] = (v, t)
        print(f"  -> {v} t={t:.1f}s ({d})\n")
        time.sleep(4)
    print("== ABBA (onset s; HELD = full cap) ==")
    for k, (v, t) in res.items():
        print(f"  {k}: {v:6s} t={t:.1f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
