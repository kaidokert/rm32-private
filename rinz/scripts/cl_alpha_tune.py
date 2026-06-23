#!/usr/bin/env python3
"""Alpha auto-tuner for the Path-B closed loop (examples/scope_cl2.rs).

Spins the motor to a catch-boundary operating point at alpha=0 (open-loop), then steps
the closed-loop authority alpha through a sequence (firmware keys 0/1/2/3 = 0.0/0.2/0.5
/1.0), capturing a few dumps at each level and reporting how the loop behaves:

  fire%      -- fraction of commutations where the detector found a ZC (coast=0); the
                detector-health number. Should rise vs open-loop if the loop is helping.
  period_est -- the loop's filtered sector period (ticks) vs the commanded ol_period.
                G3/anti-skunk: it must stay ~= the rotor's true period, NOT drift to a
                detector-constant value.
  iu_ma      -- current proxy; the abort trigger.

Safety: arms the firmware watchdog, aborts to alpha=0 + kill on over-current, and always
parks at alpha=0 + 'w' in a finally. Reuses scope_sweep's serial plumbing. ATTENDED use.

    python scripts/cl_alpha_tune.py COM41 --hz 250 --amp 13
    python scripts/cl_alpha_tune.py COM41 --hz 250 --amp 13 --alphas 0 1 2 3 --snaps 4
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
from scope_sweep import (
    Setpoint,
    capture,
    dump_iu_ma,
    position,
    send,
    set_watchdog,
)

# firmware alpha keys -> value (examples/scope_cl2.rs)
ALPHA_VALUE = {"0": 0.0, "1": 0.2, "2": 0.5, "3": 1.0}

_CL_RE = re.compile(r"cl i=\d+ phys=\d+ zc=(-?\d+) coast=(\d)")


def ramp_alpha(ser, cur_x1000: int, tgt_x1000: int, dwell: float = 0.05) -> int:
    """Gradually walk alpha from cur to tgt via fine +/-0.05 firmware steps (m/n), so
    engaging/disengaging the loop never jumps the commutation. Returns the new alpha."""
    tgt = max(0, min(1000, round(tgt_x1000 / 50) * 50))
    while cur_x1000 != tgt:
        step = 50 if tgt > cur_x1000 else -50
        send(ser, "m" if step > 0 else "n")
        cur_x1000 += step
        time.sleep(dwell)
    return cur_x1000


def parse_loop(dump: str):
    """(alpha_x1000, period_est_x100, iu_ma, n_comm, n_coast0, n_zc, isr_cyc) from a dump."""
    def grab(key):
        m = re.search(rf"\b{key}=(\d+)", dump)
        return int(m.group(1)) if m else None

    cl = _CL_RE.findall(dump)
    n = len(cl)
    coast0 = sum(1 for _zc, co in cl if co == "0")
    zc = sum(1 for z, _co in cl if z != "-1")
    return grab("alpha"), grab("period_est"), dump_iu_ma(dump), n, coast0, zc, grab("isr_cyc")


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("port")
    p.add_argument("--baud", type=int, default=BAUD)
    p.add_argument("--hz", type=int, default=250)
    p.add_argument("--amp", type=float, default=13.0, help="catch-boundary amp %% (open-loop spin-up)")
    p.add_argument("--alpha-step", type=float, default=0.1, help="alpha increment per level (mult of 0.05)")
    p.add_argument("--alpha-max", type=float, default=1.0, help="highest alpha to ramp to")
    p.add_argument("--snaps", type=int, default=4)
    p.add_argument("--settle", type=float, default=0.5, help="dwell after an alpha change")
    p.add_argument("--current-limit", type=int, default=3000)
    p.add_argument("--capture-timeout", type=float, default=4.0)
    p.add_argument("--outdir", type=Path, default=None)
    args = p.parse_args()

    ol_period = 20000.0 / (args.hz * 6)  # commanded ticks/sector (ADC frame rate / (hz*6))
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    outdir = args.outdir or Path("logs") / f"alpha_{stamp}"
    outdir.mkdir(parents=True, exist_ok=True)
    targets = [round(i * args.alpha_step, 3) for i in range(int(args.alpha_max / args.alpha_step) + 1)]
    print(f"alpha tune: {args.hz} Hz / {args.amp}% (ol_period={ol_period:.2f} ticks), "
          f"alphas={targets} -> {outdir}")

    with serial.Serial(args.port, args.baud, timeout=0.05) as ser:
        ser.reset_input_buffer()
        sp = Setpoint()
        if set_watchdog(ser, True):
            print("watchdog ARMED (firmware self-kills after ~10s of no serial)")
        else:
            print("WARNING: watchdog did NOT arm")
        try:
            # spin up + lock at the operating point, alpha=0 (open-loop)
            position(ser, sp, args.hz, int(round(args.amp * 10)),
                     qsettle=0.8, ramp_step=20, ramp_dwell=0.3, transit_margin=8.0, settle=0.3)
            send(ser, "0")  # ensure alpha=0 after positioning
            time.sleep(args.settle)

            print(f"\n  {'alpha':>5} {'fire%':>6} {'period_est':>10} {'(ol)':>6} {'zc%':>5} "
                  f"{'iu_ma':>6} {'isr_cyc':>7} {'%bud':>5}")
            cur = 0  # alpha x1000; firmware is at 0 after positioning's q-reset
            send(ser, "0")
            for tgt in targets:
                cur = ramp_alpha(ser, cur, int(round(tgt * 1000)))  # gradual, no jump
                time.sleep(args.settle)
                fires, periods, zcs, ius, cycs = [], [], [], [], []
                aborted = False
                for snap in range(args.snaps):
                    dump = capture(ser, args.capture_timeout, "c")
                    tag = f"{int(round(tgt * 100)):03d}"
                    (outdir / f"a{tag}_s{snap}.log").write_text(dump, encoding="ascii", errors="replace")
                    _a, per, iu, n, c0, zc, cyc = parse_loop(dump)
                    if n:
                        fires.append(c0 / n * 100)
                        zcs.append(zc / n * 100)
                    if per:
                        periods.append(per / 100.0)
                    if cyc is not None:
                        cycs.append(cyc)
                    if iu is not None:
                        ius.append(iu)
                        if iu > args.current_limit:
                            send(ser, "0")  # immediate alpha -> open-loop
                            send(ser, "w")  # and kill
                            print(f"  CURRENT ABORT: iu_ma={iu} > {args.current_limit} at alpha {tgt}")
                            aborted = True
                            break
                med = lambda xs: (sorted(xs)[len(xs) // 2] if xs else float("nan"))
                cyc_max = max(cycs) if cycs else float("nan")  # worst-case is what matters
                print(f"  {tgt:>5.2f} {med(fires):5.0f}% {med(periods):10.2f} "
                      f"{ol_period:6.2f} {med(zcs):4.0f}% {med(ius):6.0f} "
                      f"{cyc_max:7.0f} {cyc_max / 8500 * 100:4.0f}%")
                if aborted:
                    cur = 0
                    break
            cur = ramp_alpha(ser, cur, 0)  # gentle ramp-down avoids the teardown stall
            send(ser, "w")
        except KeyboardInterrupt:
            print("\naborted; parking")
            try:
                send(ser, "0")
                send(ser, "w")
            except Exception:
                pass
        finally:
            try:
                set_watchdog(ser, False)
            except Exception:
                pass
    print(f"\nraw dumps in {outdir} -- run the oracle check with:\n"
          f"  python scripts/scope_cl_validate.py {outdir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
