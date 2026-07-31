#!/usr/bin/env python3
"""CH17-drop test: 100% hold with the VERBATIM desync trip (no 'T').

Build under test replaces ch17 (internal temp) with a ch8 repeat in the
regular scan. Baseline (ch17 present, verbatim T): storms in 1.2-2.5s.
If this build holds 100% on the verbatim detector, the jitter floor
dropped below avg/2 => ch17 mux-switching named as the injector and the
relaxed-T divergence becomes unnecessary.

Usage: ch17_test.py [port] [secs] [reps]
"""
import sys
import time

from bench_lib import Bench, reset_board


def rep(port, secs):
    identity, _ = reset_board(port)
    if identity != "rm32":
        return ("ABORT", "identity")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", "engage")
        # NO 'T' — verbatim avg/2 detector on purpose.
        for p in (70, 80, 90, 93, 96):
            b.hold(p, 2.2)
        s96 = b.info()
        print(f"  96%: {s96}")
        if s96 is None or not s96.running:
            return ("ABORT", "no lock at 96")
        d0 = s96.dsy
        t0 = time.time()
        last = None
        while time.time() - t0 < secs:
            b.hold(100, 1.0)
            if b.kill_line_seen or b.reboot_events:
                return ("ABORT", "kill/reboot")
            inf = b.info()
            if inf is None:
                continue
            last = inf
            print(f"  [100 t={time.time()-t0:4.1f}] {inf}")
            if inf.dsy - d0 > 150:
                return ("STORM", f"t={time.time()-t0:.1f}s")
        held = last is not None and last.running and last.duty >= 1990
        return ("HELD" if held else "FELL", repr(last))


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    secs = float(sys.argv[2]) if len(sys.argv) > 2 else 10.0
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 2
    out = []
    for r in range(1, n + 1):
        print(f"== ch17-drop rep {r} (VERBATIM detector) ==")
        v, d = rep(port, secs)
        out.append(v)
        print(f"  -> {v} ({d})\n")
        time.sleep(4)
    print(f"ch17-drop @100% verbatim-T: {out}")
    print("(ch17-present verbatim baseline: STORM at 1.2-2.5s, n=6+)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
