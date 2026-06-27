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
                          linfit_mb, parse_capture, parse_cl_bounds)
from scope_sweep import Setpoint, capture, position, send, set_watchdog

FLOAT_SECTOR = {"A": (2, 5), "B": (1, 4), "C": (0, 3)}  # each phase's two float sectors


def set_alpha(ser, alpha: float) -> None:
    send(ser, "0")
    for _ in range(int(round(max(0.0, min(1.0, alpha)) / 0.05))):
        send(ser, "m")
        time.sleep(0.02)


def set_beta(ser, beta: float) -> None:
    for _ in range(20):
        send(ser, ",")
        time.sleep(0.01)
    for _ in range(int(round(max(0.0, min(0.95, beta)) / 0.05))):
        send(ser, ".")
        time.sleep(0.01)


def set_amp(ser, cur_tenths: int, tgt_tenths: int) -> int:
    """Ramp amp from cur to tgt (0.1%% units) via a/z (+/-1%%) then +/- (+/-0.1%%)."""
    d = tgt_tenths - cur_tenths
    full, tenths = abs(d) // 10, abs(d) % 10
    for _ in range(full):
        send(ser, "a" if d > 0 else "z")
        time.sleep(0.05)
    for _ in range(tenths):
        send(ser, "+" if d > 0 else "-")
        time.sleep(0.03)
    return tgt_tenths


def collect_wave(ser, snaps: int, timeout: float, slope_min: float = 15.0) -> dict[int, float]:
    """Median UNGATED python-linfit ZC% per physical sector (0..5) over `snaps` captures.

    We use the python linfit (not the firmware lf) because the firmware lf is gated to
    [-30,130]% -- in open loop the crossings sit far out-of-window and would all reject to
    nan. The ungated fit projects a crossing for every sector with a real slope; only
    genuinely FLAT windows (|slope| < slope_min, e.g. on the BEMF flat-top or a driven
    rail) are dropped as unobservable. Closed-loop captures get the actual bnd= boundaries;
    open-loop ones fall back to the uniform grid."""
    acc: dict[int, list[float]] = defaultdict(list)
    for _ in range(snaps):
        cap = parse_capture(capture(ser, timeout, "c"))
        hz = float(cap.debug.get("hz", "0") or 0)
        if hz <= 0:
            continue
        fps = cap.sample_hz / (hz * 6.0)
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
    return {s: st.median(v) for s, v in acc.items() if v}


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
    p.add_argument("--beta", type=float, default=0.6)
    p.add_argument("--snaps", type=int, default=6)
    p.add_argument("--settle", type=float, default=1.0)
    p.add_argument("--capture-timeout", type=float, default=4.0)
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    freqs = args.freqs or [args.hz]
    grid: dict[tuple[int, float], dict[int, float]] = {}  # (hz, amp) -> wave

    with serial.Serial(args.port, args.baud, timeout=0.1) as ser:
        ser.reset_input_buffer()
        if set_watchdog(ser, True):
            print("watchdog ARMED")
        try:
            for hz in freqs:
                cur = int(round(args.amps[0] * 10))
                # Re-spin at each frequency (a big open-loop freq jump would lose sync).
                position(ser, Setpoint(), hz, cur, qsettle=0.8, ramp_step=20,
                         ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
                set_beta(ser, args.beta)
                set_alpha(ser, args.alpha)
                time.sleep(args.settle)
                # Spin check: if the motor isn't producing valid BEMF here (too slow/fast
                # for the open-loop envelope), skip this frequency rather than nan-spam.
                probe = collect_wave(ser, 1, args.capture_timeout)
                if len(probe) < 3:
                    print(f"\n  {hz} Hz: not spinning open-loop ({len(probe)}/6 sectors) -- skipping")
                    continue
                print(f"\n  {hz} Hz   {'amp%':>5} | " + " ".join(f"s{s}" for s in range(6))
                      + "   (linfit ZC % per sector)")
                for amp in args.amps:
                    cur = set_amp(ser, cur, int(round(amp * 10)))
                    time.sleep(args.settle)
                    w = collect_wave(ser, args.snaps, args.capture_timeout)
                    grid[(hz, amp)] = w
                    row = " ".join(f"{w.get(s, float('nan')):3.0f}" for s in range(6))
                    print(f"  {hz:>5} Hz {amp:>5.1f} | {row}")
                set_alpha(ser, 0.0)  # park between frequencies
        finally:
            set_alpha(ser, 0.0)
            send(ser, "w")
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
