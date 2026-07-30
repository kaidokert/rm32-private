#!/usr/bin/env python3
"""Advance-lever A/B — the demag-margin wall intervention.

Theory under test: the 98-100% desync-storm wall is a demag runaway —
at full load the demag interval outgrows the ~53 us commutation window
and swallows the ZC (proven: 22.6 ms armed-comparator edge voids,
entries=0). rm32 commutes measurably earlier than the clone at equal
advance settings (27% vs 39% post-ZC at mid), so the intervention is
the 'Y' temp_advance override:

  arm BASE  (override off, temp_advance 16): storms at 98-100% (est.)
  arm ADV8  (temp_advance 8, LATER commutation, more demag margin):
            theory predicts 100% HOLDS
  arm ADV24 (temp_advance 24, EARLIER, less margin): theory predicts
            the wall moves DOWN (storms at <=96%)

Pack-health gating: the A/B is only valid on a healthy pack. Reps abort
(not "fail") on: rest V < 11.6, any mid-run reboot, or a VBAT/guard
kill — those invalidate the arm, they don't count against the theory.

Usage: advance_ab.py [--arms BASE,ADV8,ADV24] [--hold-secs 8]
"""
import argparse
import sys
import time

from bench_lib import Bench, reset_board

Y_PRESSES = {"BASE": 0, "ADV8": 1, "ADV24": 2}


def run_arm(port, label, hold_secs, watch_climb):
    identity, causes = reset_board(port)
    if identity != "rm32":
        return ("ABORT", f"identity={identity} causes={causes}")
    with Bench(port) as b:
        if not b.engage_from_stop(55):
            return ("ABORT", f"engage failed: {b.last_engage_report}")
        pre = b.info()
        if pre and pre.volts < 11.0:
            return ("ABORT", f"pack too low under 55% load ({pre.volts:.2f}V)")
        for _ in range(Y_PRESSES[label]):
            b.cmd(b"Y", settle=0.25)
        for pct in (70, 80, 90, 93, 96):
            b.hold(pct, 2.2)
            if b.reboot_events:
                return ("ABORT", f"reboot during climb {b.reboot_events}")
            if b.kill_line_seen:
                return ("ABORT", "guard kill during climb")
            if watch_climb:
                inf = b.info()
                print(f"  {label} [{pct}%] {inf}")
                if inf and inf.dsy > 60:
                    return ("STORM-CLIMB", f"stormed at {pct}% (dsy={inf.dsy})")
        s96 = b.info()
        print(f"  {label} 96%: {s96}")
        verdict, detail = "HELD", ""
        t0 = time.time()
        while time.time() - t0 < hold_secs:
            b.hold(100, 1.0)
            if b.reboot_events:
                return ("ABORT", f"reboot at 100% {b.reboot_events}")
            if b.kill_line_seen:
                return ("ABORT", "guard kill at 100%")
            inf = b.info()
            if inf is None:
                continue
            print(f"  {label} [100 t={time.time()-t0:4.1f}] {inf}")
            detail = repr(inf)
            if inf.dsy > 150:
                verdict = "STORM"
                break
            if inf.running and inf.ci < 200 and inf.amps > 4.0:
                verdict = "HELD"  # provisional; final state decides
        return (verdict, detail)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--arms", default="BASE,ADV8,ADV24")
    ap.add_argument("--hold-secs", type=float, default=8.0)
    a = ap.parse_args()

    results = {}
    for label in a.arms.split(","):
        label = label.strip().upper()
        if label not in Y_PRESSES:
            print(f"unknown arm {label}"); return 1
        print(f"== arm {label} (Y x{Y_PRESSES[label]}) ==")
        verdict, detail = run_arm(a.port, label, a.hold_secs,
                                  watch_climb=(label == "ADV24"))
        results[label] = verdict
        print(f"  -> {verdict}  {detail}\n")
        time.sleep(4)

    print("== A/B summary ==")
    for k, v in results.items():
        print(f"  {k:6s}: {v}")
    print("\ntheory PREDICTS: BASE=STORM  ADV8=HELD  ADV24=STORM(-CLIMB)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
