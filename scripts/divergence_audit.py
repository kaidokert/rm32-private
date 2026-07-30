#!/usr/bin/env python3
"""Kept-divergence audit at the 100% wall — one revert per arm.

Each arm: fresh reset (all toggles return to boot defaults), engage from
stop, verify clean 96%, then 4 s at 100%. STORM = dsy blows up; HELD =
Running, fast ci, real current at the end.

Arms (live toggles, no rebuild):
  ADC-PAUSE : 'A'    — rm32's software-random regular ADC scan OFF
                       (clone uses hw-timed injected; never tested @100%)
  COMP-FIX  : 'D'x2  — drive comp unconditional (clone-verbatim) vs AUTO
  DIODE     : 'D'x1  — diode freewheel drive
  FILTER-2  : 'F'x1  — persistence filter forced 2
  FILTER-1  : 'F'x2  — persistence filter forced 1
BASE (auto/adc-on/filter-map) is the established STORM baseline (n~10).
"""
import sys
import time

from bench_lib import Bench, reset_board

ARMS = [
    ("ADC-PAUSE", [b"A"]),
    ("COMP-FIX", [b"D", b"D"]),
    ("DIODE", [b"D"]),
    ("FILTER-2", [b"F"]),
    ("FILTER-1", [b"F", b"F"]),
]


def run_arm(port, label, keys, hold_secs=4.0):
    identity, causes = reset_board(port)
    if identity != "rm32":
        return ("ABORT", f"identity={identity} causes={causes}")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", "engage failed: " + b.last_engage_report)
        for k in keys:
            b.cmd(k, settle=0.3)
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        s96 = b.info()
        print(f"  {label} 96%: {s96}")
        if s96 is None or not s96.running:
            return ("ABORT", "not locked at 96% (toggle broke the ride?)")
        if s96.volts < 10.0:
            return ("ABORT", f"pack sag {s96.volts:.2f}V")
        last = None
        verdict = "HELD"
        t0 = time.time()
        while time.time() - t0 < hold_secs:
            b.hold(100, 1.0)
            if b.reboot_events:
                return ("ABORT", f"reboot {b.reboot_events}")
            if b.kill_line_seen:
                return ("ABORT", "guard kill")
            inf = b.info()
            if inf is None:
                continue
            last = inf
            print(f"  {label} [100 t={time.time()-t0:4.1f}] {inf}")
            if inf.dsy > 150:
                verdict = "STORM"
                break
        if verdict == "HELD" and last is not None:
            ok = last.running and last.ci < 200 and last.amps > 4.0
            verdict = "HELD" if ok else "DEGRADED"
        return (verdict, repr(last))


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    results = {}
    for label, keys in ARMS:
        print(f"== arm {label} ==")
        v, d = run_arm(port, label, keys)
        results[label] = v
        print(f"  -> {v}\n")
        time.sleep(4)
    print("== divergence audit (BASE = STORM, established) ==")
    for k, v in results.items():
        marker = "  <<<< WALL MOVED" if v == "HELD" else ""
        print(f"  {k:10s}: {v}{marker}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
