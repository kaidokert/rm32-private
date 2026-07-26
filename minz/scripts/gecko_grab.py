#!/usr/bin/env python3
"""GECKO on-demand current capture for am32_clone ('G' key, 2026-07-26).

Sends 'G' on the UART_DUTY_MODE link; the clone runs the free-run
current oversample for ~1 ms (one full CUR_RING wrap), freezes, and
dumps the 2048-sample ring as ASCII hex:

    GK n=2048 start=<oldest-slot-index> ci=<commutation_interval>
    <128 lines of 16 space-separated 4-hex-char u16s, raw ring order>
    GK END

This tool reorders from `start=` (oldest sample first), saves
captures/gecko_<tag>.csv (index,raw) and renders
captures/gecko_<tag>.png (raw + mA + smoothed vs time, 0.27 us/sample).

NOTE: the clone's UartDuty parser takes RAW single characters — the
DOUBLED-key convention was motor_tester2's EMI guard, NOT this
protocol. Send 'G' once.

Usage:
    python scripts/gecko_grab.py --tag idle
    python scripts/gecko_grab.py --pct 90 --settle 4 --kill --tag t90
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

# Sense calibration (CLAUDE.md / gecko.py): 12-bit over 3.3 V ->
# 0.806 mV/count; INA1801 (20 V/V) x 1.5 mOhm shunt -> 30 mV/A.
ADC_MV_PER_COUNT = 3300.0 / 4096.0  # ~0.806
ISNS_MV_PER_AMP = 30.0
US_PER_SAMPLE = 0.27  # free-run ch8 rate (~3.7 MHz)
N_EXPECTED = 2048

HDR_RE = re.compile(rb"GK n=(\d+) start=(\d+) ci=(\d+)")


def capture(ser, timeout=5.0):
    """Send 'G', read until 'GK END'. Returns (samples, start, ci)."""
    ser.reset_input_buffer()
    paced_write(ser, b"G")
    buf = b""
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        buf += ser.read(65536)
        if b"GK END" in buf:
            break
    m = HDR_RE.search(buf)
    if not m:
        raise RuntimeError("no GK header seen (wire tail: %r)" % buf[-200:])
    n, start, ci = int(m.group(1)), int(m.group(2)), int(m.group(3))
    body = buf[m.end():buf.index(b"GK END")]
    words = [int(t, 16) for t in re.findall(rb"[0-9a-fA-F]{4}", body)]
    if len(words) < n:
        raise RuntimeError(f"short dump: {len(words)}/{n} words")
    words = words[:n]
    # Reorder oldest-first from the firmware's start index.
    return words[start:] + words[:start], start, ci


def render(samples, tag, ci):
    raw = np.array(samples, dtype=float)
    ma = raw * ADC_MV_PER_COUNT / ISNS_MV_PER_AMP * 1000.0
    t_us = np.arange(len(raw)) * US_PER_SAMPLE
    win = 32
    smooth = np.convolve(ma, np.ones(win) / win, mode="same")

    cap = pathlib.Path(__file__).resolve().parent.parent / "captures"
    cap.mkdir(exist_ok=True)
    csv = cap / f"gecko_{tag}.csv"
    with open(csv, "w") as f:
        f.write("index,raw\n")
        for i, v in enumerate(samples):
            f.write(f"{i},{v}\n")

    fig, ax = plt.subplots(figsize=(14, 5))
    ax.plot(t_us, ma, lw=0.4, color="#88aadd", label="current (mA)")
    ax.plot(t_us, smooth, lw=1.4, color="#cc3333", label=f"smoothed (n={win})")
    ax.set_xlabel(f"time (us, {US_PER_SAMPLE} us/sample)")
    ax.set_ylabel("current (mA)")
    ax2 = ax.twinx()
    ax2.set_ylim(np.array(ax.get_ylim()) * ISNS_MV_PER_AMP / ADC_MV_PER_COUNT / 1000.0)
    ax2.set_ylabel("raw (12-bit counts)")
    ax.set_title(f"GECKO {tag}: ci={ci} (0.5us ticks), "
                 f"min/max/mean raw {raw.min():.0f}/{raw.max():.0f}/{raw.mean():.1f}")
    ax.legend(loc="upper right")
    ax.grid(alpha=0.3)
    png = cap / f"gecko_{tag}.png"
    fig.tight_layout()
    fig.savefig(png, dpi=120)
    print(f"saved {csv}")
    print(f"saved {png}")
    print(f"raw  min={raw.min():.0f} max={raw.max():.0f} mean={raw.mean():.1f}")
    print(f"mA   min={ma.min():.0f} max={ma.max():.0f} mean={ma.mean():.1f}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--tag", default="grab")
    ap.add_argument("--pct", type=int, default=None,
                    help="set throttle percent before capturing")
    ap.add_argument("--settle", type=float, default=3.0,
                    help="seconds to hold --pct before 'G'")
    ap.add_argument("--kill", action="store_true",
                    help="send '0\\n' + 'w' on every exit path")
    a = ap.parse_args()

    ser = serial.Serial(a.port, a.baud, timeout=0.05)
    try:
        if a.pct is not None:
            paced_write(ser, f"{a.pct}\n".encode())
            # re-send halfway through settle (clone_starts arm pattern:
            # the deadman zeroes throttle after 3 s of silence).
            time.sleep(a.settle / 2)
            paced_write(ser, f"{a.pct}\n".encode())
            time.sleep(a.settle / 2)
        samples, start, ci = capture(ser)
        print(f"captured {len(samples)} samples, start={start}, ci={ci}")
        render(samples, a.tag, ci)
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
