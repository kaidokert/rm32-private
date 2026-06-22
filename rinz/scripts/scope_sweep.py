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
from scope_common import BAUD, classify_rotor_state, deinterleave, parse_capture

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


def reset_and_lock(ser: serial.Serial, sp: Setpoint, qsettle: float) -> bool:
    """Send 'q' and CONFIRM the firmware echoed 'reset:' before trusting it; retry
    if not (a dropped 'q' is what desyncs the setpoint tracking and wanders). Then
    wait qsettle for the rotor to actually spin up and lock from rest."""
    for _ in range(3):
        drain(ser)
        send(ser, "q")
        echo = read_until(ser, "reset:", 1.5)
        if "reset:" in echo:
            sp.hz, sp.amp_tenths = 60, 90
            sp.update(echo)
            time.sleep(qsettle)
            return True
    return False


def position(
    ser: serial.Serial,
    sp: Setpoint,
    hz: int,
    amp_tenths: int,
    *,
    qsettle: float,
    ramp_step: int,
    ramp_dwell: float,
    transit_margin: float,
    settle: float,
) -> None:
    """Re-lock then walk frequency up to the target SLOWLY (the rotor has to
    physically accelerate to follow), holding amp ~ stall(step)+margin. The margin
    must clear the lock-catch (~stall+6%, measured at 440/500 Hz) so the rotor
    engages TRUE synchronous lock rather than the low-current below-catch (slip /
    subharmonic) branch -- the catch is hysteretic, so once engaged from above it
    rides down the locked branch into the sweep range."""
    if not reset_and_lock(ser, sp, qsettle):
        reset_and_lock(ser, sp, qsettle)  # one more try; proceed regardless
    step_hz = 60
    while step_hz < hz:
        step_hz = min(step_hz + ramp_step, hz)
        transit = int(round((stall_amp(step_hz) + transit_margin) * 10))
        ramp_amp_to(ser, sp, max(transit, sp.amp_tenths))  # raise only, never dip
        ramp_freq_to(ser, sp, step_hz)
        time.sleep(ramp_dwell)
    ramp_amp_to(ser, sp, amp_tenths)
    time.sleep(max(settle, 0.3))


_TERM_RE = re.compile(r"[\r\n]end\b")


def capture(ser: serial.Serial, timeout: float, key: str = "c") -> str:
    """Trigger one capture ('c' = binary Ascii85, ~2x faster than 'd' = hex) and read
    until the line-anchored 'end' -- b85 payload can contain 'end' as a substring, so
    a bare `marker in buf` would stop early."""
    drain(ser)
    send(ser, key)
    buf = ""
    deadline = time.time() + timeout
    while time.time() < deadline:
        n = ser.in_waiting
        chunk = ser.read(n) if n else ser.read(1)
        if chunk:
            buf += chunk.decode("ascii", errors="replace")
            if _TERM_RE.search(buf):
                break
    return buf


def set_watchdog(ser: serial.Serial, want_armed: bool) -> bool:
    """Drive the firmware command-watchdog to a known state. 'p' TOGGLES it, so we
    send + read the echo and retry if it went the wrong way. Armed = the firmware
    kills the motor itself after ~10 s of no serial activity (protects unattended
    runs against a host hang -- the thing that cooks a stalled motor)."""
    target = "ARMED" if want_armed else "disarmed"
    other = "disarmed" if want_armed else "ARMED"
    for _ in range(3):
        send(ser, "p")
        time.sleep(0.2)
        echo = drain(ser)
        if target in echo:
            return True
        if other not in echo:  # no echo at all -> firmware busy; brief wait, retry
            time.sleep(0.2)
    return False


_IU_RE = re.compile(r"iu_ma=(\d+)")


def dump_iu_ma(dump: str):
    m = _IU_RE.search(dump)
    return int(m.group(1)) if m else None


