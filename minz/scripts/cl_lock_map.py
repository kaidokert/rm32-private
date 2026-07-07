#!/usr/bin/env python3
"""Closed-loop lock map: steady-state capture at each throttle setting.

Arms open loop (q → blank=20/phys_ZC → amp 10 → f=100), engages the
closed loop (`y`), then for each amp in --amps: set it, settle,
stream MAGPIE for --secs, save captures/<tag>_a{amp}.bin. A desync
or trip marks the point BROKEN and the loop is re-engaged for the
next point. Renders with plot_lock_map.py.

Usage:
    python scripts/cl_lock_map.py --amps 9,10,11,12,13,14,15,16
"""

import argparse
import pathlib
import sys
import time

import serial

from magpie import parse_frames

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--amps", default="9,10,11,12,13,14,15,16")
ap.add_argument("--secs", type=float, default=4.0)
ap.add_argument("--settle", type=float, default=3.0)
ap.add_argument("--tag", default="lockmap")
args = ap.parse_args()

amps = [int(a) for a in args.amps.split(",")]
capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

AMP_START, AMP_ENGAGE = 15, 10


class Bench:
    """All received bytes are appended to a session log — nothing is
    ever discarded, so async kill messages (DESYNC/STARVED/TRIP) can't
    vanish into a buffer reset like they did in the first version of
    this script."""

    def __init__(self, p, log_path):
        self.p = p
        self.log = open(log_path, "wb")

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

    def check_active(self):
        echo = self.send("i", 1.2)
        return "cl: ACTIVE" in echo, echo

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

    def steps(self, key, n, wait=0.15):
        for _ in range(n):
            self.send(key, wait)

    def engage(self, cur_amp):
        """Arm open loop at the engage point and switch to CL.
        Returns the current amp after engagement (AMP_ENGAGE)."""
        self.send("q", 3.0)
        self.steps("z", AMP_START - AMP_ENGAGE)
        self.steps("f", 5)
        time.sleep(2.0)
        echo = self.send("y", 2.5)
        if "ARMED" not in echo:
            sys.exit(f"engage failed: {echo!r}")
        return AMP_ENGAGE

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
        cur = b.engage(AMP_START)

        for amp in amps:
            if amp >= cur:
                b.steps("a", amp - cur)
            else:
                b.steps("z", cur - amp)
            cur = amp
            time.sleep(args.settle)
            active, echo = b.check_active()
            if not active:
                print(f"amp {amp:2d}: LOOP NOT ACTIVE before capture - aborting"
                      f" (see session log). Last echo:\n{echo.strip()}", flush=True)
                break
            data = b.capture(args.secs)
            (capdir / f"{args.tag}_a{amp}.bin").write_bytes(data)
            broke = b"DESYNC" in data or b"STARVED" in data or b"TRIP" in data
            frames = parse_frames(data)
            note = "BROKE" if broke else "locked"
            qzc_pct = 0.0
            if frames:
                import statistics

                lens = [f["len_us"] for f in frames if f["len_us"] > 0]
                q = sum(1 for f in frames if f["qzc_off_us"] != 0xFFFF)
                qzc_pct = 100 * q / len(frames)
                note += (
                    f"  f_e={1e6 / (6 * statistics.mean(lens)):.0f}Hz"
                    f"  qzc={qzc_pct:.0f}%  ({len(frames)} win)"
                )
            # A lock that isn't seeing ZCs is a stall/zombie even if
            # nothing tripped (the rotor lies; the coverage doesn't).
            # Kill and STOP — no blind re-engagement into an unknown
            # mechanical state.
            if not broke and frames and qzc_pct < 50:
                print(f"amp {amp:2d}: {note}  << BLIND (qzc<50%) - aborting sweep", flush=True)
                break
            print(f"amp {amp:2d}: {note}", flush=True)
            if broke:
                print("   breakage reported by firmware - stopping sweep", flush=True)
                break
    finally:
        b.send("y", 0.4)
        b.send("w", 0.5)
print("lock map sweep done")
