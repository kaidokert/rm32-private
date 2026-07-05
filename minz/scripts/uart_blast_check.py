#!/usr/bin/env python3
"""Verify the motor_tester2 'u' blast: 64 KiB counting pattern.

Usage: uart_blast_check.py [baud] [port]
Sends 'u', reads until the 'uart_test done' marker, verifies every
payload byte and reports the sustained wire rate.
"""

import sys
import time

import serial

BAUD = int(sys.argv[1]) if len(sys.argv) > 1 else 115200
PORT = sys.argv[2] if len(sys.argv) > 2 else "COM41"
PAYLOAD_LEN = 65536
HDR = b"uart_test start len=65536\r\n"
FOOTER = b"uart_test done"

with serial.Serial(PORT, BAUD, timeout=0.2) as p:
    p.reset_input_buffer()
    p.write(b"u")

    data = bytearray()
    t0 = time.perf_counter()
    t_first = t_last = None
    deadline = t0 + 30.0
    while time.perf_counter() < deadline:
        chunk = p.read(65536)
        if chunk:
            now = time.perf_counter()
            if t_first is None:
                t_first = now
            t_last = now
            data += chunk
            if FOOTER in data[-64:]:
                break

if t_first is None:
    print("FAIL: no data received")
    sys.exit(1)

print(f"received {len(data)} bytes in {t_last - t0:.2f}s (first at +{t_first - t0:.2f}s)")

hi = data.find(HDR)
if hi < 0:
    print("FAIL: header not found")
    sys.exit(1)
payload = data[hi + len(HDR) : hi + len(HDR) + PAYLOAD_LEN]
if len(payload) < PAYLOAD_LEN:
    print(f"FAIL: payload truncated - {len(payload)} of {PAYLOAD_LEN}")
    sys.exit(1)

expected = bytes(range(256)) * (PAYLOAD_LEN // 256)
if payload == expected:
    dur = t_last - t_first
    rate = len(data) / dur if dur > 0 else float("inf")
    print(f"sustained ~{rate:,.0f} bytes/s = ~{rate * 10:,.0f} baud effective")
    print(f"PASS: all {PAYLOAD_LEN} payload bytes correct @ {BAUD} baud")
else:
    errs = [(i, payload[i], i % 256) for i in range(PAYLOAD_LEN) if payload[i] != i % 256]
    print(f"FAIL: {len(errs)} byte errors @ {BAUD} baud, first 5: {errs[:5]}")
    sys.exit(1)
