#!/usr/bin/env python3
"""Phase 4 (path 2) -- climb electrical frequency under closed loop, measure if lock HOLDS.

The detection wall is ~2/6 sectors in-window, and loop tuning can't fix it (the predict-gate
A/B refuted that). The open question is whether the resulting weak-but-real lock is GOOD
ENOUGH to climb toward the 90-95% duty / ~1300 Hz goal. So: engage the loop once at a sweet-
spot alpha, then ramp the commanded frequency up in tiers and measure at each tier whether the
lock holds. The frequency is governed by the open-loop schedule (alpha<1), so the motor spins
at the commanded speed regardless of lock -- we are measuring lock QUALITY vs speed, safely.

Per tier (amp ridden up the stall curve so it keeps spinning):
  lock_slow  -- sustained ZC-hit fraction (the honest 'is it locked' signal; lock_fast spikes)
  jit_slow   -- ZC jitter in ticks (commutation timing quality; /period*60 = elec degrees)
  in-win ZC  -- observed in-window crossings this capture
  iu_ma      -- current (a desync usually spikes it)
  + every FAULT: line is surfaced inline (COAST_BURST maxrun = worst desync this tier).

The verdict watches lock_slow / jit across speed: holds flat -> the sparse loop scales; crashes
at some tier -> that's the ceiling, and the evidence that forces the streaming harmonic detector.

  python scripts/cl_climb.py COM41 --freqs 210 250 300 350 400 450 500 --alpha 0.5
ATTENDED. Built for scope_cl2_48k (48 kHz). Reuses cl_engage's measurement + the FAULT telemetry.
"""

from __future__ import annotations

import argparse
import math
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

from scope_common import BAUD, parse_capture  # noqa: E402
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog, stall_amp  # noqa: E402
from cl_wave_sweep import (PacedSerial, _clear_stall, _stalled, reset_cycles,  # noqa: E402
                           set_alpha, set_stall_kill)
from cl_engage import oracle_offsets, set_detector, set_predict_gate  # noqa: E402


def amp_for(hz, margin, min_amp):
    """Amp % to hold at `hz`: above the stall curve so the rotor keeps spinning open-loop."""
    return max(min_amp, math.ceil((stall_amp(hz) + margin)))


def ramp_to(ser, cur_hz, cur_amp, hz, amp, step_dwell):
    """Gradually nudge (freq, amp) from current to target so the rotor stays synced while the
    speed changes. Freq via f/v (+-10) then g/b (+-1); amp via a/z (+-1%)."""
    # frequency
    while cur_hz != hz:
        if hz - cur_hz >= 10:
            send(ser, "f"); cur_hz += 10
        elif hz - cur_hz <= -10:
            send(ser, "v"); cur_hz -= 10
        elif hz > cur_hz:
            send(ser, "g"); cur_hz += 1
        else:
            send(ser, "b"); cur_hz -= 1
        # amp rides along proportionally
        if cur_amp < amp:
            send(ser, "a"); cur_amp += 1
        elif cur_amp > amp:
            send(ser, "z"); cur_amp -= 1
        time.sleep(step_dwell)
    while cur_amp != amp:
        send(ser, "a" if cur_amp < amp else "z")
        cur_amp += 1 if cur_amp < amp else -1
        time.sleep(step_dwell)
    return cur_hz, cur_amp


