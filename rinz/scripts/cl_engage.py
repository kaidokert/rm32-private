#!/usr/bin/env python3
"""Phase 3 — ENGAGE THE CLOSED LOOP, measurably.

Spin up open-loop at a safe speed, then step `alpha` 0 -> 0.2 -> 0.5 -> 1.0 (the loop's
bounded steering authority) and MEASURE what the loop does at each step. The whole point is
observability: does engaging the loop pull the BEMF zero crossings into the float windows,
drop the current (lock-catch), and raise the firmware's own lock metric -- or does it lose
lock and fall back? Either outcome is recorded with numbers and pictures, not asserted.

Per alpha step it captures `--snaps` frames and reports, as medians:
  * in-window ZC count  -- directly-observed sign-change crossings (analyze_zero_crossings,
    status=='zc'); rises if the loop centers the ZC in the window.
  * oracle |offset|     -- validated harmonic-fit per-sector crossing distance from window
    centre (% of window); falls toward 0 as the loop centers the crossings.
  * lock_fast / jit     -- the firmware's own lock & jitter metrics from the debug line.
  * iu_ma               -- phase-U current proxy; the lock-catch shows as a current DROP.
Raw captures + an oracle render at the first and last alpha are saved for eyeballing.

  python scripts/cl_engage.py COM41 --hz 250 --amp 28
  python scripts/cl_engage.py COM41 --hz 300 --amp 30 --alphas 0 0.2 0.5 1.0 --snaps 5

ATTENDED. The motor spins. Bounded authority: the loop only nudges commutation within
+-CL_SLEW_FRAC of the open-loop period, so it cannot run away; alpha=0 / w / q are instant
fallbacks. Built for the scope_cl2 / scope_cl2_48k firmware (Path B).
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

import scope_common as sc
# Shim the reversed-drive tables (unused for forward captures) so the stale zc_fit imports.
sc.SIX_STEP_HIGH_REV = sc.SIX_STEP_HIGH
sc.SIX_STEP_LOW_REV = sc.SIX_STEP_LOW
import zc_fit as zf  # noqa: E402
from scope_common import (BAUD, analyze_zero_crossings, parse_capture,  # noqa: E402
                          parse_cl_bounds)
from scope_sweep import Setpoint, capture, drain, position, send, set_watchdog  # noqa: E402
from cl_wave_sweep import (PacedSerial, _clear_stall, _stalled, reset_cycles,  # noqa: E402
                           set_alpha, set_stall_kill)

TWO_PI = 2.0 * math.pi
SECTOR_RAD = TWO_PI / 6.0
# alpha is set finely via set_alpha() (firmware '0' then n/m +-0.05 steps), so any value in
# [0,1] is reachable -- not just the 4 preset keys. Quantized to 0.05 by the firmware step.


def oracle_offsets(cap):
    """Validated harmonic-fit per-sector crossing offset from window centre (% window).
    Lower |offset| = crossing nearer the window middle = better centered by the loop."""
    samples = zf.collect_phase_samples([cap], smooth_window=5, blank_frames=3, skip_first_rev=True)
    fits = {}
    for (_hz, _rev, ph), (ang, val) in samples.items():
        if len(ang) >= 8:
            a, b, c, _rms = zf.fit_sinusoid(ang, val)
            fits[ph] = zf.zero_crossings(a, b, c)
    offs = {}
    for k in range(6):
        fl = zf.phys_float(k, False)
        if fl not in fits or not fits[fl]:
            continue
        center = (k + 0.5) * SECTOR_RAD
        t, _d = min(fits[fl], key=lambda td: abs(((td[0] - center + math.pi) % TWO_PI) - math.pi))
        offs[k] = (((t - center + math.pi) % TWO_PI) - math.pi) / SECTOR_RAD * 100.0
    return offs


def measure(cap):
    """One capture -> (in-window ZC count, median |oracle offset|, lock_fast, jit_fast, iu_ma)."""
    bounds, _ = parse_cl_bounds(cap.text)
    secs, _, _ = analyze_zero_crossings(cap, smooth_window=3, sector_bounds=bounds)
    nzc = sum(1 for s in secs if s.status == "zc")
    offs = oracle_offsets(cap)
    moff = st.median([abs(v) for v in offs.values()]) if offs else float("nan")
    d = cap.debug
    f = lambda k: float(d.get(k, "nan"))  # noqa: E731
    return nzc, moff, f("lock_fast"), f("jit_fast"), f("iu_ma")


def render(cap, path):
    try:
        import zc_oracle_render as zr
        zr.render(cap, path)
    except Exception as exc:
        print(f"  (render {path.name} skipped: {exc})")


def set_detector(ser, want):
    """Drive the firmware loop detector to `want` ('linfit' | 'signchange'). The 'j' key
    toggles; send + read the "detector=..." echo and retry until it matches. Returns the
    resulting name or None. (Old firmware without 'j' just won't echo -> returns None.)"""
    import re
    for _ in range(4):
        from scope_sweep import drain
        from scope_sweep import send as _send
        cur = re.search(r"detector=(linfit|signchange)", drain(ser))
        if cur and cur.group(1) == want:
            return want
        _send(ser, "j")
        time.sleep(0.15)
        mo = re.search(r"detector=(linfit|signchange)", drain(ser))
        if mo and mo.group(1) == want:
            return want
    return None


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250, help="electrical frequency to hold (open-loop governed)")
    p.add_argument("--amp", type=float, default=28.0, help="amp %% (>=24 for valid BEMF on the 48k tool)")
    p.add_argument("--alphas", type=float, nargs="+",
                   default=[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.7, 1.0],
                   help="alpha steps to engage (any value in [0,1]; firmware quantizes to 0.05)")
    p.add_argument("--detector", choices=["linfit", "signchange"], default="signchange",
                   help="loop detector source: signchange (straddle-gated, clean) or linfit "
                        "(legacy extrapolator). Needs the 'j'-toggle firmware; ignored on old builds.")
    p.add_argument("--snaps", type=int, default=4, help="captures averaged per alpha step")
    p.add_argument("--dwell", type=float, default=1.5, help="settle (s) after each alpha change")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--cmd-delay", type=float, default=0.02)
    p.add_argument("--wait-port", type=float, default=0.0,
                   help="seconds to wait/retry if the port is held by another tool (the live UI)")
    p.add_argument("-v", "--verbose", action="count", default=0)
    args = p.parse_args()

    if any(a < 0.0 or a > 1.0 for a in args.alphas):
        raise SystemExit("alpha values must be in [0, 1]")

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    outdir = Path("logs") / f"engage_{stamp}"
    outdir.mkdir(parents=True, exist_ok=True)
    logf = open(outdir / "engage.log", "w", encoding="utf-8")
    amp_t = int(round(args.amp * 10))
    rows = []

    def both(s):
        print(s)
        logf.write(s + "\n")
        logf.flush()

    both(f"PHASE 3 -- engage the loop @ {args.hz} Hz, amp {args.amp:.1f}%  (out: {outdir})")
    # Wait for the port if another tool (the live UI) holds it -- so this auto-runs the
    # instant that UI is closed, no second launch needed.
    deadline = time.time() + args.wait_port
    raw = None
    while raw is None:
        try:
            raw = serial.Serial(args.port, args.baud, timeout=0.1)
        except serial.SerialException as exc:
            if time.time() >= deadline:
                both(f"  {args.port} busy and --wait-port elapsed: {exc}")
                logf.close()
                return 1
            both(f"  {args.port} busy (close the live UI to start)... retrying")
            time.sleep(2.0)
    with raw:
        ser = PacedSerial(raw, args.verbose, args.cmd_delay, logf)
        ser.reset_input_buffer()
        set_watchdog(ser, True)
        set_stall_kill(ser, False)  # let the host see lock-loss; don't let fw kill mid-step
        reset_cycles(ser, 0.4)
        # Spin up SOLID, open-loop (alpha=0 after q), at the target setpoint.
        position(ser, Setpoint(), args.hz, amp_t, qsettle=0.8, ramp_step=20,
                 ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
        drain(ser)
        # Set the loop detector AFTER all the q/reset spin-up so it sticks for the alpha sweep.
        got_det = set_detector(ser, args.detector)
        both(f"  loop detector: {args.detector}" +
             ("" if got_det == args.detector else "  (WARNING: firmware did not confirm -- old build without 'j'?)"))
        # Discard any spin-up stall flag: the open-loop freq-ramp can false-trip the stall
        # detector (the firmware now gates FAULT emission on alpha>0, but the host may have
        # latched a STALL from a pre-gate build or a ramp transient). Start the sweep clean.
        _clear_stall(ser)
        both(f"\n  alpha | in-win ZC | oracle|off|% | lock_fast |  jit  |  iu_ma  | note")
        try:
            for ai, a in enumerate(args.alphas):
                set_alpha(ser, a)                 # engage / change loop authority (fine, n/m steps)
                time.sleep(args.dwell)
                if _stalled(ser):
                    both(f"  {a:5.1f} |  --       |   --        |    --     |   --  |   --    | STALL/lock-loss")
                    _clear_stall(ser)
                    break
                acc = defaultdict(list)
                first_cap = None
                for _ in range(args.snaps):
                    try:
                        cap = parse_capture(capture(ser, args.capture_timeout, "c"))
                    except Exception:
                        continue
                    if int(float(cap.debug.get("hz", "0"))) != args.hz:
                        continue
                    first_cap = first_cap or cap
                    nzc, moff, lf, jit, iu = measure(cap)
                    for k, v in (("nzc", nzc), ("moff", moff), ("lf", lf), ("jit", jit), ("iu", iu)):
                        if v == v:  # not nan
                            acc[k].append(v)
                med = {k: st.median(v) for k, v in acc.items() if v}
                rows.append((a, med))
                # Save raw + an oracle render for the first and last alpha (visual proof).
                if first_cap is not None:
                    (outdir / f"alpha_{a:.1f}.log").write_text(first_cap.text, encoding="utf-8", errors="replace")
                    if ai == 0 or ai == len(args.alphas) - 1:
                        render(first_cap, outdir / f"alpha_{a:.1f}.png")
                both(f"  {a:5.1f} |  {med.get('nzc', float('nan')):4.0f}     |  {med.get('moff', float('nan')):6.1f}     |"
                     f"  {med.get('lf', float('nan')):6.0f}   | {med.get('jit', float('nan')):5.0f} |"
                     f" {med.get('iu', float('nan')):6.0f}  |")
        finally:
            send(ser, "w")
            set_stall_kill(ser, True)
            try:
                set_watchdog(ser, False)
            except Exception:
                pass

    # Verdict: did engaging the loop measurably help vs alpha=0 baseline?
    if len(rows) >= 2:
        base, last = rows[0][1], rows[-1][1]
        def delta(k, sign):  # sign=+1 means "up is good", -1 means "down is good"
            if k in base and k in last:
                d = last[k] - base[k]
                good = (d > 0) == (sign > 0)
                return f"{base[k]:.0f}->{last[k]:.0f} ({'+' if d>=0 else ''}{d:.0f}) {'OK' if good else '--'}"
            return "n/a"
        both("\n  VERDICT (alpha 0 -> {:.1f}):".format(rows[-1][0]))
        both(f"    in-window ZC : {delta('nzc', +1)}   (loop centering the crossings -> more in-window)")
        both(f"    oracle|off|% : {delta('moff', -1)}   (crossings pulled toward window centre)")
        both(f"    lock_fast    : {delta('lf', +1)}   (firmware lock metric)")
        both(f"    iu_ma        : {delta('iu', -1)}   (lock-catch: synchronous drive draws less current)")
    both(f"\n  raw + renders -> {outdir}")
    logf.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
