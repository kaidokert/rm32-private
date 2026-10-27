#!/usr/bin/env python3
"""Flight-grade envelope re-qual: 2% ladder to 100% under BF DSHOT300.

The parity standard (ladder_quiet.py) re-run through the PRODUCTION
input path: BF CLI motor overrides step 10->100->10 in 2% rungs with
1.5 s dwells, ESC log captured passively (poll law: zero ESC-side
queries during the ladder). The throttle never returns to zero, so the
whole ladder is ONE `[run]` session; its summary (printed after the
final stop) is the verdict: one start, dsy=0, locked commutations, plus
zero resets during the ladder and no bench-guard kill.

Either board on the FC bench (fc_bench.py): --board l431 (BF motor 0)
or binz (motor 1); the relay is switched to that board first.

Parity standard (bench-UART, 07-31): dsy=0, ~1.7M comms over this
ladder (L431, 24 kHz).

Kill guards: motor 1000 + CLI exit on every path; firmware bench guard
(OC/vbat, latched) armed underneath.

Usage: bf_ladder.py [--board l431|binz] [--dwell 1.5] [--top 100]
       [--esclog FILE]
"""
import argparse
import sys
import time

from fc_bench import INDEX, Bf, EscLog, port, route_motor, wait_ready, wait_run

MIN_COMMS = 500_000


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--board", choices=sorted(INDEX), default="l431")
    ap.add_argument("--bf", default=None, help="default: bench_ports 'bf'")
    ap.add_argument("--esc", default=None, help="default: the board's port")
    ap.add_argument("--dwell", type=float, default=1.5)
    ap.add_argument("--top", type=int, default=100)
    ap.add_argument("--esclog", default="bf_ladder_esc.log")
    a = ap.parse_args()

    route_motor(a.board)
    esc = EscLog(a.esc or port(a.board), logfile=a.esclog)
    bf = Bf(a.bf or port("bf"), INDEX[a.board])
    resets_during = 0
    run = None
    try:
        if bf.ensure_unidir_dshot300():
            print("[bf] unidir DSHOT300 set, FC rebooting", flush=True)
        else:
            print("[bf] already unidir DSHOT300", flush=True)
        if not wait_ready(esc):
            print("ready: FAIL (ESC keeps resetting)", flush=True)
            return 1
        print("ready: OK", flush=True)

        rungs = list(range(10, a.top + 1, 2)) + list(range(a.top - 2, 9, -2))
        print(f"== {a.board} ladder 10->{a.top}->10, {len(rungs)} rungs, "
              f"dwell {a.dwell}s ==", flush=True)
        esc.flush()
        bf.enter()
        t0 = time.time()
        r0 = esc.resets
        for pct in rungs:
            bf.motor(1000 + pct * 10)
            esc.watch(a.dwell)  # passive: count resets, never query
        bf.motor(1000)
        resets_during = esc.resets - r0
        print(f"ladder done in {time.time() - t0:.0f}s resets={resets_during}",
              flush=True)
        run = wait_run(esc)
    finally:
        bf.stop_and_exit()
        esc.close()

    print(f"\n=== FLIGHT-GRADE RE-QUAL ({a.board}, BF DSHOT300) ===")
    print(f"  run: {run}")
    print(f"  resets_during={resets_during}")
    print("  (bench-UART parity standard: dsy=0, ~1.7M comms)")
    if esc.killed:
        print(f"  !! bench guard fired: {esc.killed}")
    verdict = (
        run is not None
        and run["starts"] == 1
        and run["dsy"] == 0
        and run["comms"] > MIN_COMMS
        and resets_during == 0
        and esc.killed is None
    )
    print(f"  verdict: {'PASS' if verdict else 'CHECK'}")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
