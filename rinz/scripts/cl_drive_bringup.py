#!/usr/bin/env python3
"""Step-3 bring-up: governed spin-up -> alpha-blend builds lock -> full-sensorless DRIVE -> climb.

Careful, instrumented, with SETTLING WAITS (lock_slow is a ~0.6 s IIR at 380 Hz, so each alpha
step is given several seconds before reading). It does NOT force anything: it sweeps alpha and
MEASURES lock_slow; it only engages drive ('o') if the lock genuinely clears the handoff, and
only climbs amp while mode stays 'drive'. Aborts on any FAULT. Every command + reading is logged.

The open question this answers: does the alpha-blend loop actually raise lock_slow to the 0.40
handoff at a sane speed, or not? If not, drive can't be reached this way and we know to lower
the threshold / fix detection instead of guessing.

  python scripts/cl_drive_bringup.py COM41 --hz 380 --amp 30
  python scripts/cl_drive_bringup.py COM41 --hz 380 --amp 30 --climb   # also try the amp climb
ATTENDED. Built for scope_cl2_48k. Keep a hand near the bench; --climb accelerates the motor.
"""

from __future__ import annotations

import argparse
import statistics as st
import sys
import time
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import BAUD, parse_capture  # noqa: E402
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import (PacedSerial, _clear_stall, _stalled, reset_cycles,  # noqa: E402
                           set_alpha, set_stall_kill)
from cl_engage import set_detector  # noqa: E402


