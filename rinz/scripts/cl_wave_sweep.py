#!/usr/bin/env python3
"""Test the cause of the per-sector ZC wave: LOAD-ANGLE or GEOMETRY?

The closed loop's in-window ZC coverage is one-per-electrical-rev asymmetric -- the first
half of the rev (sectors 0,1,2) crosses late/in-window, the second half (3,4,5) crosses
early/out-of-window. This sweeps AMP at a fixed frequency (closed loop), measures the
per-sector firmware-linfit ZC wave at each amp, and overlays them:

  - if the wave's PHASE slides as amp changes  -> the wave tracks LOAD ANGLE.
  - if it stays nailed to the same sectors     -> fixed motor/sense GEOMETRY.

Amp is the load-angle knob at fixed speed (more torque margin -> different rotor lead).
Each capture's `cl ... lf=` log gives the raw linfit % per sector; we aggregate the
median per physical sector across snaps. Writes logs/wave_<ts>.png + .json. ATTENDED.

Every point is measured INDEPENDENTLY -- no state carried between setpoints, so a stall at
one can't poison the next. Per (hz, amp):
  1. q/w/q/w (with pauses) -- aggressive clear of any latched / half-stuck state to idle.
  2. q   -- reset to base (inside position()).
  3. ramp fresh up to (hz, amp).
  4. VERIFY: each capture's debug line must report the target hz AND amp, else it's rejected
     (the ramp didn't land / firmware isn't where we asked).
  5. LOCKED: the host rotor classifier must certify the rotor genuinely spinning, else
     rejected (a stuck rotor's demag transient otherwise fools the linfit into fake data).
  6. Only verified+locked captures feed the wave; a point that never satisfies both is
     honest nan, flagged STALLED / NOT-AT-SETPOINT. Slower (a full re-spin per point) but
     the data is trustworthy, not garbage carried over from a prior stall. OPEN loop is the
     clean probe (closed loop re-times the commutation and masks the wave).

  python scripts/cl_wave_sweep.py COM41 --hz 250 --amps 13 15 17 19          # open loop (default)
  python scripts/cl_wave_sweep.py COM41 --freqs 150 250 350 --amps 12 15 18  # the larger (Hz, amp) plane
"""

from __future__ import annotations

import argparse
import json
import statistics as st
import sys
import time
from collections import defaultdict
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import (BAUD, PHASE_NAMES, PHASE_TO_CHANNEL, analyze_zero_crossings,
                          classify_rotor_state, linfit_mb, parse_capture, parse_cl_bounds)
from scope_sweep import Setpoint, capture, position, send, set_watchdog

def set_alpha(ser, alpha: float) -> None:
    send(ser, "0")
    for _ in range(int(round(max(0.0, min(1.0, alpha)) / 0.05))):
        send(ser, "m")
        time.sleep(0.02)


def collect_wave(ser, snaps, timeout, want_hz, want_amp_t, slope_min=15.0):
    """Take `snaps` captures at a setpoint; return (median linfit wave, n_locked, n_verified).

    Each capture must pass TWO gates before it feeds the wave:
      VERIFIED -- the firmware's own debug line reports the target hz AND amp (else the ramp
                  didn't land where we asked / the firmware is in some other state -> reject).
      LOCKED   -- the host rotor classifier certifies the rotor is genuinely spinning (a
                  stuck rotor's demag transient otherwise fools the linfit into fake numbers).
    Only verified+locked captures are measured; everything else is dropped so a point that
    never satisfies both comes back honest nan. Linfit is ungated by the firmware [-30,130]%
    (open-loop crossings sit far out-of-window); flat windows (|slope| < slope_min) dropped."""
    acc: dict[int, list[float]] = defaultdict(list)
    n_locked = n_verified = 0
    for _ in range(snaps):
        cap = parse_capture(capture(ser, timeout, "c"))
        try:
            chz = int(float(cap.debug.get("hz", "x")))
            camp = int(float(cap.debug.get("amp", "x")))  # 0.1% units, e.g. 200 = 20.0%
        except (TypeError, ValueError):
            continue
        if chz != want_hz or abs(camp - want_amp_t) > 2:
            continue  # firmware NOT at the requested setpoint -> not a valid measurement
        n_verified += 1
        if classify_rotor_state(cap, smooth_window=3).get("state") != "locked":
            continue  # at setpoint but not spinning (stalled) -> don't trust
        n_locked += 1
        fps = cap.sample_hz / (chz * 6.0)
        bounds, _ = parse_cl_bounds(cap.text)
        secs, smooth, neutral = analyze_zero_crossings(cap, smooth_window=3, sector_bounds=bounds)
        frames = len(neutral)
        for s in secs:
            ch = PHASE_TO_CHANNEL[PHASE_NAMES.index(s.phase)]
            s0 = int(round(s.start_frame)) + 1
            s1 = min(int(round(s.end_frame)), frames)
            mb = linfit_mb([smooth[ch][f] - neutral[f] for f in range(s0, s1)])
            if mb is None or abs(mb[0]) < slope_min:
                continue  # flat window -> no observable crossing
            z = (-mb[1] / mb[0]) / fps * 100.0
            if -100.0 <= z <= 200.0:  # sane range (drops bonkers extrapolations)
                acc[s.index % 6].append(z)
    return {s: st.median(v) for s, v in acc.items() if v}, n_locked, n_verified


