#!/usr/bin/env python3
"""MUSTANG: open-loop frequency sweep with MAGPIE window-record capture.

Arms the motor (q → six-step, f=50, amp=AMP_START) and, for each target
electrical frequency, captures the window-record stream under one or
more filter configs. Filter state in the firmware is set with *relative*
keys (`.`/`,` blanking, `k` edge-mode cycle), so this script drives them
closed-loop: it reads the key echoes and presses until the firmware
confirms the requested state — robust against state left over from
previous runs.

Usage:
    python scripts/sweep_windows.py --freqs 50,100,200,300,400 --secs 6
"""

import argparse
import pathlib
import re
import statistics
import sys
import time

import serial

from magpie import parse_frames, raw_to_ma, seq_gaps

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--freqs", default="50,100,150,200,250,300,350,400")
ap.add_argument(
    "--amps",
    default=None,
    help="comma list of amplitude %% values (CONDOR 2D mode); "
    "omit for the single-amp legacy sweep at AMP_START",
)
ap.add_argument("--secs", type=float, default=6.0)
ap.add_argument("--settle", type=float, default=2.5)
ap.add_argument(
    "--max-ma",
    type=int,
    default=800,
    help="overcurrent guard: kill + abort the current amp row when the "
    "captured window-mean current exceeds this (a stalled rotor under "
    "open-loop chop is a locked-rotor heater)",
)
ap.add_argument("--tag", default="sweep")
ap.add_argument(
    "--configs",
    default="raw:0:0,filt:20:3",
    help="comma list of name:blank_us:edge_mode",
)
args = ap.parse_args()

freqs = [int(x) for x in args.freqs.split(",")]
configs = []
for tok in args.configs.split(","):
    name, blank, mode = tok.split(":")
    configs.append((name, int(blank), int(mode)))

capdir = pathlib.Path(__file__).resolve().parent.parent / "captures"
capdir.mkdir(exist_ok=True)

EDGE_NAMES = {
    0: "edges = both",
    1: "edges = raw rise",
    2: "edges = raw fall",
    3: "edges = phys ZC",
    4: "edges = phys anti-ZC",
    5: "edges = value-gated",
}


