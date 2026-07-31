#!/usr/bin/env python3
"""WAXWING comp-fraction cross-check for am32_clone (2026-07-27).

Reports the normalized post-ZC comparator fraction from a WAXWING
capture, the matched host-side analogue of rm32's battery-wall
comp-fraction dump. The clone packs, in WAX_T1S bit15, a NORMALIZED
post-ZC bit computed IN-FIRMWARE from its own polarity:

    post_zc = (comp2::value() == RISING)   # bit15 of T1S

This is the SAME physical condition as rm32's `output_level != rising`
(both reduce to raw_CSR != rising), but computed with the clone's own
value()/rising so the host does ZERO polarity reasoning — no inversion
can creep into the decode.

Reading:
  - post_zc%% near ~50 %% = HEALTHY: the comparator flips freely inside
    each float window (a locked, well-timed loop crossing mid-window).
  - post_zc%% > ~70 %% = LATE commutation: the comparator sits at the
    post-ZC level for most of the window (rm32 measured 83 %% at its
    battery wall). The clone, holding lock at qzc 95-100 %%, is the
    healthy reference and should land near 50 %%.

Run classification (matches rm32's): runs of >=3 consecutive same-step
records are classified STUCK-HIGH (post_zc=1 the whole run), STUCK-LOW
(post_zc=0 the whole run), or ACTIVE (post_zc flips within the run).

Inputs (one of):
  --grab            capture live over serial (send X), analyze inline.
  <file>            a captures/wax_*.csv (must carry the post_zc column
                    emitted by waxwing_grab.py) OR a raw WX text dump.

Usage:
    python scripts/comp_fraction.py captures/wax_t90.csv
    python scripts/comp_fraction.py --grab --pct 90 --settle 4 --tag t90
    python scripts/comp_fraction.py --grab --tag idle
"""

import argparse
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import numpy as np  # noqa: E402

from waxwing_grab import capture, process  # noqa: E402
from am32_census import paced_write  # noqa: E402

MIN_RUN = 3  # rm32's run threshold: >=3 consecutive same-step records
HEALTHY_PCT = 50.0
LATE_PCT = 70.0


def load_csv(path):
    """Parse a waxwing_grab csv (idx,a,b,pos_halfus,pos_corrected,step,
    post_zc) into ordered step/post_zc arrays. Rows are already
    oldest-first with the torn guard dropped."""
    steps, pzs = [], []
    with open(path) as f:
        header = f.readline().strip().split(",")
        if "post_zc" not in header:
            raise SystemExit(
                f"{path}: no post_zc column — re-grab with the updated "
                "waxwing_grab.py (bit15 post-ZC), or use --grab."
            )
        si = header.index("step")
        pi = header.index("post_zc")
        for line in f:
            parts = line.strip().split(",")
            if len(parts) <= max(si, pi):
                continue
            steps.append(int(float(parts[si])))
            pzs.append(int(float(parts[pi])))
    return np.array(steps, dtype=int), np.array(pzs, dtype=int)


def load_raw(path):
    """Parse a raw WX text dump saved to a file (same wire format the
    clone emits on 'X'). Reuses waxwing_grab's capture-format unpack via
    process()."""
    import re

    buf = open(path, "rb").read()
    m = re.search(rb"WX n=(\d+) head=(\d+) ci=(\d+) arr=(\d+)", buf)
    if not m:
        raise SystemExit(f"{path}: no WX header found")
    n, head, ci, arr = (int(m.group(i)) for i in range(1, 5))
    body = buf[m.end():buf.index(b"WX END")]
    words = [int(t, 16) for t in re.findall(rb"[0-9a-fA-F]{4}", body)][: 4 * n]
    recs = [tuple(words[4 * i: 4 * i + 4]) for i in range(n)]
    d = process(recs, head, arr)
    return d["step"].astype(int), d["post_zc"].astype(int), ci


def classify_runs(steps, pzs):
    """Segment into runs of constant step; classify runs of length
    >=MIN_RUN. Returns per-step dict {step: [stuck_hi, stuck_lo, active]}
    and a totals triple."""
    per_step = {s: [0, 0, 0] for s in range(1, 7)}  # [hi, lo, active]
    totals = [0, 0, 0]
    i, n = 0, len(steps)
    while i < n:
        j = i
        while j < n and steps[j] == steps[i]:
            j += 1
        run_len = j - i
        s = int(steps[i])
        if run_len >= MIN_RUN and 1 <= s <= 6:
            seg = pzs[i:j]
            if seg.all():
                cls = 0  # STUCK-HIGH
            elif not seg.any():
                cls = 1  # STUCK-LOW
            else:
                cls = 2  # ACTIVE
            per_step[s][cls] += 1
            totals[cls] += 1
        i = j
    return per_step, totals


