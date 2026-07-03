#!/usr/bin/env python3
"""Path A validation, HONEST version: climb commanded Hz in the governed closed loop and, at
each plateau, decide whether the ROTOR IS ACTUALLY TURNING from the BEMF waveform
(scope_common.classify_rotor_state) -- NOT from the loop's self-reported lockS, which stays
high even when the rotor has stalled and the field is running away from it (the detector
latches onto PWM/driven-phase artifacts). lockS is shown alongside so the gap is visible.

  python scripts/cl_gov_sweep.py COM41 --hz-start 250 --hz-end 1300 --hz-step 150 --alpha 0.4 --signchange
ATTENDED -- watch the motor and cross-check the 'rotor' column against what you physically see.
Built for scope_cl2_48k. Leaves the motor killed.

NOTE: at very high Hz the ADC gives few frames/sector (~6 at 1300 Hz), so the waveform
classifier gets noisier up there; below ~900 Hz it's solid. --signchange avoids the harmonic's
CPU-budget fault at speed.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD, classify_rotor_state, parse_capture  # noqa: E402
from scope_sweep import (Setpoint, capture, drain, position, ramp_amp_to,  # noqa: E402
                         ramp_freq_to, send, set_watchdog)
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_alpha, set_detector  # noqa: E402


def amp_for_hz(hz: int, base_pct: float, slope: float, cap_pct: float) -> int:
    return int(round(min(cap_pct, base_pct + slope * hz) * 10))


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz-start", type=int, default=250)
    p.add_argument("--hz-end", type=int, default=1300)
    p.add_argument("--hz-step", type=int, default=150)
    p.add_argument("--alpha", type=float, default=0.4)
    p.add_argument("--amp-base", type=float, default=26.0)
    p.add_argument("--amp-slope", type=float, default=0.014)
    p.add_argument("--amp-cap", type=float, default=45.0)
    p.add_argument("--signchange", action="store_true", help="cheap 2/6 driver (no harmonic CPU fault)")
    p.add_argument("--settle", type=float, default=1.5, help="settle (s) at each Hz before capture")
    p.add_argument("--announce-pause", type=float, default=1.5,
                   help="pause (s) after announcing the next Hz so you can focus before it moves")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--stop-on-stall", action="store_true", default=True)
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

    def classify(hz, amp_t):
        """Capture a frame and return (rotor_state_dict, lockS_0_1, iu_ma, hz_echo)."""
        cap = parse_capture(capture(ser, args.capture_timeout, "c"))
        rotor = classify_rotor_state(cap)
        d = cap.debug
        lockS = int(float(d.get("lock_slow", "0"))) / 1000.0
        iu = d.get("iu_ma", "?")
        return rotor, lockS, iu, d.get("hz", "?")

    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, None)
        ser.reset_input_buffer()
        sp = Setpoint()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)  # DON'T auto-kill: we want to SEE the stalled waveform, not a freeze
        reset_cycles(ser, 0.4)

        amp0 = amp_for_hz(args.hz_start, args.amp_base, args.amp_slope, args.amp_cap)
        driver = "sign-change" if args.signchange else "harmonic"
        print(f"spin up governed -> {args.hz_start} Hz {amp0/10:.0f}% (driver={driver}, alpha={args.alpha}) ...")
        position(ser, sp, args.hz_start, amp0, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        set_detector(ser, "signchange")
        if not args.signchange:
            send(ser, "'")  # harm_drive on
        _clear_stall(ser)
        set_alpha(ser, args.alpha)
        drain(ser)

        # NOTE: the classifier column is ADVISORY -- in closed-loop drive it does NOT reliably
        # separate spinning from stalled (a stalled rotor still shows big late_swing). WATCH the
        # motor; the announce+pause below is there so you can track which Hz is running live.
        print(f"\n  {'Hz':>5} | {'amp%':>4} | ROTOR (advisory) | lockS | late_swing | sensing | iu_ma")
        last_turning = None
        for hz in range(args.hz_start, args.hz_end + 1, args.hz_step):
            amp_t = amp_for_hz(hz, args.amp_base, args.amp_slope, args.amp_cap)
            print(f"  -> NEXT: {hz} Hz {amp_t/10:.0f}%  --  watch the motor now ...", flush=True)
            time.sleep(args.announce_pause)
            ramp_amp_to(ser, sp, amp_t)
            ramp_freq_to(ser, sp, hz)
            drain(ser)
            time.sleep(args.settle)
            try:
                rotor, lockS, iu, hz_echo = classify(hz, amp_t)
            except Exception as exc:
                print(f"  {hz:>5} | {amp_t/10:>4.0f} | CAPTURE FAILED ({exc}) -- likely stalled/fault")
                break
            state = rotor["state"].upper()
            turning = rotor["state"] == "locked"
            if turning:
                last_turning = hz
            print(f"  {hz:>5} | {amp_t/10:>4.0f} | {state:>13} | {lockS:5.2f} | "
                  f"{str(rotor['late_swing']):>10} | {rotor['sensing']:>7} | {iu}")
            if args.stop_on_stall and rotor["state"] == "stalled" and rotor["sensing"] == "ok":
                print(f"  -- STALLED (waveform) at {hz} Hz; last confirmed turning {last_turning} Hz --")
                break

        # cleanup
        send(ser, "w")
        set_stall_kill(ser, True)
        if not args.signchange:
            send(ser, "'")
        set_alpha(ser, 0.0)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass

        print(f"\n  RESULT: rotor confirmed TURNING (waveform) up to {last_turning} Hz "
              f"with driver={driver}, alpha={args.alpha}.")
        print("  (Cross-check that number against where you physically saw it stop.)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
