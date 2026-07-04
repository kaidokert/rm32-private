#!/usr/bin/env python3
"""LOOK at a spin, don't just tally it. Spin up to ONE speed with the real closed-loop drive
config, capture a frame, and dump BOTH:
  1. a rendered PNG of the 3-phase BEMF waveform (virtual neutral, sector grid, exact ZC marks),
  2. the per-commutation cl-log (i / phys / zc / coast / harm) so we see, commutation by
     commutation, what the detector saw and where it coasted.

This is the observe-before-control step we skipped: instead of tuning knobs against aggregate
stall points and a lying lockS, actually see whether the lock is clean and where the ZCs land.

  python scripts/cl_spin_look.py COM41 --hz 420 --amp 31 --alpha 0.4
Then open logs/spin_look.png. ATTENDED. Built for scope_cl2_48k. Leaves the motor killed.
"""
from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import serial  # noqa: E402

from scope_common import BAUD, classify_rotor_state, parse_capture, plot_zc_snapshot  # noqa: E402
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import PacedSerial, _clear_stall, reset_cycles, set_stall_kill  # noqa: E402
from cl_engage import set_alpha, set_detector  # noqa: E402
from cl_harm_check import tally  # noqa: E402
from cl_gov_sweep import set_advance  # noqa: E402


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=420)
    p.add_argument("--amp", type=float, default=31.0)
    p.add_argument("--alpha", type=float, default=0.4)
    p.add_argument("--advance", type=float, default=0.0)
    p.add_argument("--adv-sched", action="store_true")
    p.add_argument("--signchange", action="store_true")
    p.add_argument("--settle", type=float, default=2.0)
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--out", default="logs/spin_look.png")
    p.add_argument("--zc-window", type=int, default=2, help="snapshot smoothing window (keep small)")
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
        sp = Setpoint()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)
        reset_cycles(ser, 0.4)
        driver = "sign-change" if args.signchange else "harmonic"
        print(f"spin up -> {args.hz} Hz {args.amp:.0f}% (driver={driver}, alpha={args.alpha}, "
              f"advance={args.advance}{' sched' if args.adv_sched else ''}) ...")
        position(ser, sp, args.hz, amp_t, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        set_detector(ser, "signchange")
        if not args.signchange:
            send(ser, "'")
        _clear_stall(ser)
        set_alpha(ser, args.alpha)
        if args.advance > 0:
            set_advance(ser, args.advance)
            if args.adv_sched:
                send(ser, "/")
        drain(ser)
        time.sleep(args.settle)

        cap = parse_capture(capture(ser, args.capture_timeout, "c"))
        Path("logs").mkdir(exist_ok=True)
        rawlog = Path("logs") / f"spin_look_{args.hz}.log"
        rawlog.write_text(cap.text, encoding="utf-8", errors="replace")

        # Render the waveform picture.
        out = Path(args.out)
        try:
            sectors = plot_zc_snapshot(cap, out, smooth_window=args.zc_window)
        except Exception as exc:
            sectors = None
            print(f"  render failed: {exc}")

        # The numbers we DO have, for context.
        d = cap.debug
        rotor = classify_rotor_state(cap)
        n, zc, harm, both, rows = tally(cap.text)
        print(f"\n  captured {cap.frames} frames @ {args.hz} Hz  ->  {rawlog}  +  {out}")
        print(f"  debug: lockS={d.get('lock_slow','?')} jitS={d.get('jit_slow','?')} "
              f"period_est={d.get('period_est','?')} iu_ma={d.get('iu_ma','?')} "
              f"det={d.get('det','?')} mode={d.get('mode','?')}")
        print(f"  rotor(advisory): {rotor['state']} late_swing={rotor['late_swing']} "
              f"sensing={rotor['sensing']} plateau_spread={rotor['plateau_spread']}%")
        if sectors is not None:
            print("\n  offline per-sector ZC (from the waveform):")
            for s in sectors:
                print(f"    {s}")
        if rows:
            print(f"\n  per-commutation cl-log ({zc}/{n} sign-change hits, {harm}/{n} harmonic):")
            print("    i phys  zc  harm")
            for (i, ph, z, hm) in rows:
                zs = f"{z:4}" if z != -1 else "  --"
                hs = f"{hm:5}" if hm != 9999 else "   --"
                print(f"    {i:2}  {ph}  {zs} {hs}")

        send(ser, "w")
        set_stall_kill(ser, True)
        if args.advance > 0:
            set_advance(ser, 0.0)
            if args.adv_sched:
                send(ser, "/")
        if not args.signchange:
            send(ser, "'")
        set_alpha(ser, 0.0)
        try:
            set_watchdog(ser, False)
        except Exception:
            pass
        print(f"\n  >>> open {out} and look at the waveform + ZC marks <<<")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
