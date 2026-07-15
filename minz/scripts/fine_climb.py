#!/usr/bin/env python3
"""Fine duty climb across the spike-onset boundary.

Engages CL, climbs to --amp (coarse %), then steps DUTY_TRIM in raw PWM
counts (`]` = +4 counts; ARR=3332 at 24 kHz so 1 % ~= 33 counts) with a
--dwell streamed capture per step. Counts single-window current spikes
(i_max > baseline_median + 1.5 A) and reports rate + f_e per step — the
spike-onset boundary in counter resolution.

Usage:
    python scripts/fine_climb.py --amp 64 --steps 33 --dwell 2
"""

import argparse
import pathlib
import re
import statistics as st
import sys
import time

import serial

from magpie import parse_frames, raw_to_ma

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
        for _ in range(2):
            if "SWIFT" in self.send("M", 0.4):
                return True
        return False

    def engage(self):
        self.set_advance(0)
        self.set_filters()
        for _ in range(4):
            self.send("w", 0.5)
            self.send("q", 3.0)
            self.steps("f", 5, wait=0.2)
            time.sleep(2.0)
            if "ARMED" not in self.send("y", 3.0):
                continue
            time.sleep(1.0)
            if "cl: ACTIVE" in self.send("i", 1.0):
                return True
        return False

    def stream(self, on):
        want = b"stream=on" if on else b"stream=off"
        buf = bytearray()
        for _ in range(2):
            self.p.write(b"g")
            time.sleep(0.3)
            buf += self.p.read(200000)
            if want in bytes(buf):
                break
        self.log.write(bytes(buf))

    def timed_read(self, secs):
        buf = bytearray()
        end = time.monotonic() + secs
        while time.monotonic() < end:
            buf += self.p.read(65536)
        self.log.write(bytes(buf))
        return bytes(buf)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--amp", type=int, default=64, help="coarse start rung (%)")
    ap.add_argument("--steps", type=int, default=33, help="number of +4-count trim steps")
    ap.add_argument("--dwell", type=float, default=2.0)
    ap.add_argument("--tag", default="fineclimb")
    args = ap.parse_args()

    capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
    capdir.mkdir(exist_ok=True)
    stamp = time.strftime("%H%M%S")
    stem = f"{args.tag}_{stamp}"
    csv = open(capdir / f"{stem}.csv", "w")
    csv.write("trim_counts,duty_counts,f_e_hz,windows,spikes,rate_per_s,med_ma\n")

    base_duty = 3332 * args.amp // 100

    with serial.Serial(args.port, args.baud, timeout=0.05) as p:
        b = Bench(p, capdir / f"{stem}_session.log")
        try:
            if not b.engage():
                sys.exit("engage failed 4x")
            if not b.enable_swift():
                sys.exit("SWIFT enable failed")
            amp = AMP_ENGAGE
            while amp < args.amp:
                nxt = min(args.amp, amp + 4)
                b.steps("a", nxt - amp, wait=0.15)
                amp = nxt
                time.sleep(2.0)
                if "cl: ACTIVE" not in b.send("i", 0.8):
                    sys.exit(f"ABORT: lost lock climbing to amp {amp}")
            print(f"locked at amp {args.amp} (duty ~{base_duty}); fine climb "
                  f"+{args.steps}x4 counts", flush=True)
            b.stream(True)
            baseline_med = None
            for k in range(args.steps + 1):
                if k > 0:
                    b.p.write(b"]")  # +4 counts
                    time.sleep(0.1)
                data = b.timed_read(args.dwell)
                if (b"DESYNC" in data or b"STARVED" in data or b"SAG KILL" in data
                        or b"OVERCURRENT" in data):
                    print(f"  trim +{k*4:3d}: KILLED — boundary is here", flush=True)
                    csv.write(f"{k*4},{base_duty+k*4},,,,KILL,\n")
                    break
                fr = parse_frames(data)
                if not fr:
                    print(f"  trim +{k*4:3d}: no frames", flush=True)
                    continue
                imax = [raw_to_ma(f["i_max"]) for f in fr]
                med = st.median(imax)
                if baseline_med is None:
                    baseline_med = med
                thr = baseline_med + 1500
                # cluster adjacent windows into events
                sp = [i for i, v in enumerate(imax) if v > thr]
                ev = 0
                last = -10
                for i in sp:
                    if i - last > 2:
                        ev += 1
                    last = i
                lens = [f["len_us"] for f in fr if f["len_us"] > 0]
                fe = 1e6 / (6 * st.mean(lens))
                secs = sum(lens) / 1e6 * 5  # decimation x5
                duty = base_duty + k * 4
                print(f"  trim +{k*4:3d} (duty {duty}, {100*duty/3332:5.2f}%): "
                      f"f_e={fe:5.0f}Hz med={med:4.0f}mA spikes={ev:2d} "
                      f"({ev/secs:4.1f}/s)", flush=True)
                csv.write(f"{k*4},{duty},{fe:.0f},{len(fr)},{ev},{ev/secs:.2f},{med:.0f}\n")
        finally:
            b.stream(False)
            b.send("y", 0.4)
            b.send("w", 0.5)
            csv.close()
    print(f"csv: captures/{stem}.csv")


if __name__ == "__main__":
    main()
