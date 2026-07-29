#!/usr/bin/env python3
"""ISR-delay injection sweep for am32_clone — the CPU / critical-section
causal test for the deaf-window BEMF-miss hypothesis (2026-07-26).

HYPOTHESIS UNDER TEST: a deaf window (the COMP priority-0 ISR missing a
real mid-window BEMF crossing) is caused NOT by total ISR budget but by
PRIMASK critical-section time — a `cortex_m::interrupt::free` span in the
TIM6 tick (priority 3) that overlaps the crossing masks COMP. The
firmware rig (examples/am32_clone.rs) busy-waits an adjustable delay in
the TIM6 trampoline, split by whether it runs INSIDE a critical section
(--mode in, masks COMP for its whole span) or OUTSIDE it (--mode out,
COMP can still preempt). Everything else about the known-good clone is
unchanged, so this isolates the one variable.

This sweeps the chosen delay from --from to --to (in microseconds) and,
at each level, measures the fall/desync rate over --dwell seconds from
the info line's `dsy=` counter delta and the `killed=` flag.

THE ORACLE: raising the IN-free delay should induce falls (dsy climbing
/ killed) at a throttle where the clone is normally clean, while the
OUT-free delay at the same magnitude should not. Run both modes and
compare the two tables.

Firmware keys (RAW single chars — the UartDuty parser has no doubled-key
EMI guard, so each key is sent once):
  in-free  : '[' down   ']' up
  out-free : ';' down   "'" up
Each bump = 280 CPU cycles = 3.5 us @ 80 MHz; both delays hard-capped at
4000 cyc (50 us) in firmware. The live values are reported on info line
2 as `din=`/`dout=` (in CYCLES), which this script reads back to confirm
each level actually took.

Kill-guard (house rule): '0\\n' + 'w' on every exit path. The firmware
deadman zeroes throttle after ~3 s of silence, so the throttle is
re-sent through settle and dwell. If a level KILLS, the sweep logs it
and STOPS — no auto re-arm (manual r/q between reps, survival protocol).

Usage:
  python scripts/delay_sweep.py --pct 40 --mode in  --from 0 --to 35 --step 3.5 --dwell 6
  python scripts/delay_sweep.py --pct 40 --mode out --from 0 --to 35 --step 3.5 --dwell 6
"""

import argparse
import pathlib
import re
import sys
import time

import serial

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from am32_census import paced_write  # noqa: E402

CPU_HZ = 80_000_000.0
CYC_PER_US = CPU_HZ / 1e6          # 80
BUMP_CYC = 280                     # firmware DELAY_BUMP_CYC (~3.5 us)
CAP_CYC = 4000                     # firmware DELAY_CAP_CYC (~50 us)
ZERO_PRESSES = 20                  # > CAP/BUMP (=15) → guarantees a floor at 0

# in/out mode → (down_key, up_key) on the firmware UartDuty parser.
MODE_KEYS = {"in": (b"[", b"]"), "out": (b";", b"'")}

FIELDS = {
    "killed": rb"killed=(\d+)",
    "dsy": rb"dsy=(\d+)",
    "comm": rb"comm=(\d+)",
    "din": rb"din=(\d+)",
    "dout": rb"dout=(\d+)",
    "ci": rb"ci=(\d+)",
}


def read_info(ser, timeout=0.6):
    """Send 'i', parse both info lines into a dict. Returns whatever
    fields were seen within `timeout` (empty dict if the wire is dead)."""
    ser.reset_input_buffer()
    paced_write(ser, b"i")
    buf = b""
    out = {}
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        buf += ser.read(4096)
        out = {}
        for k, r in FIELDS.items():
            m = re.search(r, buf)
            if m:
                out[k] = int(m.group(1))
        if len(out) == len(FIELDS):
            break
    return out


def set_delay(ser, mode, target_cyc):
    """Drive the chosen delay to `target_cyc` from an unknown state:
    fully drain to 0 (ZERO_PRESSES down-keys), then step up to target.
    Returns the number of up-presses (== target rounded to a bump)."""
    down, up = MODE_KEYS[mode]
    for _ in range(ZERO_PRESSES):
        paced_write(ser, down)
    n = int(round(target_cyc / BUMP_CYC))
    for _ in range(n):
        paced_write(ser, up)
    return n


def frange(lo, hi, step):
    vals, x = [], lo
    while x <= hi + 1e-9:
        vals.append(round(x, 3))
        x += step
    return vals


def keepalive_dwell(ser, pct, dwell):
    """Hold `dwell` seconds re-sending the throttle inside the deadman."""
    end = time.monotonic() + dwell
    last_ka = 0.0
    while time.monotonic() < end:
        if time.monotonic() - last_ka > 0.8:
            paced_write(ser, f"{pct}\n".encode())
            last_ka = time.monotonic()
        time.sleep(0.05)


def stage_up(ser, pct):
    """Gentle staged climb to `pct` — the clone won't cold-arm at high
    throttle (drops to the OldRoutine limit cycle); patient staging
    locks it clean (bench memory)."""
    stages = [s for s in (20, 40, 60, 80, pct) if s <= pct]
    for s in stages:
        for _ in range(3):  # hold each stage inside the deadman
            paced_write(ser, f"{s}\n".encode())
            time.sleep(0.4)


