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
    ap.add_argument("--hunt", type=float, default=0.0, help="arm J and hold N s for a >4A auto-trigger spike capture")
    ap.add_argument("--bb", action="store_true", help="press B for an on-demand black-box dump (commutation delays at speed)")
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
            print(f"locked+ACTIVE, amp~{args.amp}, isns={m.group(1) if m else '?'}A", flush=True)
            b.drain()
            if args.bb:
                # On-demand black-box dump at speed, then done.
                b.p.write(b"B")
                t0 = time.time()
                bb = b""
                while time.time() - t0 < 2.0:
                    c = p.read(65536)
                    if c:
                        bb += c
                text = bb.decode("ascii", errors="replace")
                b.send("w", 0.3)
                (capdir / f"{stem}_bb.txt").write_text(text)
                acc = [int(m) for m in re.findall(r"ACC s\d+ d=(\d+)", text)]
                if acc:
                    import statistics
                    print(f"ACC delays (us): n={len(acc)} min={min(acc)} max={max(acc)} "
                          f"mean={statistics.mean(acc):.1f} — {sorted(set(acc))}")
                else:
                    print("no ACC events parsed; raw bb saved")
                return
            if args.hunt:
                # Arm the >4A peak trigger (J turns on the free-run
                # oversample + the intra-cycle peak scan) and HOLD,
                # watching for the firmware's auto-trigger gecko dump on
                # a real spike. Abort on any kill.
                print(f"  arming J, hunting {args.hunt:.0f}s for a >4A spike...", flush=True)
                b.p.write(b"J")
                deadline = time.time() + args.hunt
                buf = b""
                while time.time() < deadline:
                    c = p.read(65536)
                    if c:
                        buf += c
                        if b"gecko:" in buf and b"end" in buf:
                            print("  !! SPIKE CAUGHT — auto-trigger fired", flush=True)
                            # keep reading ~2 s for the trailing bb dump
                            t3 = time.time() + 2.0
                            while time.time() < t3:
                                cc = p.read(65536)
                                if cc:
                                    buf += cc
                            break
                        if b"STARVED" in buf or b"DESYNC" in buf or b"SAG KILL" in buf:
                            print("  (loop killed during hunt — capturing any dump)", flush=True)
                            # keep reading briefly for a trailing dump
                            t2 = time.time() + 1.5
                            while time.time() < t2:
                                c = p.read(65536)
                                if c:
                                    buf += c
                            break
                text = buf.decode("ascii", errors="replace")
                if "gecko:" not in text:
                    print("  no spike caught in window; firing manual G", flush=True)
                    b.drain()
                    b.p.write(b"G")
                    d2 = time.time() + 6.0
                    gb = b""
                    while time.time() < d2:
                        c = p.read(65536)
                        if c:
                            gb += c
                            if b"gecko:" in gb and b"end" in gb:
                                break
                    text = gb.decode("ascii", errors="replace")
            else:
                # Fire G immediately for a baseline shape.
                print("  firing G", flush=True)
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
