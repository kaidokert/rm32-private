#!/usr/bin/env python3
"""The probe we kept skipping: hand the spinning motor to the REAL PI loop (on_frame, full
frequency-tracking) and watch whether it HOLDS lock or drops out.

Sequence: spin up governed -> harmonic drives -> raise alpha -> wait for a genuine lock ->
press 'o' (CL_DRIVE: switch governed P-only -> on_frame full PI) -> stream the ~1 Hz `glitch:`
monitor line for --secs and report lockS + mode + coast-rate over time.

Holds  = mode stays 'drive', lockS stays high, coast count climbs slowly.
Drops  = mode reverts to 'gov' (lock hysteresis), lockS collapses, or stall kills it.

  python scripts/cl_handoff_probe.py COM41 --hz 380 --amp 30 --alpha 0.5 --secs 20
ATTENDED. Built for scope_cl2_48k. Leaves the motor killed.
"""
from __future__ import annotations

import argparse
import re
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD  # noqa: E402
from scope_sweep import Setpoint, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_alpha, set_detector  # noqa: E402

GLITCH_RE = re.compile(
    r"glitch: comm=(\d+) coast=(\d+) burst=(\d+) resid=(\d+) maxrun=(\d+) since=(\d+) "
    r"lockS=(\d+) mode=(\w+) det=(\w+) alpha=(\d+) beta=(\d+) pc=(\d+) cpu_busy=(\d+) isr_util=(\d+)"
)


def read_glitch(rs, secs, on_line=None):
    """Collect parsed glitch lines for `secs` from the raw serial.Serial `rs`. Returns dicts."""
    rows = []
    end = time.time() + secs
    buf = b""
    while time.time() < end:
        buf += rs.read(256)
        while b"\n" in buf:
            line, buf = buf.split(b"\n", 1)
            m = GLITCH_RE.search(line.decode("ascii", "replace"))
            if m:
                d = {
                    "comm": int(m.group(1)), "coast": int(m.group(2)), "burst": int(m.group(3)),
                    "maxrun": int(m.group(5)), "lockS": int(m.group(7)) / 1000.0,
                    "mode": m.group(8), "alpha": int(m.group(10)) / 1000.0,
                }
                rows.append(d)
                if on_line:
                    on_line(d)
    return rows


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=380)
    p.add_argument("--amp", type=float, default=30.0)
    p.add_argument("--alpha", type=float, default=0.5, help="governed authority before handoff")
    p.add_argument("--lock-gate", type=float, default=0.30, help="lockS needed before pressing 'o'")
    p.add_argument("--lock-wait", type=float, default=12.0, help="max s to wait for lock")
    p.add_argument("--secs", type=float, default=20.0, help="observation window after handoff")
    p.add_argument("--harm", action="store_true", default=True, help="harmonic drives (default)")
    p.add_argument("--signchange", action="store_true", help="sign-change drives instead of harmonic")
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

    amp_t = int(round(args.amp * 10))
    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, None)
        ser.reset_input_buffer()
        set_watchdog(ser, True)
        set_stall_kill(ser, True)  # clean cut on a real drop-out -> definitive failure marker
        reset_cycles(ser, 0.4)
        print(f"spin up governed -> {args.hz} Hz {args.amp:.0f}% ...")
        position(ser, Setpoint(), args.hz, amp_t, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)

        set_detector(ser, "signchange")
        _clear_stall(ser)
        send(ser, "i")  # enable the ~1 Hz glitch monitor stream
        set_alpha(ser, 0.0)
        drain(ser)

        # Stage 0: confirm the open-loop spin actually CAUGHT (lockS rises at alpha 0 on the
        # sign-change) before we hand any authority over. If it never locks here, the rotor
        # isn't following the field -- nothing downstream can work.
        driver = "sign-change" if args.signchange else "harmonic"
        print(f"\n  Stage 0: confirm catch at alpha=0 (sign-change) ...")
        for d in read_glitch(raw, 4.0):
            print(f"    [a=0]  lockS={d['lockS']:.2f} mode={d['mode']} coast={d['coast']}")

        # Stage 1: enable the chosen driver and ramp authority gradually so the loop (and, for
        # harmonic, its fit) settles at each step instead of being shocked straight to 0.5.
        if not args.signchange:
            send(ser, "'")  # harm_drive on
            drain(ser)
        print(f"\n  Stage 1: {driver} driving; ramp alpha -> {args.alpha}, wait lockS>{args.lock_gate} ...")
        locked = False
        ramp = [a for a in (0.2, 0.35, args.alpha) if a <= args.alpha] or [args.alpha]
        for a in ramp:
            set_alpha(ser, a)
            for d in read_glitch(raw, 2.5):
                print(f"    [a={a:.2f}] lockS={d['lockS']:.2f} mode={d['mode']} coast={d['coast']}")
                if d["lockS"] >= args.lock_gate:
                    locked = True
            if locked:
                break
        if not locked:
            print(f"  never reached lockS>{args.lock_gate} in {args.lock_wait}s -- aborting handoff.")
        else:
            # HAND OFF: governed P-only -> on_frame full PI.
            print("\n  >>> HANDOFF: pressing 'o' (CL_DRIVE -> on_frame full PI) <<<\n")
            send(ser, "o")
            base = None
            held_drive = 0
            rows = read_glitch(raw, args.secs)
            for d in rows:
                if base is None:
                    base = d
                print(f"    [hold] t~{rows.index(d):2}s lockS={d['lockS']:.2f} mode={d['mode']:5} "
                      f"coast={d['coast']} burst={d['burst']} maxrun={d['maxrun']}")
                if d["mode"] == "drive":
                    held_drive += 1

            # Verdict.
            print()
            if not rows:
                print("  VERDICT: no monitor lines after handoff (stalled/killed immediately?).")
            else:
                last = rows[-1]
                dcomm = last["comm"] - rows[0]["comm"]  # commutations actually advanced
                dcoast = last["coast"] - rows[0]["coast"]
                coast_frac = 100.0 * dcoast / dcomm if dcomm > 0 else 0.0
                drive_frac = 100.0 * held_drive / len(rows)
                print(f"  VERDICT: in 'drive' {drive_frac:.0f}% | dcomm={dcomm} (commutations advanced) "
                      f"| final lockS={last['lockS']:.2f} | coast {coast_frac:.0f}% | maxrun={last['maxrun']}")
                if dcomm <= 2:
                    print("  => STALLED: commutations STOPPED at handoff (counters frozen -> motor killed).")
                elif drive_frac >= 80 and last["lockS"] >= args.lock_gate and coast_frac < 50:
                    print("  => HELD: the full PI loop sustained closed-loop drive.")
                elif drive_frac < 20:
                    print("  => DID NOT TAKE: reverted to governed almost immediately (lock too weak).")
                else:
                    print("  => UNSTABLE: drifted in/out of drive -- the PI isn't holding sync.")

        send(ser, "i")  # monitor off
        send(ser, "w")  # kill
        send(ser, "'") if not args.signchange else None  # harm_drive off
        set_alpha(ser, 0.0)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