def arm(ser, pct, tries=4):
    """Start the motor at `pct`; confirm the commutation counter climbs.
    Returns True on a confirmed spin, False after `tries` failures."""
    for attempt in range(tries):
        stage_up(ser, pct)
        paced_write(ser, f"{pct}\n".encode())
        time.sleep(1.0)
        paced_write(ser, f"{pct}\n".encode())
        a = read_info(ser)
        time.sleep(1.2)
        paced_write(ser, f"{pct}\n".encode())
        b = read_info(ser)
        c0, c1 = a.get("comm"), b.get("comm")
        if c0 is not None and c1 is not None and c1 > c0 + 50 and not b.get("killed"):
            print(f"armed at {pct}% (attempt {attempt + 1}, comm {c0}->{c1})",
                  flush=True)
            return True
        print(f"arm attempt {attempt + 1} failed "
              f"(comm {c0}->{c1} killed={b.get('killed')}) - retry", flush=True)
        paced_write(ser, b"0\n")
        time.sleep(1.5)
    return False


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--baud", type=int, default=2_000_000)
    ap.add_argument("--pct", type=int, required=True,
                    help="throttle percent to hold (pick one normally clean)")
    ap.add_argument("--mode", choices=("in", "out"), required=True,
                    help="in = inside interrupt::free (masks COMP); "
                         "out = outside (COMP can preempt)")
    ap.add_argument("--from", dest="lo", type=float, default=0.0,
                    help="first delay level, microseconds")
    ap.add_argument("--to", dest="hi", type=float, default=35.0,
                    help="last delay level, microseconds")
    ap.add_argument("--step", type=float, default=3.5,
                    help="delay step, microseconds (default = one bump)")
    ap.add_argument("--dwell", type=float, default=6.0,
                    help="seconds to hold each level")
    ap.add_argument("--no-arm", action="store_true",
                    help="skip the arm/confirm (motor already spinning)")
    a = ap.parse_args()

    levels = frange(a.lo, a.hi, a.step)
    other = "dout" if a.mode == "in" else "din"
    this = "din" if a.mode == "in" else "dout"

    ser = serial.Serial(a.port, a.baud, timeout=0.05)
    # Trace stream floods the TX ring at high throttle and starves the
    # info line ('i' response dropped -> comm reads None). Toggle 'Z'
    # off until an info poll parses cleanly (bench trace-flood note).
    for _ in range(3):
        if read_info(ser).get("comm") is not None:
            break
        paced_write(ser, b"Z")
        time.sleep(0.3)
    first_fall = None      # first level (us) that induced dsy delta or a kill
    clean_through = None   # highest level with zero dsy delta and no kill
    killed = False
    try:
        # Start clean: zero the injected delay before arming.
        set_delay(ser, a.mode, 0)
        if not a.no_arm:
            if not arm(ser, a.pct):
                raise SystemExit("motor would not start - aborting (no data)")

        print(f"\nmode={a.mode}  pct={a.pct}  dwell={a.dwell}s  "
              f"levels(us)={levels}")
        print(f"{'delay_us':>9} {'delay_cyc':>9} {this + '(cyc)':>11} "
              f"{'dsy_delta':>9} {'killed':>7} {'ci':>6}")
        for us in levels:
            target_cyc = int(round(us * CYC_PER_US))
            set_delay(ser, a.mode, target_cyc)
            # deadman re-send + settle at this level.
            paced_write(ser, f"{a.pct}\n".encode())
            time.sleep(0.8)
            paced_write(ser, f"{a.pct}\n".encode())

            base = read_info(ser)
            dsy0 = base.get("dsy", 0)
            readback = base.get(this)
            # confirm the OTHER channel stayed at 0 (isolation sanity).
            if base.get(other, 0) != 0:
                print(f"  WARN: {other}={base.get(other)} nonzero - not isolated")

            keepalive_dwell(ser, a.pct, a.dwell)

            end = read_info(ser)
            dsy1 = end.get("dsy", dsy0)
            killed = bool(end.get("killed", 0))
            ci = end.get("ci", 0)
            dsy_delta = dsy1 - dsy0
            print(f"{us:>9.1f} {target_cyc:>9d} "
                  f"{('?' if readback is None else readback):>11} "
                  f"{dsy_delta:>9d} {('YES' if killed else 'no'):>7} {ci:>6d}",
                  flush=True)

            fell = dsy_delta > 0 or killed
            if fell and first_fall is None:
                first_fall = us
            if not fell and first_fall is None:
                clean_through = us
            if killed:
                print("  !! KILLED - stopping sweep (manual r/q re-arm "
                      "before the next rep, per survival protocol)", flush=True)
                break

        # ---- verdict hint ----
        print("\n== verdict hint ==")
        label = "in-free (masks COMP)" if a.mode == "in" else "out-free (COMP preempts)"
        if first_fall is not None:
            print(f"{label}: induced falls (dsy delta>0 or kill) starting at "
                  f">= {first_fall:.1f} us"
                  + ("  [ended on a KILL]" if killed else ""))
        else:
            top = clean_through if clean_through is not None else a.hi
            print(f"{label}: NO falls through {top:.1f} us")
        print("Run the other --mode at the same --pct/--from/--to to complete "
              "the A/B: the hypothesis predicts in-free falls where out-free "
              "stays clean.")
    finally:
        # Kill-guard: safe stop on every exit path (does NOT clear a latch).
        try:
            set_delay(ser, a.mode, 0)   # leave the rig inert for the next run
            paced_write(ser, b"0\n")
            time.sleep(0.2)
            paced_write(ser, b"w")
        except Exception:
            pass
        ser.close()


if __name__ == "__main__":
    main()
