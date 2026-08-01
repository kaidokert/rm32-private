#!/usr/bin/env python3
"""Run one Betaflight CLI command and print the response (roundtrip).

Enters the CLI on the FC's USB port, sends the command, captures the
echo + response, and exits the CLI. NOTE: BF's `exit` reboots the FC —
the ESC will see a ~1-2 s signal gap and do one reset/re-arm cycle
(expected). Use --stay to leave the CLI session open instead (no
reboot, but BF stays in CLI mode).

Usage: bf_cli.py "status" [--port COM42] [--stay]
"""
import argparse
import sys
import time

import serial


def drain(p, quiet=0.25):
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
    ap.add_argument("command")
    ap.add_argument("--port", default="COM42")
    ap.add_argument("--stay", action="store_true", help="leave CLI open (no FC reboot)")
    a = ap.parse_args()
    p = serial.Serial(a.port, 115_200, timeout=0.05)
    try:
        p.write(b"#\n")
        drain(p, 0.4)
        p.write((a.command + "\n").encode())
        print(drain(p, 0.6).strip())
        return 0
    finally:
        try:
            if not a.stay:
                p.write(b"exit\n")
                time.sleep(0.2)
        finally:
            p.close()


if __name__ == "__main__":
    sys.exit(main())
