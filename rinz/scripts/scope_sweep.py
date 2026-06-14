#!/usr/bin/env python3
"""Automated (frequency, amplitude) sweep collector for the rinz scope.

Drives the firmware over UART, walks a grid of electrical frequencies and PWM
amplitudes, and dumps the raw hex capture at each point to disk. NO images during
capture (slow + huge) -- render offline with sweep_map.py only where it's
interesting. Run this INSTEAD of scope_live_ui.py (serial is exclusive).

Strategy (smart two-pass; see sweep_map.py for analysis):
  - coarse:  python scripts/scope_sweep.py COM41 --amp-step 1.0 --snaps 2
  - fine:    python scripts/scope_sweep.py COM41 --amp-step 0.1 --snaps 5 \
                 --freq 180 220 --freq-jitter 1 --amp-min 8 --amp-max 12

Per frequency: q-reset -> co-ramp freq+amp up (tracking just above the measured
stall curve, so current stays low in transit) -> up to the ceiling -> sweep amp
DOWN to the floor, capturing --snaps dumps at each amp. Self-bounds with the stall
curve (no live stall detector). Setpoints are read back from the firmware echoes,
so the host never desyncs from the actual hz/amp.

Safety: ceiling is frequency-dependent to keep supply current bounded (current
falls with freq at fixed duty). Default 25% @100 Hz .. 30% @500 Hz. The low-freq
high-current corner is the heat risk -- transit through it is brief by design.
"""

from __future__ import annotations

import argparse
from datetime import datetime
from pathlib import Path
import re
import sys
import time

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

sys.path.insert(0, str(Path(__file__).parent))
from scope_common import BAUD

# Measured stall boundary (min running amp% before dropout) ~ 0.035*hz + 2.6.
STALL_A = 0.035
STALL_B = 2.6


def stall_amp(hz: float) -> float:
    return STALL_A * hz + STALL_B


def ceiling_amp(hz: float, lo: float, hi: float) -> float:
    """Frequency-dependent amplitude ceiling, lo @100 Hz rising to hi @500 Hz."""
    frac = (hz - 100.0) / 400.0
    return max(lo, min(hi, lo + (hi - lo) * frac))


# --- serial plumbing ---------------------------------------------------------

_AMP_RE = re.compile(r"amp=(\d+)\.(\d+)%")
_FREQ_RE = re.compile(r"freq=(\d+)Hz")


def drain(ser: serial.Serial) -> str:
    time.sleep(0.02)
    n = ser.in_waiting
    return ser.read(n).decode("ascii", errors="replace") if n else ""


def send(ser: serial.Serial, key: str) -> None:
    ser.write(key.encode("ascii"))
    ser.flush()


def read_until(ser: serial.Serial, marker: str, timeout: float) -> str:
    buf = ""
    deadline = time.time() + timeout
    while time.time() < deadline:
        n = ser.in_waiting
        chunk = ser.read(n) if n else ser.read(1)
        if chunk:
            buf += chunk.decode("ascii", errors="replace")
            if marker in buf:
                break
    return buf


class Setpoint:
    """Tracks the firmware's actual hz / amp(tenths) from its echoes."""

    def __init__(self) -> None:
        self.hz = 60
        self.amp_tenths = 80

    def update(self, text: str) -> None:
        for m in _FREQ_RE.finditer(text):
            self.hz = int(m.group(1))
        for m in _AMP_RE.finditer(text):
            self.amp_tenths = int(m.group(1)) * 10 + int(m.group(2))


def press(ser: serial.Serial, sp: Setpoint, key: str, count: int, delay: float = 0.015) -> None:
    for _ in range(abs(count)):
        send(ser, key)
        time.sleep(delay)
    sp.update(drain(ser))


def ramp_amp_to(ser: serial.Serial, sp: Setpoint, target_tenths: int) -> None:
    delta = target_tenths - sp.amp_tenths
    whole, tenths = divmod(abs(delta), 10)
    if delta >= 0:
        press(ser, sp, "a", whole)
        press(ser, sp, "+", tenths)
    else:
        press(ser, sp, "z", whole)
        press(ser, sp, "-", tenths)


def ramp_freq_to(ser: serial.Serial, sp: Setpoint, target_hz: int) -> None:
    delta = target_hz - sp.hz
    tens, ones = divmod(abs(delta), 10)
    if delta >= 0:
        press(ser, sp, "f", tens)
        press(ser, sp, "g", ones)
    else:
        press(ser, sp, "v", tens)
        press(ser, sp, "b", ones)


def reset_and_lock(ser: serial.Serial, sp: Setpoint, settle: float) -> None:
    send(ser, "q")
    time.sleep(settle)
    sp.hz, sp.amp_tenths = 60, 80
    sp.update(drain(ser))


