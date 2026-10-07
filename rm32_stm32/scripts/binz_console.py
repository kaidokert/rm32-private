#!/usr/bin/env python3
"""Minimal binz bench console: send commands to rm32 over USART3 (COM41,
115200) and print everything received for a while.

    python scripts/binz_console.py --listen 3            # just listen
    python scripts/binz_console.py --send "i" --listen 1

Commands are sent with a trailing newline. Never sends throttle — use
binz_spin.py for anything that can drive the motor.
"""

from __future__ import annotations

import argparse
import sys
import time

import serial

PORT = "COM41"
BAUD = 115_200


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=PORT)
    ap.add_argument("--send", action="append", default=[])
    ap.add_argument("--listen", type=float, default=2.0)
    args = ap.parse_args()
    for cmd in args.send:
        if any(c.isdigit() for c in cmd) and cmd.strip() not in ("0",):
            raise SystemExit(f"refusing throttle-like command {cmd!r}; use binz_spin.py")
    with serial.Serial(args.port, BAUD, timeout=0.05) as s:
        s.reset_input_buffer()
        for cmd in args.send:
            s.write((cmd + "\n").encode())
            time.sleep(0.05)
        end = time.time() + args.listen
        while time.time() < end:
            b = s.read(4096)
            if b:
                sys.stdout.buffer.write(b)
                sys.stdout.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
