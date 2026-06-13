#!/usr/bin/env python3
"""Equilibrium chaser: per frequency, search the amp that centers the ZCs.

For each electrical frequency this runs a two-phase adaptive search on the
amplitude knob, capturing and ZC-analyzing at every probe (same artifacts as
zc_scan.py):

  Phase 1 (coarse): secant search on the pooled mean ZC position until it is
  within --tol of the window center, then
  Phase 2 (fine):   sweep +/- --fine-range around the best point in
  --fine-step increments (0.1 % = the firmware knob resolution).

Every probe point is measured --repeats times and the ZC positions pooled —
single captures at one operating point scatter by tens of %-points, so without
repeats a granular search just samples noise.

A point only counts as converged/best when it has at least --min-in-window
pooled in-window crossings AND pooled span <= --max-span. A centered mean with
a huge span is an averaging accident, not an equilibrium.

Control model: in open loop the rotor load angle shrinks as amplitude rises, so
crossings move EARLIER in the float window as amp goes up (d mean_pct / d amp < 0).
The first step uses that assumed sign; afterwards a secant update estimates the
actual local slope, so a wrong sign self-corrects. Captures with nothing
in-window still steer: mostly-early sectors push amp down, mostly-missing
(late) push amp up.

Example:
    python scripts/zc_chase.py COM41 --hz-list 120,180,240,300
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
import re
import sys

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import BAUD, read_available, send_key
from zc_scan import HEADER, PointResult, run_point

# Pseudo-error (in mean_pct percentage points) assigned when no in-window ZC
# exists but the miss direction is known from sector statuses.
EARLY_ERR = -60.0
LATE_ERR = 60.0


def measure(ser, hz: int, amp: float, args, out_dir: Path, repeat_tag: bool) -> PointResult:
    """Capture `--repeats` times at one operating point and pool the results."""
    merged = PointResult(hz=hz, amp=amp)
    for rep in range(args.repeats):
        rep_dir = out_dir / f"rep{rep}" if repeat_tag else out_dir
        rep_dir.mkdir(parents=True, exist_ok=True)
        res = run_point(ser, hz, amp, args.settle, args.timeout, rep_dir)
        if res.error:
            if merged.sectors == 0:
                merged.error = res.error
            continue
        merged.error = None
        merged.frames += res.frames
        merged.sectors += res.sectors
        merged.in_window += res.in_window
        merged.early += res.early
        merged.missing += res.missing
        merged.pcts += res.pcts
    if merged.pcts:
        merged.mean_pct = sum(merged.pcts) / len(merged.pcts)
        merged.span_pct = max(merged.pcts) - min(merged.pcts)
    return merged


def point_error(res: PointResult, target_pct: float) -> float | None:
    """Signed distance of the pooled mean ZC from the target, or a pseudo-error
    encoding the miss direction. None when direction is unknowable."""
    if res.error or res.sectors == 0:
        return None
    if res.mean_pct is not None:
        return res.mean_pct - target_pct
    if res.early > res.missing:
        return EARLY_ERR
    if res.missing > res.early:
        return LATE_ERR
    return None


def is_equilibrium(res: PointResult, err: float | None, args) -> bool:
    return (
        err is not None
        and abs(err) <= args.tol
        and res.in_window >= args.min_in_window
        and res.span_pct is not None
        and res.span_pct <= args.max_span
    )


def chase_key(res: PointResult, target: float):
    """Best-point ordering: most in-window, then tightest span, then centered."""
    if res.error or res.sectors == 0:
        return (-1, -1000.0, -1000.0)
    span = -(res.span_pct if res.span_pct is not None else 999.0)
    centered = -abs((res.mean_pct if res.mean_pct is not None else 999.0) - target)
    return (res.in_window, span, centered)


def propose_amp(
    history: list[tuple[float, float]],
    gain: float,
    max_step: float,
    amp_min: float,
    amp_max: float,
) -> float:
    """Next amp from (amp, err) history: secant when two points are usable,
    proportional bootstrap otherwise. err > 0 means crossing too late -> amp up."""
    amp, err = history[-1]

    step = gain * err
    if len(history) >= 2:
        amp_prev, err_prev = history[-2]
        d_err = err - err_prev
        d_amp = amp - amp_prev
        if abs(d_err) > 1e-6 and abs(d_amp) > 1e-6:
            slope = d_err / d_amp
            step = -err / slope

    step = max(-max_step, min(max_step, step))
    if step == 0.0:
        step = 0.2 if err > 0 else -0.2
    return round(max(amp_min, min(amp_max, amp + step)), 1)


@dataclass
class ChaseResult:
    hz: int
    points: list[PointResult] = field(default_factory=list)
    converged: bool = False
    fine_swept: bool = False

    def best(self, target: float) -> PointResult | None:
        usable = [p for p in self.points if not p.error]
        return max(usable, key=lambda p: chase_key(p, target)) if usable else None


def chase_hz(ser, hz: int, args, out_dir: Path) -> ChaseResult:
    chase = ChaseResult(hz=hz)
    seen: dict[float, PointResult] = {}
    history: list[tuple[float, float]] = []
    repeat_tag = args.repeats > 1

    def probe(amp: float, label: str) -> PointResult:
        res = measure(ser, hz, amp, args, out_dir, repeat_tag)
        seen[amp] = res
        chase.points.append(res)
        err = point_error(res, args.target)
        err_str = f"{err:+6.1f}" if err is not None else "     ?"
        print(f"  {label}: {res.row()}  err={err_str}", flush=True)
        return res

    # --- Phase 1: coarse secant ---
    amp = args.amp_start
    for it in range(args.max_iters):
        if amp in seen:
            break  # converged on the 0.1 % amp grid
        res = probe(amp, f"coarse {it}")
        err = point_error(res, args.target)

        if err is None:
            # No directional information -- widen around the start alternating
            # up/down until something is observable.
            swing = args.amp_start + (it + 1) * args.max_step * (1 if it % 2 == 0 else -1)
            amp = round(max(args.amp_min, min(args.amp_max, swing)), 1)
            continue

        history.append((amp, err))
        if is_equilibrium(res, err, args):
            chase.converged = True
            break
        amp = propose_amp(history, args.gain, args.max_step, args.amp_min, args.amp_max)

    # --- Phase 2: fine sweep around the best coarse point ---
    best = chase.best(args.target)
    if best is not None and not args.no_fine and best.in_window > 0:
        chase.fine_swept = True
        center = best.amp
        offsets = sorted(
            {round(center + k * args.fine_step, 1)
             for k in range(-int(args.fine_range / args.fine_step),
                            int(args.fine_range / args.fine_step) + 1)}
        )
        for amp in offsets:
            if amp in seen or not (args.amp_min <= amp <= args.amp_max):
                continue
            res = probe(amp, "fine    ")
            err = point_error(res, args.target)
            if is_equilibrium(res, err, args):
                chase.converged = True

    return chase


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("port", help="serial port, e.g. COM41")
    parser.add_argument("--baud", type=int, default=BAUD)
    parser.add_argument("--hz-list", default="120,180,240,300")
    parser.add_argument("--amp-start", type=float, default=14.0, help="initial amp %% per hz")
    parser.add_argument("--amp-min", type=float, default=6.0)
    parser.add_argument("--amp-max", type=float, default=28.0)
    parser.add_argument("--target", type=float, default=50.0, help="target mean ZC position (%% of window)")
    parser.add_argument("--tol", type=float, default=8.0, help="convergence tolerance on pooled mean ZC (%%-points)")
    parser.add_argument("--repeats", type=int, default=3, help="captures pooled per amp point")
    parser.add_argument("--min-in-window", type=int, default=4, help="pooled in-window ZCs required for equilibrium")
    parser.add_argument("--max-span", type=float, default=30.0, help="max pooled ZC span (%%-points) for equilibrium")
    parser.add_argument("--gain", type=float, default=0.05, help="bootstrap step: amp %% per %%-point of error")
    parser.add_argument("--max-step", type=float, default=1.5, help="max amp change per coarse iteration (%%)")
    parser.add_argument("--max-iters", type=int, default=6, help="coarse probe budget per frequency")
    parser.add_argument("--fine-range", type=float, default=0.3, help="fine sweep half-width around best amp (%%)")
    parser.add_argument("--fine-step", type=float, default=0.1, help="fine sweep increment (%%)")
    parser.add_argument("--no-fine", action="store_true", help="skip the fine sweep phase")
    parser.add_argument(
        "--dir",
        choices=["fwd", "rev"],
        default=None,
        help="set drive direction before chasing (firmware 'r' toggle; sticky across 'q')",
    )
    parser.add_argument(
        "--revs",
        type=int,
        default=None,
        help="capture length in electrical revs (firmware clamps to DMA buffer: 6 revs fit at 180 Hz)",
    )
    parser.add_argument("--settle", type=float, default=2.0)
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--out-dir", type=Path, default=None)
    return parser.parse_args()


def set_direction(ser, want: str) -> None:
    """Toggle the firmware drive direction until it reports the wanted one."""
    for _ in range(2):
        reply = send_key(ser, "r", delay_s=0.1)
        if f"dir={want}" in reply:
            print(f"drive direction: {want}")
            return
    raise SystemExit(f"could not set dir={want}; firmware replied: {reply!r} (old scope1 build?)")


def set_capture_revs(ser, want: int) -> None:
    """Step the firmware capture-revs setting ('e'/'c') to the wanted value."""
    m = re.search(r"revs=(\d+)", send_key(ser, "e", delay_s=0.1))
    if m is None:
        raise SystemExit("firmware did not report revs= (old scope1 build?)")
    current = int(m.group(1))
    for _ in range(64):
        if current == want:
            print(f"capture revs: {want}")
            return
        key = "e" if current < want else "c"
        m = re.search(r"revs=(\d+)", send_key(ser, key, delay_s=0.05))
        if m is None:
            break
        current = int(m.group(1))
    raise SystemExit(f"could not reach revs={want} (stuck at {current})")


def main() -> int:
    args = parse_args()
    hz_list = [int(round(float(v) / 10.0) * 10) for v in args.hz_list.split(",") if v.strip()]
    out_dir = args.out_dir or Path("logs") / f"chase_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    out_dir.mkdir(parents=True, exist_ok=True)

    per_point = args.repeats
    print(f"chasing equilibrium at {hz_list} Hz, {per_point} captures/point -> {out_dir}")
    print(HEADER)

    chases: list[ChaseResult] = []
    with serial.Serial(args.port, args.baud, timeout=0.01) as ser:
        read_available(ser, idle_s=0.1, max_s=1.0)
        try:
            if args.dir:
                set_direction(ser, args.dir)
            if args.revs:
                set_capture_revs(ser, args.revs)
            for hz in hz_list:
                print(f"hz={hz}:")
                chases.append(chase_hz(ser, hz, args, out_dir))
        finally:
            try:
                send_key(ser, "w")
            except Exception:
                pass

    lines = ["equilibrium per hz:"]
    for chase in chases:
        best = chase.best(args.target)
        tag = "converged" if chase.converged else "best effort"
        captures = sum(args.repeats for _ in chase.points)
        if best is None or best.in_window == 0:
            lines.append(f"  {chase.hz:>5} Hz: no in-window ZCs found ({captures} captures)")
        else:
            lines.append(
                f"  {chase.hz:>5} Hz: amp={best.amp:g}% (duty={best.duty:.1f}%) "
                f"-> {best.in_window}/{best.sectors} in-window, mean={best.mean_pct:.1f}% "
                f"span={best.span_pct:.1f} [{tag}, {captures} captures]"
            )
    lines.append("")
    lines.append("all points (pooled over repeats):")
    lines.append(HEADER)
    for chase in chases:
        lines += [p.row() for p in chase.points]

    summary = "\n".join(lines) + "\n"
    (out_dir / "summary.txt").write_text(summary, encoding="ascii")
    print()
    print(summary)
    print(f"outputs in {out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
