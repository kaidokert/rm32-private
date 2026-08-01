#!/usr/bin/env python3
"""Send bench verbs over the PA0 soft-UART link and capture replies.

Two-baud protocol (see softuart_check.py): each token's text is sent
at 9600 (the ESC's PA0 RX rate), then the port switches to 115200 to
capture the ESC's PB6 debug output for the token's delay window.

Vocabulary (rm32::bench_input): `c` eeprom hex dump, `<n>o` latch
config offset, `<n>v` write value via the A4 ring, `S` save settings,
`i` info line, `B` recorder dump, `w` kill.

Usage:
    softuart_cmd.py [--port COM41] TOKEN:DELAY [TOKEN:DELAY ...]
Example — persist round trip on offset 32:
    softuart_cmd.py c:3 32o:1 129v:1 S:3 c:3 32o:1 128v:1 S:3 c:3
"""
import argparse
import sys
import time

import serial


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("tokens", nargs="+", metavar="TOKEN:DELAY")
    ap.add_argument("--raw", action="store_true",
                    help="print all captured lines, not just []-tagged ones")
    a = ap.parse_args()

    p = serial.Serial(a.port, 115_200, timeout=0.05)
    try:
        time.sleep(0.5)
        p.reset_input_buffer()
        for tok in a.tokens:
            text, _, delay = tok.rpartition(":")
            delay = float(delay)
            p.baudrate = 9_600
            time.sleep(0.05)
            # Trailing newline terminates any pending accumulator state.
            p.write(text.encode() + b"\n")
            p.flush()
            time.sleep(0.05 + 0.002 * len(text))
            p.baudrate = 115_200
            p.reset_input_buffer()  # drop garbled-while-9600 input
            deadline = time.time() + delay
            buf = b""
            while time.time() < deadline:
                buf += p.read(4096)
            print(f">>> {text!r}")
            for line in buf.decode("ascii", "replace").splitlines():
                line = line.strip()
                if not line:
                    continue
                if a.raw or line.startswith(("[eep", "[cfg", "[i ", "[su",
                                             "SR", "[bench")):
                    print(f"    {line}")
    finally:
        p.baudrate = 115_200
        p.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
