#!/usr/bin/env python3
"""FALCON discriminator probe: WAXWING bursts at setpoints + CL episodes.

Per (f, amp) setpoint: settle in open loop, take --shots `j` bursts.
Then --cl-shots closed-loop episodes at the first setpoint: `y`
(engage), wait ~0.4 s (v2 typically desyncs within a few windows;
the 85 ms ring then holds pre-engage windows, the CL episode, the
desync kill, and coasting BEMF), then `j`, then re-arm.

Captures land in captures/<tag>_*.txt for falcon_stats.py.

Usage:
    python scripts/falcon_probe.py --points 100:10,200:10,300:10 --shots 3
"""

import argparse
import pathlib
import re
import sys
import time

import serial

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--points", default="100:10,200:10,300:10,100:15")
ap.add_argument("--shots", type=int, default=3)
ap.add_argument("--cl-shots", type=int, default=3)
ap.add_argument("--tag", default="probe")
args = ap.parse_args()

points = [tuple(int(x) for x in p.split(":")) for p in args.points.split(",")]
capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

AMP_START = 15


class Bench:
    def __init__(self, p):
        self.p = p

    def send(self, key, wait=0.25):
        self.p.reset_input_buffer()
        self.p.write(key.encode())
        time.sleep(wait)
        return self.p.read(16384).decode("ascii", errors="replace")

    def set_blank_us(self, target):
        last = ""
        for _ in range(6):
            last = self.send(",")
        for _ in range(target // 10):
            last = self.send(".")
        m = re.search(r"blank window = (\d+)", last)
        if not m or int(m.group(1)) != target:
            sys.exit(f"blank set failed: {last!r}")

    def set_edge_mode3(self):
        for _ in range(7):
            if "phys ZC" in self.send("k"):
                return
        sys.exit("edge mode set failed")

    def step(self, plus, minus, ten_plus, ten_minus, cur, target):
        d = target - cur
        keys = (ten_plus * (d // 10) + plus * (d % 10)) if d >= 0 else (
            ten_minus * (-d // 10) + minus * (-d % 10)
        )
        for k in keys:
            self.send(k, 0.15)
        return target

    def burst(self, out: pathlib.Path):
        self.p.reset_input_buffer()
        self.p.write(b"j")
        buf = bytearray()
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            buf += self.p.read(65536)
            if b"\nend" in buf or b"\rend" in buf:
                break
        out.write_bytes(buf)
        return len(buf)


with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    b = Bench(p)
    try:
        b.send("q", 3.0)
        b.set_blank_us(20)
        b.set_edge_mode3()
        cur_f, cur_a = 50, AMP_START

        for f, amp in points:
            cur_f = b.step("d", "c", "f", "v", cur_f, f)
            cur_a = b.step("a", "z", "s", "x", cur_a, amp)
            time.sleep(2.0)
            for k in range(args.shots):
                out = capdir / f"{args.tag}_f{f}_a{amp}_{k}.txt"
                n = b.burst(out)
                print(f"f={f} a={amp} shot {k}: {n} bytes", flush=True)
                time.sleep(0.4)

        # CL episodes at the first setpoint.
        f, amp = points[0]
        cur_f = b.step("d", "c", "f", "v", cur_f, f)
        cur_a = b.step("a", "z", "s", "x", cur_a, amp)
        for k in range(args.cl_shots):
            time.sleep(1.5)
            echo = b.send("y", 0.4)  # engage; v2 usually desyncs fast
            out = capdir / f"{args.tag}_cl_{k}.txt"
            n = b.burst(out)
            tail = b.send("i", 0.5)
            state = "DESYNC" if "DESYNC" in (echo + tail) else (
                "ACTIVE" if "ACTIVE" in tail else "?"
            )
            print(f"CL episode {k}: {n} bytes, state={state}", flush=True)
            # Re-arm open loop for the next episode (y killed on desync;
            # if still active, y disengage-kills first).
            if "ACTIVE" in tail:
                b.send("y", 0.3)
            b.send("q", 3.0)
            cur_f = b.step("d", "c", "f", "v", 50, f)
            cur_a = b.step("a", "z", "s", "x", AMP_START, amp)
    finally:
        b.send("w", 0.5)
print("probe done")
