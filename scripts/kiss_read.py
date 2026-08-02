#!/usr/bin/env python3
"""Read + validate KISS telemetry frames from the ESC's PB6 UART.

Runs against the PRODUCTION build (no debuguart — PB6 carries KISS
telemetry, same wire/adapter/baud as the debug UART). The ESC must
have telemetry_on_interval set (AM32 periodic mode, ~30 ms cadence),
so no FC telemetry-request bit is needed.

Frame (10 bytes, rm32::telemetry::make_telem_package == KISS ESC
standard): temp i8 | voltage cV u16BE | current cA u16BE |
consumption mAh u16BE | eRPM/100 u16BE | CRC8 (poly 0x07, MSB-first).

Hunts frame alignment by sliding a 10-byte window until CRCs validate
repeatedly. Reports rate + decoded values.

Usage: kiss_read.py [--port COM41] [--seconds 6]
"""
import argparse
import sys
import time

import serial


def crc8(buf):
    crc = 0
    for b in buf:
        crc ^= b
        for _ in range(8):
            crc = ((crc << 1) ^ 0x07) & 0xFF if crc & 0x80 else (crc << 1) & 0xFF
    return crc


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--seconds", type=float, default=6.0)
    a = ap.parse_args()

    p = serial.Serial(a.port, 115_200, timeout=0.05)
    try:
        p.reset_input_buffer()
        t0 = time.time()
        buf = b""
        while time.time() - t0 < a.seconds:
            buf += p.read(4096)
    finally:
        p.close()

    frames = []
    i = 0
    while i + 10 <= len(buf):
        cand = buf[i:i + 10]
        if crc8(cand[:9]) == cand[9]:
            temp = cand[0] if cand[0] < 128 else cand[0] - 256
            volt = (cand[1] << 8) | cand[2]
            curr = (cand[3] << 8) | cand[4]
            cons = (cand[5] << 8) | cand[6]
            erpm = (cand[7] << 8) | cand[8]
            frames.append((temp, volt, curr, cons, erpm))
            i += 10
        else:
            i += 1

    n = len(frames)
    print(f"bytes={len(buf)} frames={n} "
          f"rate={n / a.seconds:.1f}/s (expect ~33/s at interval=1)")
    if not frames:
        print("NO VALID FRAMES")
        return 1
    for f in frames[:3] + frames[-3:]:
        temp, volt, curr, cons, erpm = f
        print(f"  temp={temp}C volt={volt / 100:.2f}V curr={curr / 100:.2f}A "
              f"mAh={cons} eRPM={erpm * 100}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
