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

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--freqs", default="50,100,150,200,250,300,350,400")
ap.add_argument("--secs", type=float, default=6.0)
ap.add_argument("--settle", type=float, default=2.5)
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


def parse_frames(buf: bytes):
    frames = []
    i = 0
    while i + 16 <= len(buf):
        if buf[i] == 0x5A and buf[i + 1] == 0xA5 and (buf[i + 3] & 0x0F) < 6:
            f = buf[i : i + 16]
            frames.append(
                dict(
                    seq=f[2],
                    zc_found=bool(f[3] & 0x80),
                    sector=f[3] & 0x0F,
                    len_us=int.from_bytes(f[8:10], "little") * 10,
                    zc_off_us=int.from_bytes(f[10:12], "little"),
                    raw=int.from_bytes(f[12:14], "little"),
                    valid=int.from_bytes(f[14:16], "little"),
                )
            )
            i += 16
        else:
            i += 1
    return frames


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
    frames = parse_frames(data)
    if not frames:
        print(f"{label}: NO FRAMES")
        return
    gaps = sum(1 for a, b in zip(frames, frames[1:]) if (a["seq"] + 1) % 256 != b["seq"])
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
    print(line, flush=True)


with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    b = Bench(p)
    b.send("q", 3.0)  # arm: six-step, f=50, amp=AMP_START

    current_f = 50
    for target in freqs:
        delta = target - current_f
        if delta >= 0:
            keys = "f" * (delta // 10) + "d" * (delta % 10)
        else:
            keys = "v" * (-delta // 10) + "c" * (-delta % 10)
        for k in keys:
            b.send(k, 0.15)
        current_f = target
        time.sleep(args.settle)

        for name, blank, mode in configs:
            b.set_blank_us(blank)
            b.set_edge_mode(mode)
            time.sleep(0.3)
            data = b.capture_stream(args.secs)
            (capdir / f"{args.tag}_f{target}_{name}.bin").write_bytes(data)
            summarize(f"f={target:3d} {name:4s}", data)

    b.send("w", 0.5)
print("sweep done")