def report(steps, pzs, tag, ci=None):
    n = len(steps)
    if n == 0:
        raise SystemExit("no records to analyze")
    print(f"=== comp-fraction (WAXWING bit15 = post_zc = value()==rising) "
          f"tag={tag} ===")
    if ci is not None:
        print(f"commutation_interval ci={ci} (0.5us ticks)"
              + ("  [WARNING: ci>2000, idle — result meaningless]"
                 if ci > 2000 else ""))
    print(f"records analyzed: {n}\n")

    print("per-step post_zc fraction:")
    print("  step  sector   n   post_zc%")
    for s in range(1, 7):
        sel = steps == s
        cnt = int(sel.sum())
        frac = (pzs[sel].mean() * 100.0) if cnt else float("nan")
        print(f"   {s}      {s - 1}    {cnt:5d}   {frac:6.1f}")

    per_step, totals = classify_runs(steps, pzs)
    print(f"\nrun classification (runs >= {MIN_RUN} consecutive same-step):")
    print("  step  STUCK-HIGH  STUCK-LOW  ACTIVE")
    for s in range(1, 7):
        hi, lo, ac = per_step[s]
        print(f"   {s}      {hi:6d}     {lo:6d}    {ac:5d}")
    hi, lo, ac = totals
    print(f"  all     {hi:6d}     {lo:6d}    {ac:5d}")

    overall = pzs.mean() * 100.0
    if overall <= HEALTHY_PCT + 10:
        verdict = "healthy (~50%)"
    elif overall > LATE_PCT:
        verdict = "LATE commutation (>70% — like rm32's 83% battery wall)"
    else:
        verdict = "elevated (between healthy ~50% and late >70%)"
    print(f"\nVERDICT: overall post_zc = {overall:.1f}%  -> {verdict}")
    print("  reference: healthy ~50% / late >70% "
          "(rm32 battery-wall capture = 83%)")


def grab(args):
    """Live capture: optional --pct/--settle, send X, analyze inline."""
    import serial

    ser = serial.Serial(args.port, args.baud, timeout=0.05)
    try:
        if args.pct is not None:
            paced_write(ser, f"{args.pct}\n".encode())
            # deadman re-send halfway (throttle zeroes after ~3 s silence).
            time.sleep(args.settle / 2)
            paced_write(ser, f"{args.pct}\n".encode())
            time.sleep(args.settle / 2)
        if args.freerun:
            # 'F' turns the rm32-like free-run ADC scan injector ON;
            # hold with keepalives so the disruption develops, then
            # the always-on WAXWING ring captures it.
            paced_write(ser, b"F")
            end = time.monotonic() + args.frdwell
            last = 0.0
            while time.monotonic() < end:
                if args.pct is not None and time.monotonic() - last > 0.8:
                    paced_write(ser, f"{args.pct}\n".encode())
                    last = time.monotonic()
                time.sleep(0.05)
        recs, head, ci, arr = capture(ser)
        print(f"captured {len(recs)} records, head={head}, ci={ci}, arr={arr}")
        d = process(recs, head, arr)
        report(d["step"].astype(int), d["post_zc"].astype(int), args.tag, ci)
    finally:
        # Motor-script kill-guard: kill on every exit path whenever the
        # motor was (or may have been) commanded.
        if args.kill or args.pct is not None:
            try:
                paced_write(ser, b"0\n")
                time.sleep(0.2)
                paced_write(ser, b"w")
            except Exception:
                pass
        ser.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("file", nargs="?",
                    help="captures/wax_*.csv or a raw WX text dump")
    ap.add_argument("--grab", action="store_true",
                    help="capture live over serial and analyze inline")
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--tag", default="grab")
    ap.add_argument("--pct", type=int, default=None,
                    help="set throttle percent before capturing (--grab)")
    ap.add_argument("--settle", type=float, default=3.0,
                    help="seconds to hold --pct before 'X' (--grab)")
    ap.add_argument("--freerun", action="store_true",
                    help="send 'F' to turn on the rm32-like free-run ADC "
                         "injector, hold --frdwell, then capture")
    ap.add_argument("--frdwell", type=float, default=3.0,
                    help="seconds to hold with free-run on before capture")
    ap.add_argument("--kill", action="store_true",
                    help="send '0\\n' + 'w' on every exit path (--grab)")
    args = ap.parse_args()

    if args.grab:
        grab(args)
    elif args.file:
        p = pathlib.Path(args.file)
        if p.suffix == ".csv":
            steps, pzs = load_csv(p)
            report(steps, pzs, args.tag)
        else:
            steps, pzs, ci = load_raw(p)
            report(steps, pzs, args.tag, ci)
    else:
        ap.error("provide a capture file, or use --grab")


if __name__ == "__main__":
    main()
