#!/usr/bin/env python3
"""Engage reliability test — N fresh engages, bench_lib discipline.

Rewritten on bench_lib after the 07-28 finding: the original version
waited only 2.4 s after reset and re-engaged a still-COASTING rotor,
manufacturing the phantom "engage lottery" (0-2/5) that ate a session.
With the enforced full-stop settle + gentle ramp, the same motor
engages 3/3 at clone parity.

Each rep: reset (boot cause captured) -> full-stop settle -> gentle
ramp to target -> verified-locked check (old=0 AND plausible current).
"""
import argparse
import sys

from bench_lib import Bench, reset_board


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--target", type=int, default=70)
    a = ap.parse_args()

    print(f"== engage reliability: {a.reps} fresh engages -> {a.target}% ==")
    results = []
    for rep in range(1, a.reps + 1):
        identity, causes = reset_board(a.port)
        if identity != "rm32":
            print(f"  rep {rep}: firmware identity={identity!r} "
                  f"(causes={causes}) — wrong/absent firmware, aborting")
            return 1
        with Bench(a.port) as b:
            ok = b.engage_from_stop(a.target)
            inf = b.sample(a.target) if ok else None
            results.append((ok, inf.ci if inf else None))
            print(f"  rep {rep}: {'LOCKED' if ok else 'FAIL':7s} "
                  f"{b.last_engage_report}"
                  + (f"  reboots={b.reboot_events}" if b.reboot_events else ""))
    n_ok = sum(1 for ok, _ in results if ok)
    cis = [ci for ok, ci in results if ok and ci]
    print(f"\nlocked: {n_ok}/{a.reps}"
          + (f"   ci@{a.target}%: {min(cis)}-{max(cis)}" if cis else ""))
    return 0 if n_ok == a.reps else 2


if __name__ == "__main__":
    sys.exit(main())
