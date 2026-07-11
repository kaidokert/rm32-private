#!/usr/bin/env python3
"""Drive motor_tester2 over serial: send keys with delays, capture output.

Usage:
    uart_cmd.py [--port COM41] [--baud 2000000] KEY:DELAY [KEY:DELAY ...]

Each token is a single key to send, a colon, and the seconds to wait
(and capture) after sending it. Example — arm at 50 Hz, sample rates,
then kill:

    python scripts/uart_cmd.py q:3 b:1 i:1 w:0.5
"""

import argparse
import sys
import time

import serial

# Firmware output includes non-ASCII (µ); don't let a cp1252 console
# kill the key sequence mid-run.
sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("tokens", nargs="+", metavar="KEY:DELAY")
args = ap.parse_args()

steps = []
for tok in args.tokens:
    key, _, delay = tok.partition(":")
    if len(key) != 1:
        sys.exit(f"bad token {tok!r}: KEY must be one character")
    steps.append((key, float(delay) if delay else 0.5))


def safe_print(*a, **kw):
    """stdout can die mid-run (e.g. a truncating consumer like
    PowerShell's Select-Object -First closes the pipe). Output loss
    is acceptable; aborting the KEY SEQUENCE is not — an un-sent
    trailing 'w' once left the motor spinning unattended
    (2026-07-11). Swallow pipe errors, keep sending keys."""
    try:
        print(*a, **kw)
    except OSError:
        pass


with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    p.reset_input_buffer()
    completed = False
    try:
        for key, delay in steps:
            p.write(key.encode())
            safe_print(f"--- sent {key!r}, capturing {delay}s ---")
            end = time.monotonic() + delay
            buf = bytearray()
            while time.monotonic() < end:
                buf += p.read(4096)
            text = buf.decode("ascii", errors="replace")
            if text.strip():
                safe_print(text.rstrip())
        completed = True
    finally:
        # Kill guard: if the sequence did not run to completion, the
        # operator's trailing kill key may never have been sent. 'w'
        # is idempotent (prints "off" only if armed).
        if not completed:
            try:
                p.write(b"w")
                time.sleep(0.2)
                safe_print("!! sequence aborted - sent 'w' kill guard")
            except Exception:
                pass
