#!/usr/bin/env python3
"""GECKO-scope UNDER LOAD: engage the closed loop, hold, and fire a `G`
on-demand oversample dump while the motor is locked and drawing
current — so the microscope is validated on real load current, not the
idle ~0 A baseline. (A real >4 A monster autopsy needs the amp ~42+
regime; this proves the loaded pipeline at whatever rung the bench
holds.) Reuses the cl_lock_map engage recipe and gecko.py's renderer.

Usage:
    python scripts/gecko_load.py --amp 14 --tag loadshape
"""

import argparse
import pathlib
import re
import sys
import time

import serial

import gecko  # parse_gecko + render

AMP_ENGAGE = 15


class Bench:
    def __init__(self, p, log):
        self.p = p
        self.log = open(log, "wb")
        self.adv = None

    def drain(self):
        d = self.p.read(65536)
        if d:
            self.log.write(d)
        return d.decode("ascii", errors="replace")

    def send(self, key, wait=0.25):
        self.drain()
        self.p.write(key.encode())
        time.sleep(wait)
        return self.drain()

    def steps(self, key, n, wait=0.15):
        for _ in range(n):
            self.send(key, wait)

    def set_advance(self, target):
        adv = self.adv
        for _ in range(40):
            if adv == target:
                self.adv = adv
                return
            key = "t" if (adv is not None and adv < target) else "T"
            m = re.search(r"advance = (-?\d+)", self.send(key, 0.25))
            if m:
                adv = int(m.group(1))
        sys.exit(f"advance set failed at {adv}")

    def set_filters(self):
        last = ""
        for _ in range(6):
            last = self.send(",")
        for _ in range(8):
            last = self.send("n")
        if "blank window = 8" not in last:
            sys.exit(f"blank set failed: {last!r}")
        for _ in range(7):
            if "phys ZC" in self.send("k"):
                return
        sys.exit("edge mode set failed")

    def engage(self):
        self.set_advance(0)
        self.set_filters()
        for attempt in range(4):
            self.send("w", 0.5)
            self.send("q", 3.0)
            self.steps("f", 5, wait=0.2)
            time.sleep(2.0)
            if "ARMED" not in self.send("y", 3.0):
                continue
            time.sleep(1.0)
            echo = self.send("i", 1.0)
            if "cl: ACTIVE" in echo:
                return True
            print(f"  engage attempt {attempt}: not active, retry", flush=True)
        return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--amp", type=int, default=14)
    ap.add_argument("--tag", default="loadshape")
    args = ap.parse_args()

    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    capdir.mkdir(exist_ok=True)
    stamp = time.strftime("%H%M%S")
    stem = f"{args.tag}_{stamp}"

    with serial.Serial(args.port, args.baud, timeout=0.05) as p:
        b = Bench(p, capdir / f"{stem}_session.log")
        try:
            if not b.engage():
                sys.exit("engage failed 4x (bench may be too warm to lock)")
            # climb from AMP_ENGAGE to the target amp
            b.steps("a", max(0, args.amp - AMP_ENGAGE), wait=0.2)
            time.sleep(1.0)
            echo = b.send("i", 1.0)
            m = re.search(r"isns=(\d+\.\d+)A", echo)
            print(f"locked, amp~{args.amp}, isns={m.group(1) if m else '?'}A — firing G", flush=True)
            # Fire G and capture the gecko dump under load.
            b.drain()
            p.write(b"G")
            deadline = time.time() + 6.0
            buf = b""
            while time.time() < deadline:
                c = p.read(65536)
                if c:
                    buf += c
                    if b"gecko:" in buf and b"end" in buf:
                        break
            text = buf.decode("ascii", errors="replace")
        finally:
            b.send("w", 0.3)  # kill on every exit

    (capdir / f"{stem}.txt").write_text(text)
    vals, adc_hz = gecko.parse_gecko(text)
    print(f"parsed {len(vals)} current samples @ {adc_hz:.0f} Hz")
    gecko.render(vals, adc_hz, stem, str(capdir / f"{stem}.png"))


if __name__ == "__main__":
    main()
