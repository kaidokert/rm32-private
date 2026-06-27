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

  python scripts/cl_wave_sweep.py COM41 --hz 250 --amps 13 15 17 19 --alpha 0.75
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

from scope_common import BAUD, parse_cl_lf
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


def collect_wave(ser, snaps: int, timeout: float) -> dict[int, float]:
    """Median firmware-linfit ZC% per physical sector (0..5) over `snaps` captures."""
    acc: dict[int, list[float]] = defaultdict(list)
    for _ in range(snaps):
        dump = capture(ser, timeout, "c")
        for phys, vals in parse_cl_lf(dump).items():
            acc[phys].extend(vals)
    return {s: st.median(v) for s, v in acc.items() if v}


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250)
    p.add_argument("--amps", type=float, nargs="+", default=[13, 15, 17, 19],
                   help="amp %% values to sweep (the load-angle knob)")
    p.add_argument("--alpha", type=float, default=0.75)
    p.add_argument("--beta", type=float, default=0.6)
    p.add_argument("--snaps", type=int, default=6)
    p.add_argument("--settle", type=float, default=1.0)
    p.add_argument("--capture-timeout", type=float, default=4.0)
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    waves: dict[float, dict[int, float]] = {}

    with serial.Serial(args.port, args.baud, timeout=0.1) as ser:
        ser.reset_input_buffer()
        if set_watchdog(ser, True):
            print("watchdog ARMED")
        cur = int(round(args.amps[0] * 10))
        try:
            position(ser, Setpoint(), args.hz, cur, qsettle=0.8, ramp_step=20,
                     ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
            set_beta(ser, args.beta)
            set_alpha(ser, args.alpha)
            time.sleep(args.settle)
            print(f"\n  {'amp%':>5} | " + " ".join(f"s{s}" for s in range(6)) + "   (linfit ZC % per sector)")
            for amp in args.amps:
                cur = set_amp(ser, cur, int(round(amp * 10)))
                time.sleep(args.settle)
                w = collect_wave(ser, args.snaps, args.capture_timeout)
                waves[amp] = w
                row = " ".join(f"{w.get(s, float('nan')):3.0f}" for s in range(6))
                print(f"  {amp:>5.1f} | {row}")
        finally:
            set_alpha(ser, 0.0)
            send(ser, "w")
            try:
                set_watchdog(ser, False)
            except Exception:
                pass

    outdir = Path("logs")
    outdir.mkdir(exist_ok=True)
    (outdir / f"wave_{stamp}.json").write_text(
        json.dumps({str(a): w for a, w in waves.items()}, indent=2), encoding="ascii")
    try:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        fig, ax = plt.subplots(figsize=(9, 5))
        for amp, w in sorted(waves.items()):
            ys = [w.get(s, float("nan")) for s in range(6)]
            ax.plot(range(6), ys, "-o", label=f"{amp:.1f}%")
        ax.axhline(50, color="0.7", ls="--", lw=0.8, label="mid-window")
        ax.set_xlabel("physical sector (0..5 = one electrical rev)")
        ax.set_ylabel("firmware linfit ZC (% of window)")
        ax.set_title(f"Per-sector ZC wave vs amp @ {args.hz} Hz  "
                     f"(phase shifts with amp => load-angle; fixed => geometry)")
        ax.set_xticks(range(6))
        ax.grid(alpha=0.3)
        ax.legend(title="amp", fontsize=8)
        png = outdir / f"wave_{stamp}.png"
        fig.tight_layout()
        fig.savefig(png, dpi=110)
        print(f"\nwave plot -> {png}\njson -> {outdir / f'wave_{stamp}.json'}")
    except Exception as exc:
        print(f"(plot skipped: {exc}); json -> {outdir / f'wave_{stamp}.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
