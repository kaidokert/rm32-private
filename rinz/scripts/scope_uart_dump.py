#!/usr/bin/env python3
"""Drive examples/scope.rs over UART, dump ADC samples, and plot them.

Usage:
    python scripts/scope_uart_dump.py COM41 120 12

That means:
    - open COM41 at 115200 8N1
    - set electrical frequency to 120 Hz (rounded to nearest 10 Hz)
    - set amplitude to 12%
    - reset scope.rs with 'q'
    - ramp amplitude from 8.0% using keys
    - ramp frequency from 60 Hz using 'f'/'v'
    - send 'd', capture the dump, and write both .log and .png
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from datetime import datetime
from pathlib import Path

import matplotlib.pyplot as plt

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc


BAUD = 115_200
AMP_START_TENTHS = 80
FREQ_START_HZ = 60
FREQ_STEP_HZ = 10
SAMPLE_HZ = 20_000
FRAME_HZ = 20_000
FULL_SCALE = 255
VREF = 3.3
CHANNEL_PLOT_OFFSET = 4


def read_available(ser: serial.Serial, idle_s: float = 0.15, max_s: float = 2.0) -> str:
    """Read until the UART has been idle for idle_s or max_s expires."""
    deadline = time.monotonic() + max_s
    idle_deadline = time.monotonic() + idle_s
    chunks: list[bytes] = []

    while time.monotonic() < deadline:
        n = ser.in_waiting
        if n:
            chunks.append(ser.read(n))
            idle_deadline = time.monotonic() + idle_s
        elif time.monotonic() >= idle_deadline:
            break
        else:
            time.sleep(0.01)

    return b"".join(chunks).decode("ascii", errors="replace")


def send_key(ser: serial.Serial, key: str, delay_s: float = 0.04) -> str:
    ser.write(key.encode("ascii"))
    ser.flush()
    time.sleep(delay_s)
    return read_available(ser, idle_s=0.04, max_s=0.4)


def send_keys(ser: serial.Serial, key: str, count: int) -> str:
    out = []
    for _ in range(count):
        out.append(send_key(ser, key))
    return "".join(out)


def ramp_amplitude(ser: serial.Serial, target_percent: float) -> str:
    target = int(round(target_percent * 10))
    delta = target - AMP_START_TENTHS
    out = []

    full_steps, tenths = divmod(abs(delta), 10)
    if delta >= 0:
        out.append(send_keys(ser, "a", full_steps))
        out.append(send_keys(ser, "+", tenths))
    else:
        out.append(send_keys(ser, "z", full_steps))
        out.append(send_keys(ser, "-", tenths))

    return "".join(out)


def ramp_frequency(ser: serial.Serial, target_hz: int) -> str:
    delta = target_hz - FREQ_START_HZ
    steps = abs(delta) // FREQ_STEP_HZ
    return send_keys(ser, "f" if delta >= 0 else "v", steps)


def capture_dump(ser: serial.Serial, timeout_s: float = 10.0) -> str:
    ser.write(b"d")
    ser.flush()

    deadline = time.monotonic() + timeout_s
    chunks: list[bytes] = []
    while time.monotonic() < deadline:
        n = ser.in_waiting
        if n:
            chunk = ser.read(n)
            chunks.append(chunk)
            text = b"".join(chunks).decode("ascii", errors="replace")
            if "end" in text:
                return text
        else:
            time.sleep(0.01)

    text = b"".join(chunks).decode("ascii", errors="replace")
    raise TimeoutError(f"timed out waiting for dump end marker; captured {len(text)} chars")


def parse_dump(text: str) -> tuple[list[list[int]], int | None, list[str], float]:
    lines = text.splitlines()
    try:
        start = next(
            i
            for i, line in enumerate(lines)
            if line.lstrip().startswith("dump:") or line.lstrip().startswith("dump3:")
        )
    except StopIteration:
        raise SystemExit("no dump header found in captured text")

    header = lines[start].lstrip()
    is_dump3 = header.startswith("dump3:")
    m = re.search(r"dump3?:\s*(\d+)", header)
    expected = int(m.group(1)) if m else None

    body = []
    for line in lines[start + 1 :]:
        body.append(line)
        if "end" in line:
            break

    blob = " ".join(body).replace("end", " ")
    vals = [int(tok, 16) for tok in blob.split() if re.fullmatch(r"[0-9a-fA-F]{2}", tok)]

    if not is_dump3:
        return [vals], expected, ["ch17"], SAMPLE_HZ

    usable = len(vals) - (len(vals) % 3)
    frames = vals[:usable]
    channels = [frames[i::3] for i in range(3)]
    return channels, expected, ["ch17", "ch5", "ch14"], FRAME_HZ


def plot_dump(channels: list[list[int]], labels: list[str], sample_hz: float, title: str, out_png: Path) -> None:
    max_len = max(len(ch) for ch in channels)
    t_ms = [i / sample_hz * 1e3 for i in range(max_len)]

    fig, ax = plt.subplots(figsize=(14, 5))
    for ch_i, (vals, label) in enumerate(zip(channels, labels)):
        offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
        shifted = [v + offset for v in vals]
        suffix = f" +{offset}" if offset else ""
        ax.plot(t_ms[: len(vals)], shifted, lw=0.8, label=f"{label}{suffix} (8-bit)")

    sat_total = 0
    for ch_i, (vals, label) in enumerate(zip(channels, labels)):
        offset = ch_i * CHANNEL_PLOT_OFFSET if len(channels) > 1 else 0
        sat = [i for i, v in enumerate(vals) if v == FULL_SCALE]
        sat_total += len(sat)
        if sat:
            ax.scatter(
                [t_ms[i] for i in sat],
                [vals[i] + offset for i in sat],
                s=14,
                zorder=3,
                label=f"{label} 0xff ({len(sat)})",
            )

    ax.set_title(title)
    ax.set_xlabel(f"time (ms) @ {sample_hz / 1e3:.1f} kframes/s")
    ax.set_ylabel("ADC value (top 8 of 12 bits)")
    ax.set_ylim(-5, 260 + CHANNEL_PLOT_OFFSET * max(0, len(channels) - 1))
    ax.grid(True, alpha=0.3)
    ax.legend(loc="upper right")

    ax2 = ax.twinx()
    ax2.set_ylim(-5 / FULL_SCALE * VREF, 260 / FULL_SCALE * VREF)
    ax2.set_ylabel(f"~ voltage (V, Vref={VREF})")

    fig.tight_layout()
    fig.savefig(out_png, dpi=120)
    plt.close(fig)


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
    base = f"scope_hz{target_hz}_amp{args.amp:g}_{stamp}"
    out_log = args.out_dir / f"{base}.log"
    out_png = args.out_dir / f"{base}.png"

    print(
        f"port={args.port} electrical_hz={args.hz:g}, rounded={target_hz} Hz; "
        f"amp={args.amp:g}%"
    )

    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        ser.reset_output_buffer()

        log = []
        log.append(read_available(ser, max_s=0.5))
        log.append(send_key(ser, "q"))
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
    sat = sum(1 for v in flat if v == FULL_SCALE)
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
