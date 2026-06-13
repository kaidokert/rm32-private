#!/usr/bin/env python3
"""Operating-point scanner: map where six-step ZCs land in-window.

For every (electrical hz, amplitude) grid point this script:
  1. sends 'q'  -> firmware resets to 60 Hz / 8.0 % six-step and spins up
  2. ramps amplitude then frequency to the target via key presses
  3. waits --settle seconds for the motor to reach steady state
  4. sends 'd'  -> zero-aligned capture; firmware kills the motor after the dump
  5. parses the dump, runs the ZC analysis, writes a zc png + txt + raw log
  6. records in-window count and ZC placement stats

At the end it prints a summary table and the best amplitude per hz
(most in-window sectors, tiebreak: mean ZC closest to 50 % of the sector).

Amplitude units are the firmware 'amp %' knob; actual PWM duty = amp * 2/3.
Use --duty-list to think in duty terms instead (converted to amp = duty * 3/2).

Example:
    python scripts/zc_scan.py COM41 --hz-list 120,150,180,210 --duty-list 10,12,14,16
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
import sys
import time

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import (
    BAUD,
    capture_dump,
    format_zc_report,
    parse_capture,
    plot_zc_snapshot,
    ramp_amplitude,
    ramp_frequency,
    read_available,
    send_key,
)


@dataclass
class PointResult:
    hz: int
    amp: float
    frames: int = 0
    sectors: int = 0
    in_window: int = 0
    early: int = 0
    missing: int = 0
    mean_pct: float | None = None
    span_pct: float | None = None
    error: str | None = None

    @property
    def duty(self) -> float:
        return self.amp * 2.0 / 3.0

    def row(self) -> str:
        if self.error:
            return (
                f"{self.hz:>5} {self.amp:>6.1f} {self.duty:>6.1f}  ERROR: {self.error}"
            )
        mean = f"{self.mean_pct:5.1f}" if self.mean_pct is not None else "    -"
        span = f"{self.span_pct:5.1f}" if self.span_pct is not None else "    -"
        return (
            f"{self.hz:>5} {self.amp:>6.1f} {self.duty:>6.1f} "
            f"{self.in_window:>3}/{self.sectors:<3} {self.early:>5} {self.missing:>4} "
            f"{mean} {span}"
        )

    def score(self) -> tuple[int, float]:
        if self.error or self.sectors == 0:
            return (-1, -1000.0)
        centeredness = -abs((self.mean_pct if self.mean_pct is not None else 200.0) - 50.0)
        return (self.in_window, centeredness)


def parse_float_list(text: str) -> list[float]:
    return [float(v) for v in text.replace(";", ",").split(",") if v.strip()]


def run_point(ser, hz: int, amp: float, settle_s: float, timeout_s: float, out_dir: Path) -> PointResult:
    res = PointResult(hz=hz, amp=amp)
    try:
        send_key(ser, "q", delay_s=0.1)
        ramp_amplitude(ser, amp)
        ramp_frequency(ser, hz)
        read_available(ser, idle_s=0.1, max_s=1.0)
        time.sleep(settle_s)
        text = capture_dump(ser, timeout_s=timeout_s)
    except Exception as exc:
        res.error = f"{type(exc).__name__}: {exc}"
        return res

    stem = f"hz{hz}_amp{amp:g}"
    (out_dir / f"{stem}_raw.log").write_text(text, encoding="ascii", errors="replace")

    try:
        capture = parse_capture(text)
        res.frames = capture.frames
        sectors = plot_zc_snapshot(capture, out_dir / f"{stem}_zc.png")
        (out_dir / f"{stem}_zc.txt").write_text(format_zc_report(sectors), encoding="ascii")
    except Exception as exc:
        res.error = f"{type(exc).__name__}: {exc}"
        return res

    res.sectors = len(sectors)
    res.in_window = sum(1 for s in sectors if s.status == "zc")
    res.early = sum(1 for s in sectors if s.status == "early")
    res.missing = sum(1 for s in sectors if s.status == "none")
    pcts = [s.zc_pct for s in sectors if s.status == "zc" and s.zc_pct is not None]
    if pcts:
        res.mean_pct = sum(pcts) / len(pcts)
        res.span_pct = max(pcts) - min(pcts)
    return res


HEADER = "   hz   amp%  duty%  in/tot early miss  mean  span"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("port", help="serial port, e.g. COM41")
    parser.add_argument("--baud", type=int, default=BAUD)
    parser.add_argument(
        "--hz-list",
        default="120,140,160,180,200",
        help="comma-separated electrical hz targets (multiples of 10)",
    )
    parser.add_argument(
        "--amp-list",
        default=None,
        help="comma-separated amp %% values (duty = amp*2/3); default 12,15,18,21,24",
    )
    parser.add_argument(
        "--duty-list",
        default=None,
        help="comma-separated PWM duty %% values; converted to amp = duty*3/2 (overrides --amp-list)",
    )
    parser.add_argument("--settle", type=float, default=2.0, help="seconds to wait at the operating point before capture")
    parser.add_argument("--timeout", type=float, default=20.0, help="dump read timeout in seconds")
    parser.add_argument("--out-dir", type=Path, default=None, help="output dir (default logs/scan_<timestamp>)")
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    hz_list = [int(round(v / 10.0) * 10) for v in parse_float_list(args.hz_list)]
    if args.duty_list:
        amp_list = [round(d * 1.5, 1) for d in parse_float_list(args.duty_list)]
    elif args.amp_list:
        amp_list = parse_float_list(args.amp_list)
    else:
        amp_list = [12.0, 15.0, 18.0, 21.0, 24.0]

    out_dir = args.out_dir or Path("logs") / f"scan_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    out_dir.mkdir(parents=True, exist_ok=True)

    total = len(hz_list) * len(amp_list)
    print(f"scanning {len(hz_list)} hz x {len(amp_list)} amp = {total} points -> {out_dir}")
    print(HEADER)

    results: list[PointResult] = []
    with serial.Serial(args.port, args.baud, timeout=0.01) as ser:
        read_available(ser, idle_s=0.1, max_s=1.0)
        try:
            for hz in hz_list:
                for amp in amp_list:
                    res = run_point(ser, hz, amp, args.settle, args.timeout, out_dir)
                    results.append(res)
                    print(res.row(), flush=True)
        finally:
            # Make sure the motor is not left spinning on abort.
            try:
                send_key(ser, "w")
            except Exception:
                pass

    lines = [HEADER]
    lines += [r.row() for r in results]
    lines.append("")
    lines.append("best amp per hz (most in-window, tiebreak mean closest to 50%):")
    for hz in hz_list:
        candidates = [r for r in results if r.hz == hz]
        best = max(candidates, key=lambda r: r.score())
        if best.score()[0] <= 0:
            lines.append(f"  {hz:>5} Hz: no point with in-window ZCs")
        else:
            lines.append(
                f"  {hz:>5} Hz: amp={best.amp:g}% (duty={best.duty:.1f}%) "
                f"-> {best.in_window}/{best.sectors} in-window, mean={best.mean_pct:.1f}%"
            )

    summary = "\n".join(lines) + "\n"
    (out_dir / "summary.txt").write_text(summary, encoding="ascii")
    print()
    print(summary)
    print(f"outputs in {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
