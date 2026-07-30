#!/usr/bin/env python3
"""Flywheel commutation A/B at the 100% wall.

OFF arm: known baseline — desync storm within ~1 s at 100%.
ON arm ('O'): backup forced commutation at ~1.5x ci. Prediction: a
missed ZC becomes one blended-late step (fly= counts them) instead of
freezing the mux -> no multi-window silence -> no desync cascade ->
100% HOLDS with real current (~6 A).

Usage: flywheel_test.py [port] [hold_secs]
"""
import sys
import time

from bench_lib import Bench, reset_board


def run_arm(port, flywheel_on, hold_secs):
    identity, causes = reset_board(port)
    if identity != "rm32":
        return ("ABORT", f"identity={identity} causes={causes}")
    label = "FLY-ON " if flywheel_on else "FLY-OFF"
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", "engage failed: " + b.last_engage_report)
        if flywheel_on:
            b.cmd(b"O", settle=0.25)
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        s96 = b.info()
        print(f"  {label} 96%: {s96}")
        if s96 and s96.volts < 10.0:
            return ("ABORT", f"pack sagging ({s96.volts:.2f}V at 96%)")
        verdict = "HELD"
        last = None
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
        # final state decides HELD: Running, fast, real current
        if verdict == "HELD" and last is not None:
            ok = last.running and last.ci < 200 and last.amps > 4.0
            verdict = "HELD" if ok else "DEGRADED"
        return (verdict, repr(last))


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    hold = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    results = {}
    for on in (False, True):
        name = "FLY-ON" if on else "FLY-OFF"
        print(f"== arm {name} ==")
        v, d = run_arm(port, on, hold)
        results[name] = v
        print(f"  -> {v}\n")
        time.sleep(4)
    print("== flywheel A/B ==")
    for k, v in results.items():
        print(f"  {k:8s}: {v}")
    if results.get("FLY-OFF") == "STORM" and results.get("FLY-ON") == "HELD":
        print("  => FLYWHEEL FIXES THE WALL (miss-freeze amplifier confirmed)")
    elif results.get("FLY-ON") == "STORM":
        print("  => flywheel does NOT fix it — amplifier theory wrong or "
              "backup ineffective; check fly= count")
    return 0


if __name__ == "__main__":
    sys.exit(main())
