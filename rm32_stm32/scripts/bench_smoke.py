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


MA_PER_RAW = 3300.0 / 4095.0 / 30.0 * 1000.0  # minz sense cal, ~26.9 mA/count
MV_PER_RAW = 3300.0 / 4095.0 * 9.33  # ~7.52 mV/count
INFO_RE = __import__("re").compile(
    r"i step=\d+ old=(\d+) run=(\d+) ci=\d+ avg=(\d+) "
    r"zc=\d+ duty=(\d+) iraw=(\d+) vbat=(\d+)"
)


def info(p):
    """Poll 'i' and parse with the minz map_sweep.py/fly.py regex."""
    p.write(b"i")
    p.flush()
    txt = drain(p, 0.4)
    m = None
    for m in INFO_RE.finditer(txt):
        pass
    if not m:
        print(f"   (no info match in {len(txt)}B)")
        return "", None
    old, run, avg, duty, iraw, vbat = (int(m.group(k)) for k in range(1, 7))
    fe = 2e6 / (6 * avg) if avg else 0.0
    ma = iraw * MA_PER_RAW
    print(
        f"   {'RUN' if run else 'off'}{' poll' if old else ''}"
        f" {fe:6.0f} Hz {ma:6.0f} mA {vbat * MV_PER_RAW:5.0f} mV duty {duty:4d}"
    )
    return m.group(0), ma


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
