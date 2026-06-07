#!/usr/bin/env python3
"""Like scope_uart_dump.py, but switches scope.rs to sine mode before capture.

Usage:
    python scripts/scope_uart_dump_sine.py COM41 120 12
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
    capture_dump,
    parse_dump,
    plot_dump,
    ramp_amplitude,
    ramp_frequency,
    read_available,
    send_key,
)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser()
    p.add_argument("port", help="UART port, e.g. COM41 or /dev/ttyUSB0")
    p.add_argument("hz", type=float, help="target electrical Hz, same value shown by scope.rs")
    p.add_argument("amp", type=float, help="target amplitude percent, e.g. 12 or 12.5")
    p.add_argument("--settle", type=float, default=1.0, help="seconds to wait before dumping (default: 1.0)")
    p.add_argument("--out-dir", type=Path, default=Path("logs"), help="output directory (default: logs)")
    p.add_argument("--baud", type=int, default=BAUD, help=f"UART baud rate (default: {BAUD})")
    p.add_argument("--show", action="store_true", help="open the matplotlib window after saving")
    return p.parse_args()


def main() -> int:
    args = parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    target_hz = int(round(args.hz / 10.0) * 10)
    if target_hz < 10:
        raise SystemExit(f"target electrical frequency rounds to {target_hz} Hz; too low")

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    base = f"scope_sine_hz{target_hz}_amp{args.amp:g}_{stamp}"
    out_log = args.out_dir / f"{base}.log"
    out_png = args.out_dir / f"{base}.png"

    print(
        f"port={args.port} mode=sine electrical_hz={args.hz:g}, "
        f"rounded={target_hz} Hz; amp={args.amp:g}%"
    )

    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        ser.reset_output_buffer()

        log = []
        log.append(read_available(ser, max_s=0.5))
        log.append(send_key(ser, "q"))
        log.append(send_key(ser, "m"))
        log.append(ramp_amplitude(ser, args.amp))
        log.append(ramp_frequency(ser, target_hz))

        print(f"settling {args.settle:g}s...")
        time.sleep(args.settle)
        log.append(capture_dump(ser))

    text = "".join(log)
    out_log.write_text(text, encoding="ascii", errors="replace")

    channels, expected, labels, sample_hz = parse_dump(text)
    parsed = len(channels[0]) if channels else 0
    if expected is not None and expected != parsed:
        print(f"warning: header said {expected} frames/samples, parsed {parsed}")
    if not channels or not channels[0]:
        raise SystemExit("no samples parsed from dump")

    flat = [v for ch in channels for v in ch]
    sat = sum(1 for v in flat if v == 255)
    print(
        f"parsed {parsed} frames/samples x {len(channels)} channel(s) | "
        f"min={min(flat)} max={max(flat)} mean={sum(flat) / len(flat):.1f} | 0xff={sat}"
    )

    plot_dump(channels, labels, sample_hz, f"{base}: {parsed} frames/samples", out_png)
    print(f"wrote {out_log}")
    print(f"wrote {out_png}")

    if args.show:
        plt.show()

    return 0


if __name__ == "__main__":
    sys.exit(main())