def reset_cycles(ser, pause, cycles=2):
    """Thoroughly clear firmware + motor state before a fresh ramp: cycle q (reset) /
    w (kill) with pauses, `cycles` times, so any latched / half-stuck condition settles to a
    known idle before we spin up. Ends killed; the following ramp re-arms via its own 'q'."""
    for _ in range(cycles):
        send(ser, "q")
        time.sleep(pause)
        send(ser, "w")
        time.sleep(pause)
    ser.reset_input_buffer()  # discard the reset/kill echoes


def measure_point(ser, hz, amp_t, alpha, snaps, timeout, settle, reset_pause):
    """Independent measurement of ONE (hz, amp): clear -> ramp fresh -> verify the firmware
    echoed the target -> measure. NOTHING is carried from any prior point, so a stall at one
    setpoint can't poison the next. Returns (wave, n_locked, n_verified)."""
    reset_cycles(ser, reset_pause)  # q,w,q,w -- aggressive clear before ramping
    # position() sends 'q' (reset to base) then ramps freq+amp up to the target.
    position(ser, Setpoint(), hz, amp_t, qsettle=0.8, ramp_step=20,
             ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
    if alpha > 0:
        set_alpha(ser, alpha)  # 'q' already set open-loop alpha=0
    time.sleep(settle)
    return collect_wave(ser, snaps, timeout, hz, amp_t)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250, help="single frequency (if --freqs unset)")
    p.add_argument("--freqs", type=int, nargs="+", default=None,
                   help="frequencies to sweep (the ORTHOGONAL load-angle axis); re-spins per freq")
    p.add_argument("--amps", type=float, nargs="+", default=[13, 15, 17, 19],
                   help="amp %% values to sweep (the load-angle knob)")
    p.add_argument("--alpha", type=float, default=0.0,
                   help="0 = open loop (the clean probe; cannot desync). >0 = closed loop")
    p.add_argument("--snaps", type=int, default=6)
    p.add_argument("--min-lock", type=int, default=3,
                   help="stop the amp descent when fewer than this many snaps verify LOCKED "
                        "(stay in the solid regime; below this the motor stalls hard / OCP)")
    p.add_argument("--settle", type=float, default=1.0)
    p.add_argument("--reset-pause", type=float, default=0.4,
                   help="pause (s) between each q/w in the pre-ramp clear cycle")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    freqs = args.freqs or [args.hz]
    grid: dict[tuple[int, float], dict[int, float]] = {}  # (hz, amp) -> wave

    with serial.Serial(args.port, args.baud, timeout=0.1) as ser:
        ser.reset_input_buffer()
        if set_watchdog(ser, True):
            print("watchdog ARMED")
        print(f"\n  {'hz':>4} {'amp%':>5} | " + " ".join(f"s{s}" for s in range(6))
              + "   ver/lock  status   (every point: w->q->ramp->verify->measure)")
        try:
            for hz in freqs:
                for amp in args.amps:
                    amp_t = int(round(amp * 10))
                    # FULL independent measurement -- kill, reset, ramp fresh, verify, measure.
                    w, nl, nv = measure_point(ser, hz, amp_t, args.alpha, args.snaps,
                                              args.capture_timeout, args.settle, args.reset_pause)
                    if nv == 0:
                        status = "NOT-AT-SETPOINT (ramp/echo failed)"
                    elif nl == 0:
                        status = "STALLED (at setpoint, not spinning)"
                    elif nl < args.min_lock:
                        status = f"MARGINAL (lock {nl})"
                    else:
                        status = "ok"
                        grid[(hz, amp)] = w  # only trust a solidly-locked point
                    row = " ".join(f"{w.get(s, float('nan')):3.0f}" for s in range(6))
                    print(f"  {hz:>4} {amp:>5.1f} | {row}   {nv}/{nl}/{args.snaps}  {status}")
        finally:
            send(ser, "w")  # leave the motor killed
            try:
                set_watchdog(ser, False)
            except Exception:
                pass

    outdir = Path("logs")
    outdir.mkdir(exist_ok=True)
    jpath = outdir / f"wave_{stamp}.json"
    jpath.write_text(json.dumps({f"{hz}/{amp}": w for (hz, amp), w in grid.items()}, indent=2),
                     encoding="ascii")
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        # One panel per frequency: the per-sector wave vs amp (look for slide + flatten).
        n = len(freqs)
        fig, axes = plt.subplots(1, n, figsize=(5 * n, 5), squeeze=False)
        for col, hz in enumerate(freqs):
            ax = axes[0][col]
            for amp in args.amps:
                w = grid.get((hz, amp), {})
                ys = [w.get(s, float("nan")) for s in range(6)]
                ax.plot(range(6), ys, "-o", label=f"{amp:.1f}%")
            ax.axhline(50, color="0.7", ls="--", lw=0.8)
            ax.set_title(f"{hz} Hz")
            ax.set_xlabel("physical sector (0..5)")
            ax.set_xticks(range(6))
            ax.grid(alpha=0.3)
            if col == 0:
                ax.set_ylabel("linfit ZC (% of window)")
            ax.legend(title="amp", fontsize=7)
        fig.suptitle("Per-sector ZC wave vs (amp, freq)  "
                     "(slides/flattens with load => load-angle; fixed => geometry)")
        png = outdir / f"wave_{stamp}.png"
        fig.tight_layout()
        fig.savefig(png, dpi=110)
        print(f"\nwave plot -> {png}\njson -> {jpath}")
    except Exception as exc:
        print(f"(plot skipped: {exc}); json -> {jpath}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