class Bench:
    def __init__(self, p):
        self.p = p

    def send(self, key, wait=0.25):
        self.p.reset_input_buffer()
        self.p.write(key.encode())
        time.sleep(wait)
        return self.p.read(8192).decode("ascii", errors="replace")

    def set_blank_us(self, target):
        # Clamp to 0 with ',' (-10, floor 0), then step up. Verify from
        # the last echo: "blank window = N ..s".
        last = ""
        for _ in range(6):
            last = self.send(",")
        for _ in range(target // 10):
            last = self.send(".")
        m = re.search(r"blank window = (\d+)", last)
        got = int(m.group(1)) if m else -1
        if got != target:
            sys.exit(f"blanking set failed: wanted {target}, echo said {got!r}")

    def set_edge_mode(self, target):
        # Cycle 'k' until the echo names the wanted mode (≤6 presses).
        want = EDGE_NAMES[target]
        for _ in range(7):
            echo = self.send("k")
            if want in echo:
                return
        sys.exit(f"edge mode set failed: never saw {want!r}")

    def set_amp(self, current, target):
        # Relative keys: s/x = ±10, a/z = ±1. Returns the new amp.
        delta = target - current
        keys = (
            "s" * (delta // 10) + "a" * (delta % 10)
            if delta >= 0
            else "x" * (-delta // 10) + "z" * (-delta % 10)
        )
        for k in keys:
            self.send(k, 0.15)
        return target

    def set_freq(self, current, target):
        # Relative keys: f/v = ±10, d/c = ±1. Returns the new freq.
        delta = target - current
        keys = (
            "f" * (delta // 10) + "d" * (delta % 10)
            if delta >= 0
            else "v" * (-delta // 10) + "c" * (-delta % 10)
        )
        for k in keys:
            self.send(k, 0.15)
        return target

    def capture_stream(self, secs):
        self.p.reset_input_buffer()
        self.p.write(b"g")
        data = bytearray()
        end = time.monotonic() + secs
        while time.monotonic() < end:
            data += self.p.read(65536)
        self.p.write(b"g")
        time.sleep(0.3)
        data += self.p.read(65536)
        return bytes(data)


def summarize(label, data):
    """Print the one-line summary; returns mean current in mA (0 if no
    frames) so the caller can run the overcurrent guard."""
    frames = parse_frames(data)
    if not frames:
        print(f"{label}: NO FRAMES")
        return 0.0
    gaps = seq_gaps(frames)
    zc = [f["zc_off_us"] for f in frames if f["zc_found"]]
    lens = [f["len_us"] for f in frames]
    line = (
        f"{label}: {len(frames):5d} win, {gaps} gaps, "
        f"len={statistics.mean(lens):5.0f}us (sd {statistics.stdev(lens):4.0f}), "
        f"zc={100 * len(zc) / len(frames):3.0f}%"
    )
    if len(zc) >= 2:
        line += (
            f", zc_off={statistics.mean(zc):5.0f}us "
            f"(sd {statistics.stdev(zc):4.0f}, gate {statistics.mean(lens) / 2:.0f})"
        )
    i_ma = raw_to_ma(statistics.mean([f["i_avg"] for f in frames]))
    line += f", i={i_ma:4.0f}mA"
    print(line, flush=True)
    return i_ma


AMP_START = 15  # firmware boot/arm default; `q` restores this + f=50

with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    b = Bench(p)
    # The motor MUST die with this script — a crash, Ctrl-C, or task
    # kill mid-sweep must not leave an unattended open-loop drive
    # running (or worse, stalled and heating).
    try:
        if args.amps is None:
            # Legacy single-amp frequency sweep.
            b.send("q", 3.0)  # arm: six-step, f=50, amp=AMP_START
            current_f = 50
            for target in freqs:
                current_f = b.set_freq(current_f, target)
                time.sleep(args.settle)
                for name, blank, mode in configs:
                    b.set_blank_us(blank)
                    b.set_edge_mode(mode)
                    time.sleep(0.3)
                    data = b.capture_stream(args.secs)
                    (capdir / f"{args.tag}_f{target}_{name}.bin").write_bytes(data)
                    i_ma = summarize(f"f={target:3d} {name:4s}", data)
                    if i_ma > args.max_ma:
                        print(f"OVERCURRENT {i_ma:.0f}mA > {args.max_ma}mA — aborting sweep")
                        raise SystemExit(2)
        else:
            # CONDOR 2D grid: amp rows × freq cols, filter config set
            # once. Each amp row re-arms with `q` (also works while
            # armed: resets to six-step / f=50 / amp=AMP_START) so every
            # row starts from a known-spinning state before stepping amp
            # and climbing in f. Overcurrent (stall) aborts the ROW —
            # higher f at the same amp would only stall harder — and
            # moves on to the next amp.
            amps = [int(x) for x in args.amps.split(",")]
            name, blank, mode = configs[0]
            b.send("q", 3.0)
            b.set_blank_us(blank)
            b.set_edge_mode(mode)
            for amp in amps:
                b.send("q", 2.0)
                b.set_amp(AMP_START, amp)
                time.sleep(1.0)
                current_f = 50
                for target in freqs:
                    current_f = b.set_freq(current_f, target)
                    time.sleep(args.settle)
                    data = b.capture_stream(args.secs)
                    (capdir / f"{args.tag}_f{target}_a{amp}_{name}.bin").write_bytes(data)
                    i_ma = summarize(f"a={amp:2d} f={target:3d}", data)
                    if i_ma > args.max_ma:
                        print(
                            f"OVERCURRENT {i_ma:.0f}mA > {args.max_ma}mA — "
                            f"killing, skipping rest of amp={amp} row"
                        )
                        b.send("w", 0.5)
                        break
    finally:
        b.send("w", 0.5)  # kill on ANY exit path
print("sweep done")
