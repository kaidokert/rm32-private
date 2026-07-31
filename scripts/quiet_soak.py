#!/usr/bin/env python3
"""Quiet soak: hold a throttle with NO mid-run info polling — pure 10 Hz
throttle refresh only, one info at the end. Discriminates the audible
~2s beat: instrumentation cadence (beat vanishes) vs physical
(wire/motor — beat persists).

Usage: quiet_soak.py [port] [pct] [secs]
"""
import sys
import time

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    pct = int(sys.argv[2]) if len(sys.argv) > 2 else 100
    secs = float(sys.argv[3]) if len(sys.argv) > 3 else 60.0
    identity, _ = reset_board(port)
    if identity != "rm32":
        print(f"ABORT identity={identity}")
        return 1
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            print("engage failed")
            return 1
        b.cmd(b"T", settle=0.25)
        for p in (70, 80, 90, 96, pct):
            if p <= pct:
                b.hold(p, 2.2)
        print(f"pre:  {b.info()}")
        print(f"== quiet hold {pct}% x{secs:.0f}s — NO polling ==")
        b.hold(pct, secs)          # pure 10 Hz refresh, nothing else
        print(f"post: {b.info()}")
    print("killed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
