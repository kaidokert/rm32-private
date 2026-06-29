#!/usr/bin/env python3
"""Stage-2 experiment: let the HARMONIC drive commutation (bounded by alpha/slew) and measure
whether the honest in-window SIGN-CHANGE coverage rises -- i.e. does closing the loop on a good
detector collapse the open-loop +-48deg per-sector wave that causes the 2/6 sparseness?

  zc=  in the cl-log is now cl.sc_zc() -- the in-window sign-change crossing, captured EVERY
       commutation independent of what drives. THIS is the measurement.
  harm= is the harmonic crossing (the driver here).

Rows: open-loop baseline (alpha 0), then harm-driving at raised alpha. If zc= coverage climbs
from the ~2/6 baseline as the harmonic locks the loop, the wave was an open-loop artifact and
classical six-step is recoverable. If it stays ~2/6, the sparseness is intrinsic.

  python scripts/cl_harm_drive.py COM41 --hz 380 --amp 30 --alphas 0.2 0.5 --snaps 5
ATTENDED. Built for scope_cl2_48k. Leaves the motor killed.
"""
from __future__ import annotations

import argparse
import statistics as st
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD, parse_capture  # noqa: E402
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_alpha, set_detector  # noqa: E402
from cl_harm_check import BUDGET, tally  # noqa: E402


def set_harm_drive(ser, on: bool):
    send(ser, "'")
    drain(ser)


def set_zc_beta(ser, target: float):
    """Floor zc_beta to 0 (many ','), then step up to `target` (each '.' = +0.05)."""
    for _ in range(20):
        send(ser, ",")
    drain(ser)
    for _ in range(int(round(max(0.0, target) / 0.05))):
        send(ser, ".")
    drain(ser)


def measure(ser, snaps, capture_timeout):
    """Median over `snaps` captures: (sc_cov%, harm_cov%, n_per_snap, lock_slow, jit_slow, isr%)."""
    sc = harm = tot = 0
    locks, jits, ics = [], [], []
    for _ in range(snaps):
        try:
            cap = parse_capture(capture(ser, capture_timeout, "c"))
        except Exception as exc:
            print(f"    capture/parse failed: {exc}")
            continue
        n, zc, hm, _both, _rows = tally(cap.text)
        sc += zc
        harm += hm
        tot += n
        d = cap.debug
        locks.append(float(d.get("lock_slow", "0")))
        jits.append(float(d.get("jit_slow", "0")))
        ics.append(int(float(d.get("isr_cyc", "0"))))
    if not tot:
        return None
    return (
        100 * sc / tot, 100 * harm / tot, tot,
        st.median(locks), st.median(jits), st.median(ics),
    )


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=380)
    p.add_argument("--amp", type=float, default=30.0)
    p.add_argument("--alphas", type=float, nargs="+", default=[0.2, 0.5])
    p.add_argument("--betas", type=float, nargs="+", default=None,
                   help="if set, fix alpha=--drive-alpha and sweep zc_beta over these values")
    p.add_argument("--drive-alpha", type=float, default=0.5,
                   help="alpha to hold while sweeping --betas")
    p.add_argument("--snaps", type=int, default=5)
    p.add_argument("--dwell", type=float, default=2.0, help="settle (s) after each alpha change")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--cmd-delay", type=float, default=0.02)
    p.add_argument("--wait-port", type=float, default=30.0)
    p.add_argument("-v", "--verbose", action="count", default=0)
    args = p.parse_args()

    deadline = time.time() + args.wait_port
    raw = None
    while raw is None:
        try:
            raw = serial.Serial(args.port, args.baud, timeout=0.1)
        except serial.SerialException as exc:
            if time.time() >= deadline:
                print(f"{args.port} busy (close the live UI): {exc}")
                return 1
            print(f"{args.port} busy, retrying (close the live UI)...")
            time.sleep(2.0)

    amp_t = int(round(args.amp * 10))
    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, None)
        ser.reset_input_buffer()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)
        reset_cycles(ser, 0.4)
        print(f"spin up governed -> {args.hz} Hz {args.amp:.0f}% ...")
        position(ser, Setpoint(), args.hz, amp_t, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        set_detector(ser, "signchange")
        _clear_stall(ser)
        set_alpha(ser, 0.0)
        time.sleep(1.0)

        hdr = f"  {'cond':>16} | in-win ZC (sc) | harm cov | lock_s | jit_s | isr%"
        print("\n" + hdr)
        rows = []

        base = measure(ser, args.snaps, args.capture_timeout)
        if base:
            sc, hm, n, lk, jt, ic = base
            print(f"  {'open-loop a=0':>16} |    {sc:4.0f}%      |  {hm:4.0f}%   | {lk:5.2f}  "
                  f"| {jt:5.0f} | {100*ic/BUDGET:3.0f}")
            rows.append(("open-loop a=0", sc))

        set_harm_drive(ser, True)
        if args.betas:
            print(f"  -- harmonic DRIVES at alpha={args.drive_alpha:.2f}; sweeping zc_beta --")
            set_alpha(ser, args.drive_alpha)
            conditions = [("beta", b) for b in args.betas]
        else:
            print("  -- harmonic now DRIVES commutation (bounded by alpha/slew) --")
            conditions = [("alpha", a) for a in args.alphas]
        for kind, val in conditions:
            if kind == "beta":
                set_zc_beta(ser, val)
                label = f"b={val:.2f} a={args.drive_alpha:.1f}"
            else:
                set_alpha(ser, val)
                label = f"harm a={val:.2f}"
            time.sleep(args.dwell)
            m = measure(ser, args.snaps, args.capture_timeout)
            if m:
                sc, hm, n, lk, jt, ic = m
                over = " OVER!" if ic > BUDGET else ""
                print(f"  {label:>16} |    {sc:4.0f}%      |  {hm:4.0f}%   | {lk:5.2f}  "
                      f"| {jt:5.0f} | {100*ic/BUDGET:3.0f}{over}")
                rows.append((label, sc))

        # restore: alpha 0, zc_beta default, harm_drive off, kill
        set_alpha(ser, 0.0)
        if args.betas:
            set_zc_beta(ser, 0.60)
        set_harm_drive(ser, False)
        send(ser, "w")
        set_stall_kill(ser, True)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass

        if len(rows) >= 2:
            base_sc = rows[0][1]
            best_sc = max(r[1] for r in rows[1:])
            print(f"\n  VERDICT: in-window sign-change coverage  baseline {base_sc:.0f}%  ->  "
                  f"best-under-harm-drive {best_sc:.0f}%  (delta {best_sc-base_sc:+.0f} pts)")
            if best_sc >= base_sc + 15:
                print("  => coverage ROSE: the 2/6 wave looks like an open-loop artifact "
                      "(classical six-step recoverable).")
            elif best_sc <= base_sc + 5:
                print("  => coverage FLAT: the sparseness looks intrinsic (observer justified).")
            else:
                print("  => partial change: inconclusive; widen alpha / settle longer.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
