#!/usr/bin/env python3
"""OWL: shadow-lock report from a MAGPIE v3 capture.

Captures the window-record stream (or reads --infile) and prints the
lockability verdict table:

  - per sector: qualified-ZC rate, qZC position mean±sd,
    prediction-error mean±sd (µs and % of window)
  - overall: interval stability, chain continuity (consecutive-qZC
    rate), the go/no-go numbers for FALCON.

The firmware estimator predicts each commutation as
qZC + smoothed_interval/2 and reports (actual − predicted) per window;
this script just aggregates.

Usage:
    python scripts/owl_report.py --secs 8 --out captures/owl_f100.bin
    python scripts/owl_report.py --infile captures/owl_f100.bin
"""

import argparse
import pathlib
import statistics
import sys
import time

from magpie import PRED_NONE, parse_frames, raw_to_ma, seq_gaps

sys.stdout.reconfigure(errors="replace")

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--baud", type=int, default=2_000_000)
ap.add_argument("--secs", type=float, default=8.0)
ap.add_argument("--out", default=None)
ap.add_argument("--infile", default=None)
ap.add_argument("--t0", type=float, default=None,
                help="analysis window start, seconds from first frame")
ap.add_argument("--t1", type=float, default=None,
                help="analysis window end, seconds from first frame")
ap.add_argument("--tail", type=float, default=None,
                help="analyze only the last N seconds (slip-hunt split)")
args = ap.parse_args()

if args.infile:
    data = pathlib.Path(args.infile).read_bytes()
else:
    import serial

    with serial.Serial(args.port, args.baud, timeout=0.05) as p:
        p.reset_input_buffer()
        p.write(b"g")
        buf = bytearray()
        end = time.monotonic() + args.secs
        while time.monotonic() < end:
            buf += p.read(65536)
        p.write(b"g")
        time.sleep(0.3)
        buf += p.read(65536)
    data = bytes(buf)
    if args.out:
        pathlib.Path(args.out).write_bytes(data)
        print(f"saved {len(data)} bytes to {args.out}")

frames = parse_frames(data)
if not frames:
    sys.exit("no frames")

# Time-split (the sector-slip discriminator): frames carry `start`
# (10 µs ticks); select a slice relative to the capture's frame
# timeline. Wrapping is irrelevant at ladder timescales (12 h wrap).
t_first = frames[0]["start"]
t_last = frames[-1]["start"]
if args.tail is not None:
    args.t0 = (t_last - t_first) / 1e5 - args.tail
    args.t1 = None
if args.t0 is not None or args.t1 is not None:
    lo = t_first + int((args.t0 or 0) * 1e5)
    hi = t_first + int(args.t1 * 1e5) if args.t1 is not None else t_last + 1
    frames = [f for f in frames if lo <= f["start"] <= hi]
    if not frames:
        sys.exit("no frames in the selected window")
    print(f"[slice {((frames[0]['start'] - t_first) / 1e5):.1f}s.."
          f"{((frames[-1]['start'] - t_first) / 1e5):.1f}s of "
          f"{(t_last - t_first) / 1e5:.1f}s]")

lens = [f["len_us"] for f in frames]
mean_len = statistics.mean(lens)
print(
    f"{len(frames)} windows, {seq_gaps(frames)} gaps, "
    f"len={mean_len:.0f}us (f~{1e6 / (6 * mean_len):.0f} Hz), "
    f"i={raw_to_ma(statistics.mean([f['i_avg'] for f in frames])):.0f}mA"
)

print(f"{'sec':>3} {'n':>5} {'qzc%':>5} {'qzc_off':>12} {'pred_err (us)':>16} {'err/win':>8} {'i mA':>6}")
all_err = []
for s in range(6):
    fs = [f for f in frames if f["sector"] == s]
    if not fs:
        continue
    q = [f["qzc_off_us"] for f in fs if f["qzc_off_us"] != 0xFFFF]
    e = [f["pred_err_us"] for f in fs if f["pred_err_us"] != PRED_NONE]
    all_err += e
    line = f"{s:>3} {len(fs):>5} {100 * len(q) / len(fs):>4.0f}%"
    if len(q) >= 2:
        line += f" {statistics.mean(q):>6.0f}±{statistics.stdev(q):<5.0f}"
    else:
        line += f" {'--':>12}"
    if len(e) >= 2:
        m, sd = statistics.mean(e), statistics.stdev(e)
        line += f" {m:>+8.0f}±{sd:<6.0f} {100 * sd / mean_len:>6.1f}%"
    else:
        line += f" {'--':>16} {'--':>8}"
    line += f" {raw_to_ma(statistics.mean([f['i_avg'] for f in fs])):>6.0f}"
    print(line)

if len(all_err) >= 2:
    m, sd = statistics.mean(all_err), statistics.stdev(all_err)
    pred_rate = 100 * len(all_err) / len(frames)
    print(
        f"\nOVERALL: prediction on {pred_rate:.0f}% of windows, "
        f"err {m:+.0f}±{sd:.0f}us = bias {100 * m / mean_len:+.1f}% / "
        f"jitter {100 * sd / mean_len:.1f}% of window"
    )
    verdict = "GO" if sd < 0.15 * mean_len and pred_rate > 60 else "NOT YET"
    print(f"FALCON verdict @ this operating point: {verdict} "
          f"(gate: jitter <15% of window, prediction rate >60%)")
