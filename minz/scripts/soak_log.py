#!/usr/bin/env python3
"""Sustained-hold soak logger for the clone jitter head-to-head.

Stages up to --pct, holds it (deadman keepalives), and polls the info
line every --interval s, logging (t, vbat, late, dsy, killed). The
`late=` field is the drop-proof late-window counter (commutations whose
ZC-to-ZC interval ran >=1.5x avg). Purpose: map the clone's late-window
rate vs rail voltage at 100% as the pack droops under load — the fork
against rm32's storm-vs-rail curve. Direct per-window metric, one
same-block run, no onset-time confound.

Usage: python scripts/soak_log.py --pct 100 --secs 45
"""
import argparse
import pathlib
import re
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

FIELDS = {
    "vbat": rb"vbat=(\d+)",
    "late": rb"late=(\d+)",
    "dsy": rb"dsy=(\d+)",
    "killed": rb"killed=(\d+)",
    "comm": rb"comm=(\d+)",
}


def read_info(ser, timeout=0.6):
    ser.reset_input_buffer()
    paced_write(ser, b"i")
    buf, out, t0 = b"", {}, time.monotonic()
    while time.monotonic() - t0 < timeout:
        buf += ser.read(8192)
        out = {k: int(m.group(1)) for k, r in FIELDS.items()
               if (m := re.search(r, buf))}
        if len(out) == len(FIELDS):
            break
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--pct", type=int, default=100)
    ap.add_argument("--secs", type=float, default=45.0)
    ap.add_argument("--interval", type=float, default=2.5)
    a = ap.parse_args()

    ser = serial.Serial(a.port, a.baud, timeout=0.05)
    # trace off so the info line isn't starved at high throttle.
    for _ in range(3):
        if read_info(ser).get("comm") is not None:
            break
        paced_write(ser, b"Z")
        time.sleep(0.3)
    try:
        # gentle staged climb (clone won't cold-arm at high throttle).
        for s in [x for x in (20, 40, 60, 80, a.pct) if x <= a.pct]:
            for _ in range(3):
                paced_write(ser, f"{s}\n".encode())
                time.sleep(0.4)
        base = read_info(ser)
        late0 = base.get("late", 0)
        print(f"armed {a.pct}% — vbat={base.get('vbat')} late0={late0} "
              f"comm={base.get('comm')}")
        print(f"{'t':>6} {'vbat':>6} {'late':>7} {'d_late':>7} {'dsy':>5} {'kill':>5}")
        t0, last_ka, last_late, last_poll = time.monotonic(), 0.0, late0, 0.0
        while time.monotonic() - t0 < a.secs:
            if time.monotonic() - last_ka > 0.8:
                paced_write(ser, f"{a.pct}\n".encode())
                last_ka = time.monotonic()
            if time.monotonic() - last_poll > a.interval:
                d = read_info(ser)
                lt = d.get("late", last_late)
                print(f"{time.monotonic() - t0:6.1f} {d.get('vbat', 0):6d} "
                      f"{lt:7d} {lt - last_late:7d} {d.get('dsy', 0):5d} "
                      f"{d.get('killed', 0):5d}", flush=True)
                last_late = lt
                last_poll = time.monotonic()
                if d.get("killed"):
                    print("  killed — stopping")
                    break
            time.sleep(0.05)
    finally:
        paced_write(ser, b"0\n")
        time.sleep(0.2)
        paced_write(ser, b"w")
        ser.close()


if __name__ == "__main__":
    main()
