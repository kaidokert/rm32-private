#!/usr/bin/env python3
"""Continuously send 'A', 'B', 'C' to COM41 at 9600 8N1, one byte per second.

Pairs with examples/bitbang_uart_capture_only.rs — wire COM41's TX to PA0, GND-GND.
Ctrl+C to stop.
"""

import sys
import time

import serial

PORT = "COM41"
BAUD = 9600
CHARS = b"ABC"
INTERVAL_S = 2.0


def main() -> int:
    try:
        ser = serial.Serial(PORT, BAUD, bytesize=8, parity="N", stopbits=1, timeout=0)
    except serial.SerialException as exc:
        print(f"open {PORT} failed: {exc}", file=sys.stderr)
        return 1

    print(f"sending A,B,C @ {BAUD} 8N1 on {PORT} every {INTERVAL_S}s — Ctrl+C to stop")
    try:
        while True:
            for ch in CHARS:
                ser.write(bytes([ch]))
                ser.flush()
                print(f"tx 0x{ch:02X} '{chr(ch)}'", flush=True)
                time.sleep(INTERVAL_S)
    except KeyboardInterrupt:
        print()
        return 0
    finally:
        ser.close()


if __name__ == "__main__":
    sys.exit(main())
