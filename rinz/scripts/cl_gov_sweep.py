#!/usr/bin/env python3
"""Path A: validate the GOVERNED closed loop across the speed range. The motor stays in
on_frame_blend (rate commanded, BEMF phase-locked, bounded alpha -- the mode that already
holds lock). We climb the commanded Hz step by step and report, per plateau, whether it holds:
lockS, coast-rate, and whether commutations keep advancing (vs a stall freeze).

Default driver is the harmonic (high coverage) bounded by a moderate alpha so it corrects phase
without the over-drive stall seen at alpha~1.0. Use --signchange for the simpler 2/6 driver.

  python scripts/cl_gov_sweep.py COM41 --hz-start 250 --hz-end 1300 --hz-step 100 --alpha 0.4
ATTENDED. Built for scope_cl2_48k. Leaves the motor killed.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD  # noqa: E402
from scope_sweep import (Setpoint, drain, position, ramp_amp_to, ramp_freq_to,  # noqa: E402
                         send, set_watchdog)
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_alpha, set_detector  # noqa: E402
from cl_handoff_probe import read_glitch  # noqa: E402


def amp_for_hz(hz: int, base_pct: float, slope: float, cap_pct: float) -> int:
    """Amp schedule (tenths). Torque/voltage demand rises with speed; ramp amp with Hz, capped."""
    return int(round(min(cap_pct, base_pct + slope * hz) * 10))


def hold_metrics(rows):
    """From a plateau's glitch rows: (lockS_med, coast_frac%, dcomm, stalled?)."""
    if not rows:
        return None
    lk = sorted(d["lockS"] for d in rows)[len(rows) // 2]
    dcomm = rows[-1]["comm"] - rows[0]["comm"]
    dcoast = rows[-1]["coast"] - rows[0]["coast"]
    coast_frac = 100.0 * dcoast / dcomm if dcomm > 0 else 0.0
    stalled = dcomm <= 2  # commutations stopped advancing -> motor died
    return lk, coast_frac, dcomm, stalled


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz-start", type=int, default=250)
    p.add_argument("--hz-end", type=int, default=1300)
    p.add_argument("--hz-step", type=int, default=100)
    p.add_argument("--alpha", type=float, default=0.4)
    p.add_argument("--amp-base", type=float, default=26.0, help="amp%% at 0 Hz (schedule intercept)")
    p.add_argument("--amp-slope", type=float, default=0.014, help="amp%% per Hz")
    p.add_argument("--amp-cap", type=float, default=45.0, help="max amp%%")
    p.add_argument("--signchange", action="store_true", help="sign-change drives instead of harmonic")
    p.add_argument("--dwell", type=float, default=3.0, help="observe (s) at each Hz plateau")
    p.add_argument("--lock-floor", type=float, default=0.15, help="lockS below this = degraded")
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
            print(f"{args.port} busy, retrying...")
            time.sleep(2.0)

    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, None)
        ser.reset_input_buffer()
        sp = Setpoint()
        set_watchdog(ser, True)
        set_stall_kill(ser, True)  # a real slip cuts cleanly -> the plateau shows a frozen dcomm
        reset_cycles(ser, 0.4)

        amp0 = amp_for_hz(args.hz_start, args.amp_base, args.amp_slope, args.amp_cap)
        driver = "sign-change" if args.signchange else "harmonic"
        print(f"spin up governed -> {args.hz_start} Hz {amp0/10:.0f}% (driver={driver}, alpha={args.alpha}) ...")
        position(ser, sp, args.hz_start, amp0, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        set_detector(ser, "signchange")
        if not args.signchange:
            send(ser, "'")  # harm_drive on (still governed -- alpha bounds authority)
        _clear_stall(ser)
        send(ser, "i")  # glitch monitor stream
        set_alpha(ser, args.alpha)
        drain(ser)

        print(f"\n  {'Hz':>5} | {'amp%':>4} | lockS | coast% | dcomm | status")
        last_ok = None
        for hz in range(args.hz_start, args.hz_end + 1, args.hz_step):
            amp_t = amp_for_hz(hz, args.amp_base, args.amp_slope, args.amp_cap)
            ramp_amp_to(ser, sp, amp_t)
            ramp_freq_to(ser, sp, hz)         # gradual: 'f'/'g' steps so the rotor accelerates
            drain(ser)
            rows = read_glitch(raw, args.dwell)
            m = hold_metrics(rows)
            if m is None:
                print(f"  {hz:>5} | {amp_t/10:>4.0f} |   -   |   -    |   -   | NO DATA")
                continue
            lk, coast_frac, dcomm, stalled = m
            if stalled:
                status = "STALL (frozen)"
            elif lk < args.lock_floor:
                status = "lock LOST"
            else:
                status = "holding"
                last_ok = hz
            print(f"  {hz:>5} | {amp_t/10:>4.0f} | {lk:5.2f} | {coast_frac:5.0f}  | {dcomm:5} | {status}")
            if stalled or lk < args.lock_floor:
                print(f"  -- stop: lost lock at {hz} Hz (last good {last_ok} Hz) --")
                break

        # cleanup
        send(ser, "i")
        send(ser, "w")
        if not args.signchange:
            send(ser, "'")
        set_alpha(ser, 0.0)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass

        if last_ok:
            print(f"\n  RESULT: governed closed loop held to {last_ok} Hz "
                  f"({last_ok*60} eRPM-ish) with driver={driver}, alpha={args.alpha}.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
