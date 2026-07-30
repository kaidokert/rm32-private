#!/usr/bin/env python3
"""No-spin bench status: reset (clears guard latch), identity, reset
causes, rest voltage. Safe to run any time — never drives the motor."""
import sys

from bench_lib import Bench, reset_board


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    identity, causes = reset_board(port)
    print(f"identity: {identity}")
    print(f"reset causes: {causes}")
    with Bench(port) as b:
        b.hold(0, 1.0)
        inf = b.info()
        print(f"idle: {inf}")
        if inf:
            v = inf.volts
            state = ("HEALTHY" if v > 11.6 else
                     "LOW - charge before load runs" if v > 10.5 else
                     "DEPLETED - do not run under load")
            print(f"rest voltage: {v:.2f} V -> {state}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
