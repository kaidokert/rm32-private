#!/usr/bin/env python3
"""Capture a sine-mode notch sweep from scope.rs.

Sequence:
    0  -> capture notch off baseline
    q  -> reset
    1  -> capture A notch
    q  -> reset
    2  -> capture B notch
    q  -> reset
    3  -> capture C notch
    q  -> reset
    0  -> capture notch off again

Each capture is run in sine mode at the requested electrical Hz and amplitude.
The script writes one raw UART log per capture and one combined PNG with stacked
plots for comparison.

Usage:
    python scripts/scope_uart_notch_sweep.py COM41 140 20
"""

from __future__ import annotations

import argparse
import sys
import time
from datetime import datetime
from pathlib import Path

import matplotlib.pyplot as plt

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_uart_dump import (
    BAUD,
    CHANNEL_PLOT_OFFSET,
    FRAME_HZ,
    FULL_SCALE,
    capture_dump,
    parse_dump,
    ramp_amplitude,
    ramp_frequency,
    read_available,
    send_key,
)


SWEEP = [
    ("off_pre", "0"),
    ("A", "1"),
    ("B", "2"),
    ("C", "3"),
    ("off_post", "0"),
]


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("port", help="UART port, e.g. COM41 or /dev/ttyUSB0")
    p.add_argument("hz", type=float, help="target electrical Hz, same value shown by scope.rs")
    p.add_argument("amp", type=float, help="target amplitude percent, e.g. 20")
    p.add_argument("--settle", type=float, default=1.0, help="seconds to wait before each dump")
    p.add_argument("--out-dir", type=Path, default=Path("logs"), help="output directory")
    p.add_argument("--baud", type=int, default=BAUD, help=f"UART baud rate (default: {BAUD})")
    p.add_argument("--show", action="store_true", help="open the matplotlib window after saving")
    return p.parse_args()


def capture_one(
    ser: serial.Serial,
    notch_key: str,
    target_hz: int,
    amp: float,
    settle: float,
) -> str:
    log = []
    log.append(send_key(ser, "q"))
    log.append(send_key(ser, "m"))
    log.append(ramp_amplitude(ser, amp))
    log.append(ramp_frequency(ser, target_hz))
    log.append(send_key(ser, notch_key))
    print(f"  notch={notch_key} settling {settle:g}s...")
    time.sleep(settle)
    log.append(capture_dump(ser))
    return "".join(log)


def plot_combined(captures: list[tuple[str, list[list[int]], list[str], float]], out_png: Path) -> None:
    fig, axes = plt.subplots(len(captures), 1, figsize=(14, 2.7 * len(captures)), sharex=True)
    if len(captures) == 1:
        axes = [axes]

    for ax, (name, channels, labels, sample_hz) in zip(axes, captures):
        max_len = max(len(ch) for ch in channels)
        t_ms = [i / sample_hz * 1e3 for i in range(max_len)]

        for ch_i, (vals, label) in enumerate(zip(channels, labels)):
            offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
            shifted = [v + offset for v in vals]
            suffix = f" +{offset}" if offset else ""
            ax.plot(t_ms[: len(vals)], shifted, lw=0.8, label=f"{label}{suffix}")

        for ch_i, (vals, label) in enumerate(zip(channels, labels)):
            offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
            sat = [i for i, v in enumerate(vals) if v == FULL_SCALE]
            if sat:
                ax.scatter(
                    [t_ms[i] for i in sat],
                    [vals[i] + offset for i in sat],
                    s=8,
                    zorder=3,
                    label=f"{label} 0xff ({len(sat)})",
                )

        flat = [v for ch in channels for v in ch]
        ax.set_title(
            f"{name}: {len(channels[0])} frames | "
            f"min={min(flat)} max={max(flat)} mean={sum(flat) / len(flat):.1f}"
        )
        ax.set_ylabel("ADC")
        ax.set_ylim(-5, 260 + CHANNEL_PLOT_OFFSET * max(0, len(channels) - 1))
        ax.grid(True, alpha=0.3)
        ax.legend(loc="upper right", fontsize="small")

    axes[-1].set_xlabel(f"time (ms) @ {FRAME_HZ / 1e3:.1f} kframes/s")
    fig.tight_layout()
    fig.savefig(out_png, dpi=120)

    if captures:
        plt.close(fig)


def main() -> int:
    args = parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    target_hz = int(round(args.hz / 10.0) * 10)
    if target_hz < 10:
        raise SystemExit(f"target electrical frequency rounds to {target_hz} Hz; too low")

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base = f"scope_sine_notch_sweep_hz{target_hz}_amp{args.amp:g}_{stamp}"
    out_png = args.out_dir / f"{base}.png"

    print(
        f"port={args.port} mode=sine sweep electrical_hz={args.hz:g}, "
        f"rounded={target_hz} Hz; amp={args.amp:g}%"
    )

    captures = []
    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        ser.reset_output_buffer()
        read_available(ser, max_s=0.5)

        for name, key in SWEEP:
            print(f"capture {name}...")
            text = capture_one(ser, key, target_hz, args.amp, args.settle)
            out_log = args.out_dir / f"{base}_{name}.log"
            out_log.write_text(text, encoding="ascii", errors="replace")

            channels, expected, labels, sample_hz = parse_dump(text)
            parsed = len(channels[0]) if channels else 0
            if expected is not None and expected != parsed:
                print(f"  warning: header said {expected} frames/samples, parsed {parsed}")
            flat = [v for ch in channels for v in ch]
            sat = sum(1 for v in flat if v == FULL_SCALE)
            print(
                f"  parsed {parsed} frames x {len(channels)} channel(s) | "
                f"min={min(flat)} max={max(flat)} mean={sum(flat) / len(flat):.1f} | 0xff={sat}"
            )
            print(f"  wrote {out_log}")
            captures.append((name, channels, labels, sample_hz))

    plot_combined(captures, out_png)
    print(f"wrote {out_png}")

    if args.show:
        plt.show()

    return 0


if __name__ == "__main__":
    sys.exit(main())
