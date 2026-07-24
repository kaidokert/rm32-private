#!/usr/bin/env python3
"""rm32 benchuart smoke test — arm, brief low-throttle spin, stop.

Wire: FTDI on COM41 @2M — TX->J3 S (PA2 = USART2 RX), RX->TX1 (PB6 = 2M
debug/info TX). Protocol = minz UartDuty (digits+terminator, s/w/i keys).

Kill guards (bench rule): 'w' on EVERY exit path; abort if reported
current exceeds --max-ma.
"""
import argparse
import sys
import time

import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--pct", type=int, default=5, help="throttle percent for the spin")
ap.add_argument("--spin-s", type=float, default=3.0)
ap.add_argument("--max-ma", type=int, default=800, help="abort threshold from 'i' line")
args = ap.parse_args()


def drain(p, dur=0.3):
    out = b""
    t0 = time.time()
    while time.time() - t0 < dur:
        b = p.read(4096)
        if b:
            out += b
    return out.decode(errors="replace")


def info(p):
    p.write(b"i")
    p.flush()
    txt = drain(p, 0.4)
    line = next((l for l in txt.splitlines() if "[i]" in l), "")
    print("  ", line or f"(no [i] in {len(txt)}B)")
    ma = None
    if "i_ma=" in line:
        try:
            ma = int(line.split("i_ma=")[1].split()[0])
        except ValueError:
            pass
    return line, ma


p = serial.Serial(args.port, args.baud, timeout=0.05)
try:
    print("== arm: zero throttle for 1.6s (ARMING_TIMEOUT = 1s @20kHz)")
    t0 = time.time()
    while time.time() - t0 < 1.6:
        p.write(b"0\n")
        p.flush()
        time.sleep(0.1)
    info(p)

    print(f"== spin: {args.pct}% for {args.spin_s}s")
    t0 = time.time()
    while time.time() - t0 < args.spin_s:
        p.write(f"{args.pct}\n".encode())
        p.flush()
        time.sleep(0.25)
        _, ma = info(p)
        if ma is not None and ma > args.max_ma:
            print(f"!! current {ma}mA > {args.max_ma}mA — aborting")
            break

    print("== stop (s)")
    p.write(b"s\n")
    p.flush()
    time.sleep(0.5)
    info(p)
finally:
    # kill guard: all-off + zero on every exit path
    try:
        p.write(b"w")
        p.flush()
        time.sleep(0.2)
        p.close()
    except Exception:
        pass
    print("== kill sent (w), port closed")
