#!/usr/bin/env python3
"""PA0 soft-UART RX validation via the latched [su] heartbeat.

One adapter, one port, TWO baud rates: the ESC's PB6 debug TX runs at
115200 while its PA0 soft-UART RX listens at 9600, and the port's baud
applies to both directions. An immediate firmware echo is transmitted
while the host is still parked at 9600 (lost), so validation reads the
LATCHED state instead: firmware surfaces frames/errors/overruns +
last-byte in the periodic `[su f= e= o= n= last=]` line, which the
host reads at 115200 after each send.

Wire: USB-TTL TX -> header pin 4 (HSE_IN / PA0), RX -> PB6 pad,
GND common.

Usage: softuart_check.py [--port COM41] [--bytes ABC]
"""
import argparse
import re
import sys
import time

import serial

SU_RE = re.compile(r"\[su f=(\d+) e=(\d+) o=(\d+) n=(\d+) last=(0x[0-9a-fA-F]{2})\]")


def read_su(p, seconds=6.0):
    buf = b""
    deadline = time.time() + seconds
    last = None
    while time.time() < deadline:
        buf += p.read(4096)
        m = None
        for m in SU_RE.finditer(buf.decode("ascii", "replace")):
            pass
        if m:
            last = m
    return last


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--bytes", default="ABC")
    a = ap.parse_args()

    p = serial.Serial(a.port, 115_200, timeout=0.05)
    results = []
    try:
        time.sleep(1.0)
        p.reset_input_buffer()
        base = read_su(p, 6.0)
        if base is None:
            print("no [su] heartbeat — wrong build or ESC not booted?")
            return 1
        print(f"baseline: f={base.group(1)} e={base.group(2)} "
              f"o={base.group(3)} n={base.group(4)} last={base.group(5)}",
              flush=True)

        for ch in a.bytes.encode():
            prev_n = int(base.group(4))
            p.baudrate = 9_600
            time.sleep(0.05)
            p.write(bytes([ch]))
            p.flush()
            time.sleep(0.1)
            p.baudrate = 115_200
            p.reset_input_buffer()  # drop garbled-while-9600 bytes
            su = read_su(p, 6.0)
            if su is None:
                print(f"0x{ch:02X} '{chr(ch)}': [su] VANISHED", flush=True)
                results.append(False)
                continue
            got_n = int(su.group(4))
            got_last = int(su.group(5), 16)
            ok = got_n == prev_n + 1 and got_last == ch
            print(f"0x{ch:02X} '{chr(ch)}': n {prev_n}->{got_n} "
                  f"last={su.group(5)} err={su.group(2)} "
                  f"{'OK' if ok else 'MISMATCH'}", flush=True)
            results.append(ok)
            base = su
    finally:
        p.baudrate = 115_200
        p.close()

    good = sum(results)
    print(f"\n=== SOFTUART CHECK: {good}/{len(results)} bytes verified ===")
    return 0 if results and good == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
