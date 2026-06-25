#!/usr/bin/env python3
"""Long-running glitch monitor for the Path-B closed loop (examples/scope_cl2.rs).

The audible clicks are RARE (seconds apart) -- far rarer than the steady ~4% ZC misses
and impossible to catch in a single 2-rev capture. The firmware now counts them over a
long window (coast BURSTS = >=2 consecutive missed ZCs = a momentary lock loss; big-
RESIDUAL hits = a found ZC that jumped far from its smoothed position) and self-emits a
`glitch:` line each second. This script spins up, parks at the operating point, runs a
timed window, and reports the totals -- the honest replacement for listening.

  python scripts/cl_glitch_mon.py COM41 --hz 250 --amp 13 --alpha 0.75 --secs 30
  python scripts/cl_glitch_mon.py COM41 --hz 250 --amp 13 --alpha 0.75 --secs 60 --ab

--ab runs the window twice (predict-coast ON then OFF) and compares, so you finally get
numbers on whether predictive coast reduces the rare events. ATTENDED use (watchdog
armed; the script pets it with newlines and parks at alpha=0 + kill on exit).
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

from scope_common import BAUD
from scope_sweep import Setpoint, position, send, set_watchdog

_G_RE = re.compile(
    r"glitch:\s*comm=(\d+)\s+coast=(\d+)\s+burst=(\d+)\s+resid=(\d+)\s+"
    r"maxrun=(\d+)\s+since=(\d+)\s+lockS=(\d+)\s+alpha=(\d+)\s+beta=(\d+)\s+pc=(\d+)"
)
_H_RE = re.compile(r"hist:\s*runlen=([\d,]+)\s+resid=([\d,]+)")
RUN_LABELS = ["1", "2", "3", "4", "5", "6", "7", "8+"]
RESID_LABELS = ["<0.5", "0.5-1", "1-2", "2-4", "4-8", ">=8"]


def fmt_hist(title: str, labels: list[str], bins: list[int]) -> list[str]:
    total = sum(bins) or 1
    peak = max(bins) or 1
    out = [f"  {title}:"]
    for lab, c in zip(labels, bins):
        bar = "#" * int(round(c / peak * 34))
        out.append(f"    {lab:>5} | {bar:<34} {c}  ({c / total * 100:.1f}%)")
    return out


def set_alpha(ser, alpha: float) -> None:
    """Park alpha deterministically: '0' (=0.0) then 'm' (+0.05) steps up to target."""
    send(ser, "0")
    for _ in range(int(round(max(0.0, min(1.0, alpha)) / 0.05))):
        send(ser, "m")
        time.sleep(0.02)


def set_beta(ser, beta: float) -> None:
    for _ in range(20):  # floor to 0 (each ',' = -0.05, clamps)
        send(ser, ",")
        time.sleep(0.01)
    for _ in range(int(round(max(0.0, min(0.95, beta)) / 0.05))):
        send(ser, ".")
        time.sleep(0.01)


def run_window(ser, secs: float) -> dict | None:
    """Reset counters, run for `secs` (petting the watchdog), return the last glitch line's
    totals (counters are cumulative since reset = window totals). Assumes the monitor is
    already on (toggled once in main). None if no data (loop never locked)."""
    send(ser, "x")  # reset glitch counters
    ser.reset_input_buffer()
    last = None
    last_run = last_resid = None
    buf = ""
    t0 = time.monotonic()
    t_pet = t0
    while time.monotonic() - t0 < secs:
        buf += ser.read(256).decode("ascii", errors="replace")
        while "\n" in buf:
            line, buf = buf.split("\n", 1)
            m = _G_RE.search(line)
            if m:
                last = {
                    "comm": int(m[1]), "coast": int(m[2]), "burst": int(m[3]),
                    "resid": int(m[4]), "maxrun": int(m[5]), "since": int(m[6]),
                    "lockS": int(m[7]) / 10.0, "alpha": int(m[8]) / 1000.0,
                    "beta": int(m[9]) / 1000.0, "pc": int(m[10]),
                }
            h = _H_RE.search(line)
            if h:
                last_run = [int(v) for v in h[1].split(",")]
                last_resid = [int(v) for v in h[2].split(",")]
        now = time.monotonic()
        if now - t_pet >= 3.0:
            send(ser, "\n")  # pet the watchdog without issuing a command
            t_pet = now
    if last is not None:
        last["run_hist"] = last_run
        last["resid_hist"] = last_resid
    return last


def fmt_report(tag: str, secs: float, d: dict) -> str:
    coast_pct = d["coast"] / d["comm"] * 100 if d["comm"] else float("nan")
    out = [
        f"=== {tag}  ({secs:.0f}s, alpha={d['alpha']:.2f} beta={d['beta']:.2f} "
        f"pc={'on' if d['pc'] else 'off'}) ===",
        f"  commutations : {d['comm']}",
        f"  coasts       : {d['coast']}  ({coast_pct:.1f}%)   lockS {d['lockS']:.1f}%",
        f"  coast bursts : {d['burst']}  (>=2 consec)  -> {d['burst'] / secs:.2f}/s",
        f"  big-resid    : {d['resid']}  (ZC jump >thr) -> {d['resid'] / secs:.2f}/s",
        f"  max coast run: {d['maxrun']}",
    ]
    if d.get("run_hist"):
        out += fmt_hist("coast run lengths (lock-loss depth)", RUN_LABELS, d["run_hist"])
    if d.get("resid_hist"):
        out += fmt_hist("ZC residual magnitude, ticks (jitter shape)", RESID_LABELS, d["resid_hist"])
    return "\n".join(out)


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250)
    p.add_argument("--amp", type=float, default=13.0)
    p.add_argument("--alpha", type=float, default=0.75)
    p.add_argument("--beta", type=float, default=0.6)
    p.add_argument("--secs", type=float, default=30.0)
    p.add_argument("--ab", action="store_true", help="compare predict-coast ON vs OFF")
    p.add_argument("--log", type=Path, default=None,
                   help="summary log path (default logs/glitch_<timestamp>.log)")
    args = p.parse_args()

    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    log_path = args.log or Path("logs") / f"glitch_{stamp}.log"
    blocks = [f"# glitch monitor {datetime.now().isoformat(timespec='seconds')}  "
              f"hz={args.hz} amp={args.amp} alpha={args.alpha} beta={args.beta} "
              f"secs={args.secs} ab={args.ab}"]

    def emit(text: str) -> None:  # to stdout AND the summary log buffer
        print(text)
        blocks.append(text)

    with serial.Serial(args.port, args.baud, timeout=0.1) as ser:
        ser.reset_input_buffer()
        if set_watchdog(ser, True):
            print("watchdog ARMED (script pets it; parks at alpha=0 + kill on exit)")
        try:
            position(ser, Setpoint(), args.hz, int(round(args.amp * 10)),
                     qsettle=0.8, ramp_step=20, ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
            set_beta(ser, args.beta)
            set_alpha(ser, args.alpha)
            send(ser, "i")  # monitor on
            time.sleep(1.0)  # let it settle into lock before the window

            d_on = run_window(ser, args.secs)
            if d_on is None:
                print("no glitch data (did the loop lock? check hz/amp/alpha)")
            else:
                emit(fmt_report("predict ON" if args.ab else "window", args.secs, d_on))

            if args.ab and d_on is not None:
                send(ser, "y")  # toggle predict OFF
                time.sleep(1.0)
                d_off = run_window(ser, args.secs)
                if d_off is not None:
                    emit(fmt_report("predict OFF", args.secs, d_off))
                    db, dr = d_off["burst"] - d_on["burst"], d_off["resid"] - d_on["resid"]
                    deep_on = sum(d_on["run_hist"][3:]) if d_on.get("run_hist") else None
                    deep_off = sum(d_off["run_hist"][3:]) if d_off.get("run_hist") else None
                    verdict = ("predict ON has FEWER glitch events" if (db > 0 or dr > 0)
                               else "no clear glitch reduction from predict")
                    deep = (f", deep(run>=4) {deep_on}->{deep_off}"
                            if deep_on is not None else "")
                    emit(f"ON vs OFF: bursts {d_on['burst']}->{d_off['burst']}, "
                         f"big-resid {d_on['resid']}->{d_off['resid']}{deep}  <- {verdict}")
        finally:
            # Kill the motor FIRST (safety), then persist the summary -- a log-write
            # failure must never leave the motor energized.
            send(ser, "i")  # monitor off (best effort)
            send(ser, "0")
            send(ser, "w")
            try:
                set_watchdog(ser, False)
            except Exception:
                pass
            log_path.parent.mkdir(parents=True, exist_ok=True)
            log_path.write_text("\n\n".join(blocks) + "\n", encoding="ascii", errors="replace")
            print(f"\nsummary -> {log_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
