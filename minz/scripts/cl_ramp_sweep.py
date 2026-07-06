#!/usr/bin/env python3
"""FALCON envelope sweep: closed-loop amp ramps at increasing rates.

For each ramp rate: re-arm open loop (q → filters → amp 10 → f 100),
engage (`y`), stream MAGPIE records while ramping amp 10→--amp-max→10
with one `a`/`z` step per --rate ms, then disengage. A desync
auto-kills and dumps the firmware black box into the same capture —
so every breakage arrives pre-diagnosed.

Outputs: captures/<tag>_r{rate}.bin (+ printed table: survive/break,
min interval = max f_e reached, break context).

Usage:
    python scripts/cl_ramp_sweep.py --rates 500,250,120,60,0
"""

import argparse
import pathlib
import re
import statistics
import sys
import time

import serial

from magpie import parse_frames

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--rates", default="500,250,120,60,0", help="ms per 1%% amp step; 0 = instant")
ap.add_argument("--amp-max", type=int, default=16)
ap.add_argument("--hold", type=float, default=1.5, help="s at amp-max")
ap.add_argument("--tag", default="ramp")
args = ap.parse_args()

rates = [int(r) for r in args.rates.split(",")]
capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

AMP_START, AMP_LO = 15, 10


class Bench:
    def __init__(self, p):
        self.p = p

    def send(self, key, wait=0.25):
        self.p.reset_input_buffer()
        self.p.write(key.encode())
        time.sleep(wait)
        return self.p.read(16384).decode("ascii", errors="replace")

    def set_blank20_mode3(self):
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

    def steps(self, key, n, wait=0.15):
        for _ in range(n):
            self.send(key, wait)


def run_ramp(b, p, rate_ms):
    # Fresh arm at the standard operating point.
    b.send("q", 3.0)
    b.steps("z", AMP_START - AMP_LO)
    b.steps("f", 5)
    time.sleep(2.0)
    echo = b.send("y", 2.0)
    if "ARMED" not in echo:
        return dict(result="ENGAGE-FAILED", data=b"")

    # Stream on; from here we interleave timed key writes with reads.
    p.reset_input_buffer()
    p.write(b"g")
    buf = bytearray()

    def pump(until):
        nonlocal buf
        while time.monotonic() < until:
            buf += p.read(65536)
            if b"DESYNC" in buf or b"TRIP" in buf:
                return False
            time.sleep(0.02)
        return True

    ok = pump(time.monotonic() + 1.0)
    plan = []
    step_s = max(rate_ms, 1) / 1000.0 if rate_ms > 0 else 0.05
    for k in ("a" * (args.amp_max - AMP_LO)) + "H" + ("z" * (args.amp_max - AMP_LO)):
        plan.append(k)
    for k in plan:
        if not ok:
            break
        if k == "H":
            ok = pump(time.monotonic() + args.hold)
            continue
        p.write(k.encode())
        ok = pump(time.monotonic() + step_s)
    # settle + drain
    pump(time.monotonic() + 1.0)
    p.write(b"g")
    time.sleep(0.3)
    buf += p.read(262144)

    broke = b"DESYNC" in buf or b"TRIP" in buf
    # Disengage / make safe.
    if not broke:
        b.send("y", 0.5)
    b.send("w", 0.4)
    return dict(result="BROKE" if broke else "SURVIVED", data=bytes(buf))


with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    b = Bench(p)
    try:
        b.send("q", 3.0)
        b.set_blank20_mode3()
        b.send("w", 0.5)

        results = []
        for rate in rates:
            r = run_ramp(b, p, rate)
            out = capdir / f"{args.tag}_r{rate}.bin"
            out.write_bytes(r["data"])
            frames = parse_frames(r["data"])
            line = f"rate {rate:4d} ms/step: {r['result']:9s} {len(frames):5d} win"
            if frames:
                lens = [f["len_us"] for f in frames if f["len_us"] > 0]
                if lens:
                    line += (
                        f", f_e {1e6 / (6 * statistics.mean(lens[:50])):.0f}"
                        f"->{1e6 / (6 * min(lens)):.0f} Hz max"
                    )
            if r["result"] == "BROKE":
                txt = r["data"].decode("ascii", errors="replace")
                bb = re.findall(r"bb \+\s*\d+us \w+ s\d d=\d+", txt)
                line += f", bb events {len(bb)} (tail: {' | '.join(bb[-3:])})"
            print(line, flush=True)
            results.append((rate, r["result"]))
            time.sleep(1.5)
    finally:
        b.send("w", 0.5)

print("sweep done:", ", ".join(f"{r}ms={res}" for r, res in results))
