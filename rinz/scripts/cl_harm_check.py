#!/usr/bin/env python3
"""Observe-only harmonic check: spin up governed, sign-change detector, capture, and tally the
per-commutation cl-log -- how many commutations the harmonic (harm=) covers vs sign-change (zc=),
plus isr_cyc / lock. The validation gate for the streaming harmonic detector on real hardware.

  python scripts/cl_harm_check.py COM41 --hz 380 --amp 30 --snaps 4
ATTENDED. Built for scope_cl2_48k. Leaves the motor killed.
"""
from __future__ import annotations

import argparse
import re
import statistics as st
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD, parse_capture  # noqa: E402
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_detector  # noqa: E402

CL_RE = re.compile(r"cl i=(\d+) phys=(\d+) zc=(-?\d+) coast=(\d+) lf=(-?\d+) harm=(-?\d+) bnd=(\d+)")
BUDGET = 170_000_000 // 48_000  # 48 kHz tick budget in cycles


def tally(text):
    zc = harm = both = n = 0
    rows = []
    for m in CL_RE.finditer(text):
        n += 1
        z = int(m.group(3)) != -1
        h = int(m.group(6)) != 9999
        zc += z
        harm += h
        both += z and h
        rows.append((int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(6))))
    return n, zc, harm, both, rows


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=380)
    p.add_argument("--amp", type=float, default=30.0)
    p.add_argument("--snaps", type=int, default=4)
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
        time.sleep(1.0)
        print(f"\n  {'snap':>4} | det mode hz  | sign-change | harmonic | isr_cyc (util%) | lockS jitS")
        zc_tot = harm_tot = tot = 0
        last_rows = None
        for i in range(args.snaps):
            try:
                cap = parse_capture(capture(ser, args.capture_timeout, "c"))
            except Exception as exc:
                print(f"  {i:>4} | capture/parse failed: {exc}")
                continue
            d = cap.debug
            n, zc, harm, both, rows = tally(cap.text)
            last_rows = rows or last_rows
            zc_tot += zc
            harm_tot += harm
            tot += n
            ic = int(float(d.get("isr_cyc", "0")))
            util = 100 * ic / BUDGET
            over = "  OVER!" if ic > BUDGET else ""
            print(f"  {i:>4} | {d.get('det','?')} {d.get('mode','?')} {d.get('hz','?')} |   "
                  f"{zc:2}/{n:<2}     |  {harm:2}/{n:<2}  | {ic:4} ({util:3.0f}%){over} | "
                  f"{d.get('lock_slow','?')} {d.get('jit_slow','?')}")
        if tot:
            print(f"\n  TOTAL across {args.snaps} snaps: sign-change {zc_tot}/{tot} "
                  f"({100*zc_tot/tot:.0f}%)  vs  harmonic {harm_tot}/{tot} ({100*harm_tot/tot:.0f}%)")
        if last_rows:
            print("\n  last capture per-commutation (i phys zc harm):")
            for (i, ph, zc, harm) in last_rows:
                zs = f"{zc:4}" if zc != -1 else "  --"
                hs = f"{harm:5}" if harm != 9999 else "   --"
                print(f"    i={i:2} phys={ph} zc={zs} harm={hs}")
        send(ser, "w")
        set_stall_kill(ser, True)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