def position(ser: serial.Serial, sp: Setpoint, hz: int, amp_tenths: int, settle: float) -> None:
    """Re-lock then walk up to (hz, amp), keeping amp just above stall in transit."""
    reset_and_lock(ser, sp, settle)
    # Co-ramp frequency up in 20 Hz steps, holding amp ~ stall(step)+3% so current
    # stays low at the low-frequency end.
    step_hz = 60
    while step_hz < hz:
        step_hz = min(step_hz + 20, hz)
        transit = int(round((stall_amp(step_hz) + 3.0) * 10))
        ramp_amp_to(ser, sp, max(transit, sp.amp_tenths))  # raise only, never dip
        ramp_freq_to(ser, sp, step_hz)
        time.sleep(0.05)
    ramp_amp_to(ser, sp, amp_tenths)
    time.sleep(settle)


def capture(ser: serial.Serial, timeout: float) -> str:
    drain(ser)
    send(ser, "d")
    return read_until(ser, "end", timeout)


# --- sweep -------------------------------------------------------------------


def frequency_list(args) -> list[int]:
    base = list(range(args.freq[0], args.freq[1] + 1, args.freq_step)) if len(args.freq) == 2 else args.freq
    out = []
    for f in base:
        for j in range(-args.freq_jitter, args.freq_jitter + 1):
            out.append(f + j)
    return out


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Automated freq/amp sweep collector for rinz scope")
    p.add_argument("port", help="UART port, e.g. COM41")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--freq", type=int, nargs="+", default=[100, 500],
                   help="freq start stop (with --freq-step), or an explicit list")
    p.add_argument("--freq-step", type=int, default=20)
    p.add_argument("--freq-jitter", type=int, default=0, help="also capture +/-N Hz around each setpoint")
    p.add_argument("--amp-step", type=float, default=1.0, help="amp step %% (1.0 coarse, 0.1 fine)")
    p.add_argument("--ceiling-lo", type=float, default=25.0, help="amp ceiling %% at 100 Hz")
    p.add_argument("--ceiling-hi", type=float, default=30.0, help="amp ceiling %% at 500 Hz")
    p.add_argument("--floor-margin", type=float, default=2.0, help="sweep down to stall(hz)-this %%")
    p.add_argument("--amp-min", type=float, default=None, help="override floor (targeted fine pass)")
    p.add_argument("--amp-max", type=float, default=None, help="override ceiling (targeted fine pass)")
    p.add_argument("--snaps", type=int, default=2, help="captures per (freq,amp) point")
    p.add_argument("--settle", type=float, default=0.2, help="seconds to settle after an amp change")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--outdir", type=Path, default=None)
    return p.parse_args()


def main() -> int:
    args = parse_args()
    freqs = frequency_list(args)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    outdir = args.outdir or Path("logs") / f"sweep_{stamp}"
    outdir.mkdir(parents=True, exist_ok=True)
    manifest = (outdir / "manifest.csv").open("w", encoding="ascii")
    manifest.write("hz_set,amp_set_pct,snap,file\n")

    # Estimate before committing.
    total = 0
    for f in freqs:
        ceil_pct = args.amp_max if args.amp_max is not None else ceiling_amp(f, args.ceiling_lo, args.ceiling_hi)
        floor_pct = args.amp_min if args.amp_min is not None else max(3.0, stall_amp(f) - args.floor_margin)
        steps = max(1, int(round((ceil_pct - floor_pct) / args.amp_step)) + 1)
        total += steps * args.snaps
    est_s = total * 0.6 + len(freqs) * 6  # ~0.6 s/capture + ~6 s/freq positioning
    print(f"sweep: {len(freqs)} freqs, ~{total} captures -> est {est_s/60:.0f} min, out={outdir}")
    print("Ctrl-C to abort; partial data is kept.")

    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        sp = Setpoint()
        done = 0
        try:
            for f in freqs:
                ceil_pct = args.amp_max if args.amp_max is not None else ceiling_amp(f, args.ceiling_lo, args.ceiling_hi)
                floor_pct = args.amp_min if args.amp_min is not None else max(3.0, stall_amp(f) - args.floor_margin)
                ceil_t, floor_t, step_t = int(round(ceil_pct * 10)), int(round(floor_pct * 10)), max(1, int(round(args.amp_step * 10)))
                position(ser, sp, f, ceil_t, args.settle)
                fpath = outdir / f"f{f:03d}hz.log"
                with fpath.open("a", encoding="ascii", errors="replace") as fh:
                    amp_t = ceil_t
                    while amp_t >= floor_t:
                        ramp_amp_to(ser, sp, amp_t)
                        time.sleep(args.settle)
                        for snap in range(args.snaps):
                            dump = capture(ser, args.capture_timeout)
                            fh.write(f"# hz_set={f} amp_set={amp_t/10:.1f} snap={snap} t={datetime.now().isoformat(timespec='milliseconds')}\n")
                            fh.write(dump if dump.endswith("\n") else dump + "\n")
                            manifest.write(f"{f},{amp_t/10:.1f},{snap},{fpath.name}\n")
                            manifest.flush()
                            done += 1
                        amp_t -= step_t
                print(f"  f={f}Hz done ({done}/{total} captures)")
            # Park safe.
            send(ser, "w")
        except KeyboardInterrupt:
            print("\naborted; parking motor")
            try:
                send(ser, "w")
            except Exception:
                pass
        finally:
            manifest.close()
    print(f"sweep complete: {done} captures in {outdir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