def measure_slow(cap):
    """(in-window ZC, median|oracle offset|%, lock_slow, jit_slow, iu_ma) from one capture."""
    from scope_common import analyze_zero_crossings, parse_cl_bounds
    bounds, _ = parse_cl_bounds(cap.text)
    secs, _, _ = analyze_zero_crossings(cap, smooth_window=3, sector_bounds=bounds)
    nzc = sum(1 for s in secs if s.status == "zc")
    offs = oracle_offsets(cap)
    moff = st.median([abs(v) for v in offs.values()]) if offs else float("nan")
    d = cap.debug
    g = lambda k: float(d.get(k, "nan"))  # noqa: E731
    return nzc, moff, g("lock_slow"), g("jit_slow"), g("iu_ma")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--freqs", type=int, nargs="+", default=[210, 250, 300, 350, 400, 450, 500])
    p.add_argument("--alpha", type=float, default=0.5, help="loop authority to hold while climbing")
    p.add_argument("--detector", choices=["linfit", "signchange"], default="signchange")
    p.add_argument("--predict-gate", type=float, default=0.5)
    p.add_argument("--amp-margin", type=float, default=4.0, help="amp %% above the stall curve")
    p.add_argument("--min-amp", type=float, default=24.0, help="amp floor (48k BEMF valid >=~22%%)")
    p.add_argument("--abort-lock", type=float, default=0.06,
                   help="stop climbing (and kill) once a tier's lock_slow drops below this -- "
                        "the loop has lost lock, so the next higher tier would hard-stall")
    p.add_argument("--snaps", type=int, default=3)
    p.add_argument("--dwell", type=float, default=1.5, help="settle (s) at each tier before measuring")
    p.add_argument("--ramp-dwell", type=float, default=0.12, help="dwell per f/a step while ramping")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--cmd-delay", type=float, default=0.02)
    p.add_argument("-v", "--verbose", action="count", default=0)
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    outdir = Path("logs") / f"climb_{stamp}"
    outdir.mkdir(parents=True, exist_ok=True)
    logf = open(outdir / "climb.log", "w", encoding="utf-8")
    rows = []

    def both(s):
        print(s)
        logf.write(s + "\n")
        logf.flush()

    base_hz = args.freqs[0]
    both(f"PHASE 4 climb -- alpha={args.alpha} detector={args.detector} gate={args.predict_gate}  (out: {outdir})")
    with serial.Serial(args.port, args.baud, timeout=0.1) as raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, logf)
        ser.reset_input_buffer()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)
        reset_cycles(ser, 0.4)
        amp0 = amp_for(base_hz, args.amp_margin, args.min_amp)
        position(ser, Setpoint(), base_hz, int(round(amp0 * 10)), qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        set_detector(ser, args.detector)
        set_predict_gate(ser, args.predict_gate)
        set_alpha(ser, args.alpha)
        _clear_stall(ser)
        both(f"  engaged @ {base_hz} Hz amp {amp0:.0f}% -- climbing\n")
        both(f"  {'hz':>4} {'amp%':>5} | in-win | oracle|off|% | lock_slow | jit_slow | jit_deg | iu_ma | note")
        cur_hz, cur_amp = base_hz, amp0
        try:
            for hz in args.freqs:
                amp = amp_for(hz, args.amp_margin, args.min_amp)
                cur_hz, cur_amp = ramp_to(ser, cur_hz, cur_amp, hz, amp, args.ramp_dwell)
                time.sleep(args.dwell)
                send(ser, "x")  # reset glitch so this tier's FAULT/maxrun is isolated
                time.sleep(0.3)
                drain(ser)
                if _stalled(ser):
                    both(f"  {hz:>4} {amp:>5.0f} | STALL/lock-loss while climbing")
                    break
                acc = defaultdict(list)
                for _ in range(args.snaps):
                    try:
                        cap = parse_capture(capture(ser, args.capture_timeout, "c"))
                    except Exception:
                        continue
                    if int(float(cap.debug.get("hz", "0"))) != hz:
                        continue
                    nzc, moff, ls, js, iu = measure_slow(cap)
                    for k, v in (("nzc", nzc), ("moff", moff), ("ls", ls), ("js", js), ("iu", iu)):
                        if v == v:
                            acc[k].append(v)
                med = {k: st.median(v) for k, v in acc.items() if v}
                ls = med.get("ls", float("nan"))      # x1000
                js = med.get("js", float("nan"))      # ticks x1000
                # jitter in electrical degrees: jit_ticks / ticks_per_sector * 60
                fps = 48000.0 / (hz * 6.0)
                jit_deg = (js / 1000.0) / fps * 60.0 if js == js else float("nan")
                rows.append((hz, amp, med, jit_deg))
                both(f"  {hz:>4} {amp:>5.0f} |  {med.get('nzc', float('nan')):3.0f}   |  "
                     f"{med.get('moff', float('nan')):6.1f}     |   {ls / 1000:5.3f}   |  "
                     f"{js / 1000:6.2f}  | {jit_deg:6.1f}  | {med.get('iu', float('nan')):5.0f} |")
                # Auto-abort: a tier whose lock_slow has collapsed means the loop lost lock;
                # climbing higher would drive the rotor into a hard stall. Stop here, killed.
                if ls == ls and ls / 1000 < args.abort_lock:
                    both(f"  -> lock_slow {ls / 1000:.3f} < {args.abort_lock} : lock lost, "
                         f"aborting climb at {hz} Hz (ceiling reached)")
                    break
        finally:
            send(ser, "w")
            set_stall_kill(ser, True)
            try:
                set_watchdog(ser, False)
            except Exception:
                pass

    if rows:
        ls0 = rows[0][2].get("ls", float("nan")) / 1000
        lsN = rows[-1][2].get("ls", float("nan")) / 1000
        both(f"\n  lock_slow {rows[0][0]}Hz->{rows[-1][0]}Hz: {ls0:.3f} -> {lsN:.3f}  "
             f"({'HOLDS' if lsN >= 0.5 * ls0 else 'DEGRADES'} across the climb)")
        both(f"  jit (deg) range: {min(r[3] for r in rows if r[3] == r[3]):.1f} .. "
             f"{max(r[3] for r in rows if r[3] == r[3]):.1f}")
    both(f"\n  log -> {outdir}/climb.log")
    logf.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
