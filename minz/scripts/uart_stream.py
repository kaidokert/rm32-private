#!/usr/bin/env python3
"""Capture and parse the MAGPIE window-record stream from motor_tester2.

Sends `g` (stream on), captures for --secs, sends `g` again (off),
then parses the 16-byte frames and prints per-sector statistics.
Raw captured bytes (binary frames + any interleaved ASCII) are saved
to --out for offline plotting.

Frame layout (little-endian):
    [0]  0x5A  sync
    [1]  0xA5  sync
    [2]  seq (u8, wraps)
    [3]  bit7 = zc_found, bits0-3 = sector (0..5)
    [4:8]   window start, 10 µs ticks (u32)
    [8:10]  window length, 10 µs ticks (u16)
    [10:12] first-valid-ZC offset from window start, µs (u16, 0xFFFF = none)
    [12:14] raw edge count (u16)
    [14:16] gate-surviving edge count (u16)

Usage:
    python scripts/uart_stream.py --secs 8 --out cap.bin
"""

import argparse
import statistics
import time

import serial

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
                    start=int.from_bytes(f[4:8], "little"),
                    len_10us=int.from_bytes(f[8:10], "little"),
                    zc_off_us=int.from_bytes(f[10:12], "little"),
                    raw=int.from_bytes(f[12:14], "little"),
                    valid=int.from_bytes(f[14:16], "little"),
                )
            )
            i += 16
        else:
            i += 1
    return frames


frames = parse_frames(bytes(data))
if not frames:
    print("NO FRAMES parsed")
    raise SystemExit(1)

gaps = sum(1 for a, b in zip(frames, frames[1:]) if (a["seq"] + 1) % 256 != b["seq"])
print(f"{len(frames)} frames, {gaps} seq gaps")

for s in range(6):
    fs = [f for f in frames if f["sector"] == s]
    if not fs:
        print(f"sector {s}: NO FRAMES")
        continue
    zc = [f["zc_off_us"] for f in fs if f["zc_found"]]
    lens = [f["len_10us"] * 10 for f in fs]
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
    print(line)
