#!/usr/bin/env python3
"""Per-ISR duration histogram capture for am32_clone ('H' key, 2026-07-26).

The CPU-margin CONTROL instrument for the rm32 `ten_khz_tick` A/B:
rm32's control ISR runs ~14 µs mean with a 15% tail at 25-32 µs (its
1 kHz PID block every 20th tick); the clone has NO PID block, so its
TIM6 tick should stay tight. This tool grabs the clone's three per-ISR
histograms so the tail can be compared bin-by-bin against rm32's.

Sends 'H' on the UART_DUTY_MODE link; the clone dumps:

    HG shift=8 nbins=16
    TIM6  <16 decimal counts, space-separated>
    TIM16 <16 counts>
    COMP  <16 counts>
    HG END

BIN LAW (must match rm32 byte-for-byte): idx = min(delta_cyc >> 8, 15),
so bin b covers [b*256, (b+1)*256) DWT cycles = [b*3.2, (b+1)*3.2) µs at
80 MHz. Bin 0 = underflow (<3.2 µs); bin 15 = overflow (>=48 µs).

Renders captures/hist_<tag>.png (3 stacked log-y bar charts, one per
ISR) and prints a per-ISR bin table with mean-ish and tail% (bins >=8,
i.e. >=25.6 µs).

NOTE: the clone's UartDuty parser takes RAW single characters (no
doubled-key EMI guard). Send 'H' once.

Usage:
    python scripts/hist_grab.py --tag idle
    python scripts/hist_grab.py --pct 40 --settle 4 --kill --tag t40
"""

import argparse
import pathlib
import re
import sys
import time

import serial

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

CPU_HZ = 80_000_000.0
SHIFT = 8
NBINS = 16
CYCLES_PER_BIN = 1 << SHIFT            # 256
US_PER_BIN = CYCLES_PER_BIN / CPU_HZ * 1e6   # 3.2 us
TAIL_BIN = 8                           # bins >= 8 => >= 25.6 us
ROWS = ("TIM6", "TIM16", "COMP")

HDR_RE = re.compile(rb"HG shift=(\d+) nbins=(\d+)")


def capture(ser, timeout=3.0):
    """Send 'H', read until 'HG END'. Returns {name: [16 counts]}."""
    ser.reset_input_buffer()
    paced_write(ser, b"H")
    buf = b""
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        buf += ser.read(65536)
        if b"HG END" in buf:
            break
    m = HDR_RE.search(buf)
    if not m:
        raise RuntimeError("no HG header seen (wire tail: %r)" % buf[-200:])
    shift, nbins = int(m.group(1)), int(m.group(2))
    if shift != SHIFT or nbins != NBINS:
        raise RuntimeError(f"bin law mismatch: dump shift={shift} nbins={nbins} "
                           f"(expected {SHIFT}/{NBINS})")
    body = buf[m.end():buf.index(b"HG END")]
    rows = {}
    for name in ROWS:
        rm = re.search(name.encode() + rb"((?:\s+\d+){%d})" % NBINS, body)
        if not rm:
            raise RuntimeError(f"row {name} not found in dump")
        rows[name] = [int(x) for x in rm.group(1).split()]
    return rows


def bin_label(b):
    lo = b * US_PER_BIN
    if b == NBINS - 1:
        return f">={lo:.1f}us"
    return f"{lo:.1f}-{lo + US_PER_BIN:.1f}us"


def summarize(counts):
    total = sum(counts)
    # mean-ish: sum(bin_mid * count) / total; overflow uses its lower edge.
    acc = 0.0
    for b, c in enumerate(counts):
        mid = (b + 0.5) * US_PER_BIN if b < NBINS - 1 else b * US_PER_BIN
        acc += mid * c
    mean = acc / total if total else 0.0
    tail = sum(counts[TAIL_BIN:])
    tailpct = 100.0 * tail / total if total else 0.0
    return total, mean, tailpct


def render(rows, tag):
    cap = pathlib.Path(__file__).resolve().parent.parent / "captures"
    cap.mkdir(exist_ok=True)
    x = np.arange(NBINS)
    xt = x * US_PER_BIN

    fig, axes = plt.subplots(3, 1, figsize=(12, 9), sharex=True)
    colors = {"TIM6": "#4477aa", "TIM16": "#ee6677", "COMP": "#228833"}
    for ax, name in zip(axes, ROWS):
        counts = rows[name]
        total, mean, tailpct = summarize(counts)
        ax.bar(x, np.array(counts, dtype=float) + 0.1, width=0.85,
               color=colors[name], log=True)
        ax.set_ylabel("count (log)")
        ax.set_title(f"{name}: total={total} mean~{mean:.1f}us "
                     f"tail(>= {TAIL_BIN * US_PER_BIN:.1f}us)={tailpct:.2f}%")
        ax.grid(alpha=0.3, axis="y")
    axes[-1].set_xticks(x)
    axes[-1].set_xticklabels([f"{v:.0f}" for v in xt])
    axes[-1].set_xlabel(f"ISR duration (us, {US_PER_BIN:.1f} us/bin; "
                        f"last bin = overflow >= {(NBINS - 1) * US_PER_BIN:.1f}us)")
    fig.suptitle(f"am32_clone per-ISR duration histogram - {tag}")
    fig.tight_layout()
    png = cap / f"hist_{tag}.png"
    fig.savefig(png, dpi=120)
    print(f"saved {png}")


def print_table(rows):
    for name in ROWS:
        counts = rows[name]
        total, mean, tailpct = summarize(counts)
        print(f"\n== {name} ==  total={total}  mean~{mean:.1f}us  "
              f"tail(>= {TAIL_BIN * US_PER_BIN:.1f}us)={tailpct:.2f}%")
        for b, c in enumerate(counts):
            pct = 100.0 * c / total if total else 0.0
            print(f"  bin{b:2d} {bin_label(b):>12}  {c:>10}  {pct:5.2f}%")
        if total < 1000:
            print(f"  WARN: only {total} samples in {name} - hold longer for a "
                  f"stable tail estimate")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--tag", default="grab")
    ap.add_argument("--pct", type=int, default=None,
                    help="set throttle percent before capturing")
    ap.add_argument("--settle", type=float, default=3.0,
                    help="seconds to hold --pct before 'H'")
    ap.add_argument("--kill", action="store_true",
                    help="send '0\\n' + 'w' on every exit path")
    a = ap.parse_args()

    ser = serial.Serial(a.port, a.baud, timeout=0.05)
    try:
        if a.pct is not None:
            paced_write(ser, f"{a.pct}\n".encode())
            # re-send halfway through settle (deadman zeroes throttle
            # after ~3 s of silence).
            time.sleep(a.settle / 2)
            paced_write(ser, f"{a.pct}\n".encode())
            time.sleep(a.settle / 2)
        rows = capture(ser)
        print_table(rows)
        render(rows, a.tag)
    finally:
        # Motor-script kill-guard (house rule): kill on every exit path
        # whenever the motor was (or may have been) commanded.
        if a.kill or a.pct is not None:
            try:
                paced_write(ser, b"0\n")
                time.sleep(0.2)
                paced_write(ser, b"w")
            except Exception:
                pass
        ser.close()


if __name__ == "__main__":
    main()
