#!/usr/bin/env python3
"""Closed-loop commutation-advance sweep at fixed throttle points.

For each amp in --amps: engage at advance 0° (open loop consumes
ADVANCE_DEG too, so engaging at anything else breaks spin-up), then
step advance through the firmware `t` cycle (0/20/40/-40/-20) while
LOCKED, capturing MAGPIE at each point. Breakage (starve/desync/trip)
is recorded, advance restored to 0, loop re-engaged, sweep continues.

Usage:
    python scripts/cl_adv_sweep.py --amps 11,12,13,14,15
"""

import argparse
import pathlib
import re
import statistics
import sys
import time

import serial

from magpie import parse_frames, raw_to_ma

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--amps", default="11,12,13,14,15")
ap.add_argument("--advs", default="0,20,40,-40,-20")
ap.add_argument("--secs", type=float, default=3.0)
ap.add_argument("--settle", type=float, default=1.5)
ap.add_argument("--tag", default="advmap")
args = ap.parse_args()

amps = [int(a) for a in args.amps.split(",")]
advs = [int(a) for a in args.advs.split(",")]
capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

AMP_START, AMP_ENGAGE = 15, 10
ADV_CYCLE = [0, 20, 40, -40, -20]  # firmware `t` order


class Bench:
    def __init__(self, p, log_path):
        self.p = p
        self.log = open(log_path, "wb")
        self.adv = None  # unknown until first set

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

    def set_filters(self):
        last = ""
        for _ in range(6):
            last = self.send(",")
        for _ in range(2):
            last = self.send(".")
        if "blank window = 20" not in last:
            sys.exit(f"blank set failed: {last!r}")
        for _ in range(7):
            if "phys ZC" in self.send("k"):
                return
        sys.exit("edge mode set failed")

    def set_advance(self, target):
        """Cycle `t` until the echo confirms the target (≤6 presses)."""
        for _ in range(6):
            if self.adv == target:
                return
            echo = self.send("t", 0.3)
            m = re.search(r"advance = (-?\d+)", echo)
            if m:
                self.adv = int(m.group(1))
        if self.adv != target:
            sys.exit(f"advance set failed: wanted {target}, at {self.adv}")

    def engage(self):
        """Advance must be 0 for the open-loop spin-up."""
        self.set_advance(0)
        self.send("w", 0.4)
        self.send("q", 3.0)
        self.steps("z", AMP_START - AMP_ENGAGE)
        self.steps("f", 5)
        time.sleep(2.0)
        echo = self.send("y", 2.5)
        if "ARMED" not in echo:
            sys.exit(f"engage failed: {echo!r}")
        time.sleep(1.0)
        return AMP_ENGAGE

    def check_active(self):
        return "cl: ACTIVE" in self.send("i", 1.2)

    def capture(self, secs):
        self.drain()
        self.p.write(b"g")
        buf = bytearray()
        end = time.monotonic() + secs
        while time.monotonic() < end:
            buf += self.p.read(65536)
        self.p.write(b"g")
        time.sleep(0.3)
        buf += self.p.read(65536)
        self.log.write(bytes(buf))
        return bytes(buf)


with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    b = Bench(p, capdir / f"{args.tag}_session.log")
    try:
        b.send("w", 0.5)
        b.send("q", 2.0)
        b.set_filters()
        b.send("w", 0.5)
        cur = b.engage()

        for amp in amps:
            if amp >= cur:
                b.steps("a", amp - cur)
            else:
                b.steps("z", cur - amp)
            cur = amp
            time.sleep(1.5)
            for adv in advs:
                if not b.check_active():
                    print(f"amp {amp:2d} adv {adv:+3d}: loop dead before set - re-engaging", flush=True)
                    cur = b.engage()
                    b.steps("a" if amp >= cur else "z", abs(amp - cur))
                    cur = amp
                    time.sleep(1.5)
                b.set_advance(adv)
                time.sleep(args.settle)
                data = b.capture(args.secs)
                name = f"{args.tag}_a{amp}_adv{'m' if adv < 0 else 'p'}{abs(adv)}"
                (capdir / f"{name}.bin").write_bytes(data)
                broke = b"DESYNC" in data or b"STARVED" in data or b"TRIP" in data
                frames = parse_frames(data)
                note = "BROKE" if broke else "locked"
                if frames:
                    lens = [f["len_us"] for f in frames if f["len_us"] > 0]
                    q = sum(1 for f in frames if f["qzc_off_us"] != 0xFFFF)
                    i_ma = raw_to_ma(statistics.mean([f["i_avg"] for f in frames]))
                    note += (
                        f"  f_e={1e6 / (6 * statistics.mean(lens)):.0f}Hz"
                        f"  qzc={100 * q / len(frames):.0f}%  i={i_ma:.0f}mA"
                    )
                print(f"amp {amp:2d} adv {adv:+3d}: {note}", flush=True)
                if broke:
                    cur = b.engage()
                    b.steps("a" if amp >= cur else "z", abs(amp - cur))
                    cur = amp
                    time.sleep(1.5)
            b.set_advance(0)
    finally:
        b.send("y", 0.4)
        b.send("w", 0.5)
print("advance sweep done")
