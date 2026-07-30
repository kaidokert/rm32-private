#!/usr/bin/env python3
"""ADC-pause confirmation at 100% — n reps, long hold.

divergence_audit found ADC-PAUSE rides 100% (dsy=6/4.6s all-recovered
vs 300+ unrecoverable in every other arm). Its classifier misread the
arm as DEGRADED because pausing the ADC freezes the current/voltage
measurements (stale amps failed the floor). This rerun classifies on
what pause cannot fake: Running + zc saturated + duty at command + dsy
rate. n reps x longer hold.
"""
import sys
import time

from bench_lib import Bench, reset_board


def rep(port, hold_secs):
    identity, causes = reset_board(port)
    if identity != "rm32":
        return ("ABORT", f"identity={identity}")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", "engage failed")
        b.cmd(b"A", settle=0.3)   # pause ADC scan
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
        s96 = b.info()
        print(f"  96%: {s96}")
        if s96 is None or not s96.running:
            return ("ABORT", "not locked at 96%")
        d0 = s96.dsy
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
            print(f"  [100 t={time.time()-t0:4.1f}] {inf}")
            if inf.dsy - d0 > 150:
                return ("STORM", repr(last))
        if last is None:
            return ("ABORT", "no samples")
        held = (last.running and last.zc > 9000 and last.duty >= 1990)
        rate = (last.dsy - d0) / hold_secs
        return ("HELD" if held else "FELL",
                f"dsy_rate={rate:.1f}/s end={last!r}")


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    hold = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 2
    out = []
    for r in range(1, n + 1):
        print(f"== ADC-PAUSE rep {r} ==")
        v, d = rep(port, hold)
        out.append(v)
        print(f"  -> {v}  {d}\n")
        time.sleep(4)
    print(f"ADC-PAUSE @100% x{hold:.0f}s: {out}")
    print("(baseline BASE arm: STORM within ~1.5s, dsy>300, unrecoverable)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
