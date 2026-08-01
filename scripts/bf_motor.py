#!/usr/bin/env python3
"""Drive a motor through the Betaflight CLI (`motor <idx> <value>`).

The BF-phase throttle path: rm32 is a DShot/PWM consumer, so throttle
commands go through the FC exactly as flight traffic would. This enters
the BF CLI on the FC's USB port, applies a motor override, holds, and
ALWAYS restores motor to 1000 + exits CLI on the way out (kill guard on
every exit path — a stalled open-loop drive is a 2 A heater).

ESC-side telemetry rides the PB6 debuguart on a separate port; run
serial_tail.py alongside (the [loop] heartbeat is suppressed while
Running — silence during the hold is expected, the post-stop line
carries the zc/duty evidence).

Usage: bf_motor.py [--port COM42] [--index 0] [--pct 25] [--seconds 8]
"""
import argparse
import sys
import time

import serial


def drain(p, quiet=0.15):
    out = b""
    t = time.time()
    while time.time() - t < quiet:
        b = p.read(4096)
        if b:
            out += b
            t = time.time()
    return out.decode("ascii", "replace")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM42")
    ap.add_argument("--index", type=int, default=0, help="CLI motor index (0-based)")
    ap.add_argument("--pct", type=float, default=25.0, help="throttle percent (0-100)")
    ap.add_argument("--seconds", type=float, default=8.0)
    a = ap.parse_args()
    value = int(1000 + a.pct * 10)
    p = serial.Serial(a.port, 115_200, timeout=0.05)
    try:
        p.write(b"#\n")  # enter CLI
        banner = drain(p, 0.5)
        if "CLI" not in banner and "#" not in banner:
            print(f"no CLI banner from {a.port}; got: {banner[:120]!r}")
            return 1
        p.write(f"motor {a.index} {value}\n".encode())
        print(f"motor {a.index} -> {value} ({a.pct:.0f}%), holding {a.seconds}s")
        print(drain(p, 0.3).strip())
        time.sleep(a.seconds)
        return 0
    finally:
        # Kill guard: restore + exit on EVERY path.
        try:
            p.write(f"motor {a.index} 1000\n".encode())
            time.sleep(0.3)
            p.write(b"exit\n")  # exit reboots? no: 'exit' leaves CLI, keeps config
            time.sleep(0.3)
            print("motor restored to 1000, CLI exited")
        finally:
            p.close()


if __name__ == "__main__":
    sys.exit(main())
