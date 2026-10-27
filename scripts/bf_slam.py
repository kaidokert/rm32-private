#!/usr/bin/env python3
"""Downthrottle-blip / double-slam test — the clone re-qual's signature
move, through BF DSHOT300 (unidir).

Engage at 10% for 5 s, then per slam cycle: 100% hold, slam to 10%,
hold, slam back to 100%, hold, slam to 10%. The floor is 10% (never a
dead stop mid-test), so the whole test is ONE `[run]` session. The
parity question: do downthrottle transitions produce desyncs/resets (the
historical 'downthrottle blip') or ride clean like the clone's re-qual
artifact (dsy~0 through double slams at 3145 Hz)?

Verdict from the ESC's `[run]` summary (one start, dsy=0, locked comms
— a churned run never locks, so comms keeps dsy=0 from being vacuous)
plus zero resets and no bench-guard kill. Either board (fc_bench.py).

Usage: bf_slam.py [--board l431|binz] [--cycles 3] [--esclog FILE]
"""
import argparse
import sys

from fc_bench import INDEX, Bf, EscLog, port, route_motor, wait_ready, wait_run

MIN_COMMS = 100_000


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--board", choices=sorted(INDEX), default="l431")
    ap.add_argument("--bf", default=None, help="default: bench_ports 'bf'")
    ap.add_argument("--esc", default=None, help="default: the board's port")
    ap.add_argument("--cycles", type=int, default=3)
    ap.add_argument("--esclog", default="bf_slam_esc.log")
    a = ap.parse_args()

    route_motor(a.board)
    esc = EscLog(a.esc or port(a.board), logfile=a.esclog)
    bf = Bf(a.bf or port("bf"), INDEX[a.board])
    resets = 0
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

        esc.flush()
        bf.enter()
        r0 = esc.resets
        # Engage gently first (clone protocol: slams happen FROM a
        # running state).
        bf.motor(1100)
        esc.watch(5.0)
        for n in range(a.cycles):
            for value, hold in ((2000, 2.5), (1100, 2.0), (2000, 2.0), (1100, 1.5)):
                bf.motor(value)
                esc.watch(hold)
            print(f"cycle {n + 1} done (resets so far: {esc.resets - r0})",
                  flush=True)
        bf.motor(1000)
        resets = esc.resets - r0
        run = wait_run(esc)
    finally:
        bf.stop_and_exit()
        esc.close()

    print(f"\n=== DOUBLE-SLAM ({a.board}) ===")
    print(f"  cycles={a.cycles} run: {run}")
    print(f"  resets={resets}")
    print("  (clone re-qual artifact rode double slams at dsy~0)")
    if esc.killed:
        print(f"  !! bench guard fired: {esc.killed}")
    verdict = (
        run is not None
        and run["starts"] == 1
        and run["dsy"] == 0
        and run["comms"] > MIN_COMMS
        and resets == 0
        and esc.killed is None
    )
    print(f"  verdict: {'PASS' if verdict else 'CHECK'}")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
