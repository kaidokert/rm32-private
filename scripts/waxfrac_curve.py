#!/usr/bin/env python3
"""rm32 post-ZC-fraction vs throttle curve — the yardstick match to the
clone's 27/39/40%.

The clone (holds lock to 100%) sits at ~40% post-ZC, uniform across all
six steps. rm32 at its lock-loss read 83%. This builds rm32's own curve
at throttles where it still holds lock, to answer: is the late-
commutation bias present everywhere (constant offset) or does it only
appear at the top (throttle-dependent trigger)? Per-step fractions test
the clone's uniformity claim against rm32's period-2 bias.

Live-ring dump (not the freeze path): at a steady hold the ring's last
~52 ms is representative. 'x' dumps AND re-arms, so a settle dwell
before each dump refills the ring.
"""
import argparse
import subprocess
import sys
import time

import serial
from wax_decode import parse, decode

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--throttles", default="40,50,60,70,80")
    ap.add_argument("--outdir", default="captures")
    a = ap.parse_args()
    throttles = [int(x) for x in a.throttles.split(",")]

    import os
    os.makedirs(a.outdir, exist_ok=True)
    subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
                   capture_output=True)
    time.sleep(2.5)
    p = serial.Serial(a.port, 2_000_000, timeout=0.02)

    def hold(pct, secs):
        t0 = time.time()
        while time.time() - t0 < secs:
            p.write(f"{pct}\n".encode()); p.flush(); time.sleep(0.08)

    def wx(secs=4.0):
        p.reset_input_buffer()
        p.write(b"x"); p.flush()
        buf = bytearray(); t0 = time.time()
        while time.time() - t0 < secs:
            c = p.read(200000)
            if c:
                buf += c
                if b"WX END" in bytes(buf[-400000:]):
                    break
        return bytes(buf)

    def trace_rate(pct):
        """0x5B-record rate over 0.6s while keeping throttle alive."""
        p.reset_input_buffer(); t0 = time.time(); buf = bytearray()
        while time.time() - t0 < 0.6:
            p.write(f"{pct}\n".encode()); p.flush()
            buf += p.read(100000); time.sleep(0.08)
        return len(buf)

    def ensure_trace_off(pct):
        """zct trace floods the WX dump; toggle it off (keep throttle
        alive so the deadman can't stop the motor mid-check)."""
        for _ in range(4):
            if trace_rate(pct) < 3000:
                return True
            p.write(b"Z"); p.flush(); time.sleep(0.15)
        return trace_rate(pct) < 3000

    rows = []
    try:
        hold(0, 1.0)
        for pct in (20, 25, 30, 40):     # patient spin-up
            hold(pct, 3.0)
        p.write(b"J"); p.flush(); time.sleep(0.2)  # arm WAXWING
        if not ensure_trace_off(40):
            print("WARN: could not silence zct trace; dumps may be dirty")
        for pct in throttles:
            hold(pct, 3.0)               # settle
            ensure_trace_off(pct)        # re-verify quiet before dump
            hold(pct, 1.5)               # refill ring after any toggles
            raw = wx()
            fn = f"{a.outdir}/waxfrac_{pct}.bin"
            open(fn, "wb").write(raw)
            txt = raw.decode("latin1")
            import re
            m = re.search(r"frozen=(\d+)", txt)
            frozen = int(m.group(1)) if m else -1
            if "WX n=" not in txt or "WX END" not in txt:
                rows.append((pct, 0, frozen, -1, {}, 0))
                print(f"  {pct}%: no/partial WX dump ({len(raw)}B) -- skipped")
                continue
            try:
                head, ci, arr, fr, tuples = parse(fn)
                s = decode(tuples, head)
                overall = sum(x[4] for x in s) / len(s) if s else 0
                perstep = {}
                for st in range(1, 7):
                    ss = [x[4] for x in s if x[3] == st]
                    perstep[st] = (sum(ss) / len(ss)) if ss else 0
                rows.append((pct, ci, frozen, overall, perstep, len(s)))
            except Exception as e:
                rows.append((pct, 0, frozen, -1, {}, 0))
                print(f"  {pct}%: decode error {e}")
    finally:
        for x in (60, 40, 25, 15):
            hold(x, 0.4)
        p.write(b"w"); p.flush(); time.sleep(0.3)
        p.close()

    print("\n== rm32 post-ZC fraction vs throttle (clone yardstick: 27/39/40%) ==")
    print(f"{'thr':>4} {'ci_us':>6} {'froz':>4} {'postZC%':>8}   per-step 1..6")
    for (pct, ci, frozen, overall, perstep, n) in rows:
        ps = " ".join(f"{perstep.get(st,0)*100:3.0f}" for st in range(1, 7))
        note = "  <-LOCK-LOSS(frozen)" if frozen == 1 else ""
        print(f"{pct:>4} {ci*0.5:>6.0f} {frozen:>4} {overall*100:>7.1f}%   {ps}{note}")
    print("\nclone: ~40% uniform across steps @100%. rm32 late = higher fraction.")


if __name__ == "__main__":
    sys.exit(main())