def read_state(ser, timeout, want_hz=None, tries=3):
    """Median lock_slow/lock_fast + latest mode/det/hz from `tries` debug captures."""
    ls, lf = [], []
    mode = det = hz = None
    for _ in range(tries):
        if _stalled(ser):
            break
        try:
            cap = parse_capture(capture(ser, timeout, "c"))
        except Exception:
            continue
        d = cap.debug
        hz = int(float(d.get("hz", "0")))
        if want_hz is not None and abs(hz - want_hz) > 5:
            continue
        mode = d.get("mode", "?")
        det = d.get("det", "?")
        try:
            ls.append(float(d.get("lock_slow", "nan")) / 1000.0)
            lf.append(float(d.get("lock_fast", "nan")) / 1000.0)
        except ValueError:
            pass
    return {
        "lock_slow": st.median(ls) if ls else float("nan"),
        "lock_fast": st.median(lf) if lf else float("nan"),
        "mode": mode, "det": det, "hz": hz,
    }


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=380, help="governed spin-up frequency")
    p.add_argument("--amp", type=float, default=30.0)
    p.add_argument("--alphas", type=float, nargs="+", default=[0.0, 0.2, 0.5, 0.7, 1.0])
    p.add_argument("--settle", type=float, default=6.0, help="s to let lock_slow settle per alpha")
    p.add_argument("--handoff", type=float, default=0.40, help="lock_slow needed to engage drive")
    p.add_argument("--climb", action="store_true", help="after drive engages, ramp amp to climb")
    p.add_argument("--amp-step", type=int, default=2, help="amp %% per climb step")
    p.add_argument("--amp-max", type=float, default=55.0)
    p.add_argument("--climb-dwell", type=float, default=2.5)
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--cmd-delay", type=float, default=0.02)
    p.add_argument("--wait-port", type=float, default=0.0)
    p.add_argument("-v", "--verbose", action="count", default=1)
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    outdir = Path("logs") / f"bringup_{stamp}"
    outdir.mkdir(parents=True, exist_ok=True)
    logf = open(outdir / "bringup.log", "w", encoding="utf-8")

    def both(s):
        print(s, flush=True)
        logf.write(s + "\n")
        logf.flush()

    both(f"STEP-3 BRING-UP @ {args.hz} Hz amp {args.amp:.0f}%  handoff lock_slow>={args.handoff}  ({outdir})")
    deadline = time.time() + args.wait_port
    raw = None
    while raw is None:
        try:
            raw = serial.Serial(args.port, args.baud, timeout=0.1)
        except serial.SerialException as exc:
            if time.time() >= deadline:
                both(f"  {args.port} busy: {exc}")
                return 1
            both(f"  {args.port} busy, retrying (close the terminal)...")
            time.sleep(2.0)
    amp_t = int(round(args.amp * 10))
    best_ls = 0.0
    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, logf)
        ser.reset_input_buffer()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)  # observe lock-loss without the motor dying mid-bring-up
        try:
            reset_cycles(ser, 0.4)
            both(f"\n[1] spin up GOVERNED to {args.hz} Hz, {args.amp:.0f}% ...")
            position(ser, Setpoint(), args.hz, amp_t, qsettle=0.8, ramp_step=20,
                     ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
            drain(ser)
            set_detector(ser, "signchange")
            _clear_stall(ser)
            st0 = read_state(ser, args.capture_timeout, args.hz)
            both(f"    spun up: det={st0['det']} mode={st0['mode']} hz={st0['hz']} "
                 f"lock_slow={st0['lock_slow']:.3f}")

            both(f"\n[2] ALPHA SWEEP (build lock; {args.settle}s settle each):")
            for a in args.alphas:
                if _stalled(ser):
                    both("    STALL during sweep -- aborting"); break
                set_alpha(ser, a)
                time.sleep(args.settle)  # lock_slow IIR needs seconds
                s = read_state(ser, args.capture_timeout, args.hz)
                best_ls = max(best_ls, s["lock_slow"] if s["lock_slow"] == s["lock_slow"] else 0)
                flag = "  >= handoff" if s["lock_slow"] >= args.handoff else ""
                both(f"    alpha={a:.2f} | lock_slow={s['lock_slow']:.3f} lock_fast={s['lock_fast']:.3f} "
                     f"mode={s['mode']} hz={s['hz']}{flag}")

            both(f"\n    => max lock_slow across alpha = {best_ls:.3f}  (handoff needs {args.handoff})")

            if _stalled(ser) or best_ls < args.handoff:
                both("\n[3] DRIVE NOT ATTEMPTED -- lock never cleared the handoff at this speed.")
                both("    The alpha-blend can't center the ZC enough here. Options: higher hz,")
                both("    or lower CL_DRIVE_HANDOFF_LOCK in firmware. (No guessing -- the numbers above show it.)")
            else:
                both("\n[3] lock cleared -- engaging DRIVE ('o') at the current alpha ...")
                send(ser, "o")
                time.sleep(2.0)
                s = read_state(ser, args.capture_timeout, args.hz)
                both(f"    after 'o': mode={s['mode']} lock_slow={s['lock_slow']:.3f} hz={s['hz']}")
                if s["mode"] != "drive":
                    both("    mode did not become 'drive' -- lock dropped at handoff (hysteresis). Stopping.")
                elif args.climb:
                    both(f"\n[4] CLIMB: ramping amp by {args.amp_step}%/step to {args.amp_max:.0f}% "
                         f"-- hz should rise on its own (speed = output in drive):")
                    cur_amp = args.amp
                    while cur_amp < args.amp_max and not _stalled(ser):
                        for _ in range(args.amp_step):
                            send(ser, "a")
                        cur_amp += args.amp_step
                        time.sleep(args.climb_dwell)
                        s = read_state(ser, args.capture_timeout)
                        both(f"    amp~{cur_amp:.0f}% | hz={s['hz']} mode={s['mode']} "
                             f"lock_slow={s['lock_slow']:.3f}")
                        if s["mode"] != "drive":
                            both("    reverted to gov (lock dropped) -- climb ceiling reached. Stopping.")
                            break
                else:
                    both("    DRIVE engaged and holding. (--climb to ramp amp and accelerate.)")
        finally:
            send(ser, "w")
            _clear_stall(ser)
            set_stall_kill(ser, True)
            try:
                set_watchdog(ser, False)
            except Exception:
                pass
    both(f"\nlog -> {outdir}/bringup.log")
    logf.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