def rotor_state(ser: serial.Serial, args) -> str:
    """One capture -> rotor state: 'locked' / 'stalled' / 'uncertain' / '?' (capture
    or parse failed = firmware not dumping). Used to verify a frequency actually
    spun up before sweeping its amps -- so an unlockable freq is SKIPPED, not dwelt
    on (which crawls on timeouts and cooks the rotor)."""
    dump = capture(ser, args.capture_timeout, "d" if args.hex else "c")
    try:
        cap = parse_capture(dump)
        cap.debug.setdefault("mode", "six-step")
        # scope2 dual captures interleave valley/peak frames -- they zigzag, so the
        # raw lowpass smears late_swing to 0 and FALSELY reads "stalled". Classify the
        # clean valley sub-stream instead.
        if cap.interleaved:
            cap = deinterleave(cap)[0]
        return classify_rotor_state(cap).get("state", "?")
    except Exception:
        return "?"


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
    p.add_argument("--qsettle", type=float, default=0.8, help="spin-up wait after 'q' reset")
    p.add_argument("--ramp-step", type=int, default=20, help="freq ramp step Hz while positioning")
    p.add_argument("--ramp-dwell", type=float, default=0.3,
                   help="dwell s per freq ramp step (20Hz/0.3s = ~67Hz/s; lower=faster, slip risk)")
    p.add_argument("--transit-margin", type=float, default=8.0,
                   help="amp %% above stall held during freq ramp. MUST clear the lock-catch "
                        "(~stall+6%%) or the rotor rides the below-catch slip branch, not true lock.")
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--hex", action="store_true", help="hex dumps ('d') instead of binary Ascii85 ('c', default, ~2x faster)")
    p.add_argument("--current-limit", type=int, default=3500, help="iu_ma over this for 2 consecutive captures -> kill + skip freq (host-alive stall guard)")
    p.add_argument("--no-verify", action="store_true",
                   help="skip the spin-verify gate -- capture every amp regardless of the stall "
                        "heuristic (for attended single-freq probes; classify offline). The "
                        "--current-limit guard still protects against a cooking stall.")
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
    pos_s = 0.0
    for f in freqs:
        ceil_pct = args.amp_max if args.amp_max is not None else round(ceiling_amp(f, args.ceiling_lo, args.ceiling_hi))
        floor_pct = args.amp_min if args.amp_min is not None else round(max(3.0, stall_amp(f) - args.floor_margin))
        steps = max(1, int(round((ceil_pct - floor_pct) / args.amp_step)) + 1)
        total += steps * args.snaps
        # positioning: ~2.3 s for q+spin-up, plus the slow freq ramp up from 60
        pos_s += 2.3 + max(0, (f - 60)) / max(1, args.ramp_step) * (args.ramp_dwell + 0.15) + 0.4
    est_s = total * 0.6 + pos_s
    print(f"sweep: {len(freqs)} freqs, ~{total} captures -> est {est_s/60:.0f} min, out={outdir}")
    print("Ctrl-C to abort; partial data is kept.")

    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        sp = Setpoint()
        done = 0
        if set_watchdog(ser, True):
            print("watchdog ARMED -- firmware self-kills the motor after ~10s of no serial "
                  "(a host hang can't cook the motor); disarmed on clean exit/Ctrl-C")
        else:
            print("WARNING: watchdog did NOT arm -- do NOT leave this run unattended")
        try:
            consec_skips = 0
            for f in freqs:
                ceil_pct = args.amp_max if args.amp_max is not None else ceiling_amp(f, args.ceiling_lo, args.ceiling_hi)
                floor_pct = args.amp_min if args.amp_min is not None else max(3.0, stall_amp(f) - args.floor_margin)
                ceil_t, floor_t, step_t = int(round(ceil_pct * 10)), int(round(floor_pct * 10)), max(1, int(round(args.amp_step * 10)))
                pos_kw = dict(qsettle=args.qsettle, ramp_step=args.ramp_step,
                              ramp_dwell=args.ramp_dwell, transit_margin=args.transit_margin, settle=args.settle)
                position(ser, sp, f, ceil_t, **pos_kw)
                # Verify the rotor actually spun up before sweeping amps. An unlockable
                # frequency is SKIPPED (kill + next freq), not dwelt on -- dwelling is
                # what crawled on timeouts and cooked the rotor in the wedged runs.
                # --no-verify bypasses this for attended single-freq probes.
                st = rotor_state(ser, args) if not args.no_verify else "locked"
                if st in ("stalled", "?"):
                    position(ser, sp, f, ceil_t, **pos_kw)  # one more lock attempt
                    st = rotor_state(ser, args)
                if st in ("stalled", "?"):
                    send(ser, "w")
                    consec_skips += 1
                    print(f"  SKIP {f}Hz: rotor not spinning ({st} x2) -- can't lock "
                          f"[{consec_skips} in a row]")
                    if consec_skips >= 8:
                        print("  ABORT: 8 frequencies failed in a row -- firmware/motor "
                              "likely dead (check the bench / power-cycle)")
                        break
                    continue
                consec_skips = 0  # this freq locked -> reset the run
                fpath = outdir / f"f{f:03d}hz.log"
                with fpath.open("a", encoding="ascii", errors="replace") as fh:
                    amp_t = ceil_t
                    over = 0
                    aborted = False
                    while amp_t >= floor_t and not aborted:
                        ramp_amp_to(ser, sp, amp_t)
                        time.sleep(args.settle)
                        for snap in range(args.snaps):
                            dump = capture(ser, args.capture_timeout, "d" if args.hex else "c")
                            # Host-alive stall guard: sustained over-current = likely a
                            # stalled rotor cooking. Kill and skip the rest of this freq.
                            iu = dump_iu_ma(dump)
                            if iu is not None and iu > args.current_limit:
                                over += 1
                                if over >= 2:
                                    send(ser, "w")
                                    print(f"  CURRENT ABORT: iu_ma={iu} > {args.current_limit} "
                                          f"at {f}Hz/{amp_t/10:.1f}% -- killed, skipping this freq")
                                    aborted = True
                                    break
                            else:
                                over = 0
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
            # Clean exit / Ctrl-C disarms. A HANG never reaches here -> watchdog stays
            # armed -> firmware fires -> motor safe.
            try:
                set_watchdog(ser, False)
            except Exception:
                pass
            manifest.close()
    print(f"sweep complete: {done} captures in {outdir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
