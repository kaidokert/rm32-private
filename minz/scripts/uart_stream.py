#!/usr/bin/env python3
"""Capture and parse the MAGPIE window-record stream from motor_tester2.

Sends `g` (stream on), captures for --secs, sends `g` again (off),
then parses the 16-byte frames and prints per-sector statistics.
Raw captured bytes (binary frames + any interleaved ASCII) are saved
to --out for offline plotting.

Frame layout: see scripts/magpie.py (22-byte v2 with current stats).

Usage:
    python scripts/uart_stream.py --secs 8 --out cap.bin
"""

import argparse
import statistics
import time

import serial

from magpie import parse_frames, raw_to_ma, seq_gaps

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--secs", type=float, default=8.0)
ap.add_argument("--out", default=None, help="save raw capture to file")
args = ap.parse_args()

with serial.Serial(args.port, args.baud, timeout=0.05) as p:
    p.reset_input_buffer()
    p.write(b"g")
    data = bytearray()
    end = time.monotonic() + args.secs
    while time.monotonic() < end:
        data += p.read(65536)
    p.write(b"g")
    time.sleep(0.3)
    data += p.read(65536)

if args.out:
    with open(args.out, "wb") as f:
        f.write(data)
    print(f"saved {len(data)} bytes to {args.out}")


frames = parse_frames(bytes(data))
if not frames:
    print("NO FRAMES parsed")
    raise SystemExit(1)

print(f"{len(frames)} frames, {seq_gaps(frames)} seq gaps")

for s in range(6):
    fs = [f for f in frames if f["sector"] == s]
    if not fs:
        print(f"sector {s}: NO FRAMES")
        continue
    zc = [f["zc_off_us"] for f in fs if f["zc_found"]]
    lens = [f["len_us"] for f in fs]
    i_avg = statistics.mean([f["i_avg"] for f in fs])
    i_max = max(f["i_max"] for f in fs)
    line = (
        f"sector {s}: n={len(fs)} len={statistics.mean(lens):.0f}us "
        f"raw={statistics.mean([f['raw'] for f in fs]):.0f} "
        f"valid={statistics.mean([f['valid'] for f in fs]):.0f} "
        f"zc_found={100 * len(zc) / len(fs):.0f}%"
    )
    if len(zc) >= 2:
        line += (
            f" zc_off={statistics.mean(zc):.0f}us "
            f"sd={statistics.stdev(zc):.0f}us "
            f"({100 * statistics.mean(zc) / statistics.mean(lens):.0f}% into window)"
        )
    line += f" i_avg={raw_to_ma(i_avg):.0f}mA i_pk={raw_to_ma(i_max):.0f}mA"
    print(line)
