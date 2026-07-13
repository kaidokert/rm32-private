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

    def enable_swift(self):
        # SWIFT (M) is a stateful toggle — press until the echo says on.
        for _ in range(2):
            if "SWIFT" in self.send("M", 0.4):
                return True
        return False

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
    ap.add_argument("--fast", action="store_true", help="enable SWIFT after engage (needed above ~amp 20)")
    ap.add_argument("--max-ma", type=float, default=2500, help="abort if isns exceeds this")
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
                sys.exit("engage failed 4x")
            if args.fast and not b.enable_swift():
                sys.exit("SWIFT enable failed")
            # Climb in ~4-amp chunks with a 2 s settle per chunk —
            # matches cl_lock_map's proven --fast ladder pacing (SWIFT
            # needs settle time after each amp jump).
            amp = AMP_ENGAGE
            while amp < args.amp:
                nxt = min(args.amp, amp + 4)
                b.steps("a", nxt - amp, wait=0.15)
                amp = nxt
                time.sleep(2.0)
                echo = b.send("i", 0.8)
                if "cl: ACTIVE" not in echo:
                    sys.exit(f"ABORT: lost lock climbing to amp {amp} (not ACTIVE)")
                m = re.search(r"isns=(\d+\.\d+)A", echo)
                cur = float(m.group(1)) if m else 0.0
                print(f"  amp {amp}: isns={cur:.3f}A", flush=True)
                if cur * 1000 > args.max_ma:
                    sys.exit(f"ABORT: isns {cur}A > max-ma")
            echo = b.send("i", 1.0)
            if "cl: ACTIVE" not in echo:
                sys.exit("ABORT: not ACTIVE before G")
            m = re.search(r"isns=(\d+\.\d+)A", echo)
            print(f"locked+ACTIVE, amp~{args.amp}, isns={m.group(1) if m else '?'}A — firing G", flush=True)
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
