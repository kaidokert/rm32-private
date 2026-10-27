#!/usr/bin/env python3
"""Throttle -> speed curve through Betaflight, either board: BF's own
bidir eRPM plus EDT temperature / voltage / current per rung, to CSV.

The ESC never prints during a powered run, so every per-rung number is
FC-side: `dshot_telemetry_info` read from the same CLI session that
drives `motor N <value>`. EDT needs BF's arm-time cmd 13 (dshot_edt=ON,
feature ESC_SENSOR off -- refactor_log Call 44), so one short MSP
arm-fly at 10 % primes it first; the CLI session that follows does not
reboot the FC, so the decode state survives.

Rungs 10 -> top -> 10 in --step % (never back to zero), --settle s
before --samples reads per rung. Verdict from the ESC's [run] summary
(one start, dsy=0) plus zero resets and no bench-guard kill. Refuses to
write a curve if EDT is not decoding at the first rung.

Usage: bf_curve.py --board l431|binz [--step 5] [--top 100] [--settle 1.0]
       [--samples 3] --csv FILE [--esclog FILE]
"""
import argparse
import csv
import re
import subprocess
import sys
import time
from pathlib import Path

from fc_bench import INDEX, Bf, EscLog, port, route_motor, wait_ready, wait_run

# idx TYPE eRPM RPM Hz INVALID% TEMP VCC CURR ...
ROW_RE = re.compile(
    r"^\s*(\d+)\s+(\S+)\s+(\d+)\s+\d+\s+\d+\s+([\d.]+)%\s+(\d+)\s+([\d.]+)\s+(\d+)", re.M)


def read_row(bf, motor):
    out = bf.cmd("dshot_telemetry_info", quiet=0.3)
    for m in ROW_RE.finditer(out):
        if int(m.group(1)) == motor + 1:
            return {"types": m.group(2), "erpm": int(m.group(3)),
                    "invalid_pct": float(m.group(4)), "temp_c": int(m.group(5)),
                    "vbat_v": float(m.group(6)), "curr_a": int(m.group(7))}
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--board", choices=sorted(INDEX), required=True)
    ap.add_argument("--step", type=int, default=5)
    ap.add_argument("--top", type=int, default=100)
    ap.add_argument("--settle", type=float, default=1.0)
    ap.add_argument("--samples", type=int, default=3)
    ap.add_argument("--csv", required=True)
    ap.add_argument("--esclog", default="bf_curve_esc.log")
    a = ap.parse_args()
    motor = INDEX[a.board]
    bf_port = port("bf")

    route_motor(a.board)
    esc = EscLog(port(a.board), logfile=a.esclog)
    bf = Bf(bf_port, motor)
    rows, run, resets = [], None, 0
    try:
        if not wait_ready(esc):
            print("ready: FAIL (ESC keeps resetting)", flush=True)
            return 1
        print("ready: OK; priming EDT with one MSP arm", flush=True)
        esc.p.close()  # the arm-fly spins the motor: keep the log passive
        subprocess.run([sys.executable, str(Path(__file__).with_name("bf_msp.py")),
                        "--port", bf_port, "--arm-fly", "10", "--hold", "3"],
                       check=True, timeout=60, capture_output=True)
        esc.p.open()
        esc.watch(2.0)  # the arm-fly's own [run] lands here, not counted

        bf.enter()
        up = list(range(10, a.top + 1, a.step))
        rungs = up + list(range(a.top - a.step, 9, -a.step))
        r0 = esc.resets
        t0 = time.time()
        for i, pct in enumerate(rungs):
            direction = "up" if i < len(up) else "down"
            bf.motor(1000 + pct * 10)
            esc.watch(a.settle)
            for s in range(a.samples):
                r = read_row(bf, motor)
                if r is None:
                    continue
                if i == 0 and s == 0 and not {"T", "V"} <= set(r["types"]):
                    raise RuntimeError(f"EDT not decoding at first rung: {r['types']}")
                rows.append({"board": a.board, "t_s": round(time.time() - t0, 2),
                             "pct": pct, "dir": direction, "sample": s, **r})
            last = rows[-1] if rows else {}
            print(f"{pct:3d}% {direction:4} erpm={last.get('erpm')} "
                  f"I={last.get('curr_a')}A V={last.get('vbat_v')} "
                  f"T={last.get('temp_c')}C inv={last.get('invalid_pct')}%", flush=True)
        bf.motor(1000)
        resets = esc.resets - r0
        run = wait_run(esc)
    finally:
        bf.stop_and_exit()
        esc.close()

    with open(a.csv, "w", newline="", encoding="ascii") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerows(rows)
    print(f"\n=== CURVE ({a.board}) === {len(rows)} samples -> {a.csv}")
    print(f"  run: {run}")
    print(f"  resets={resets}")
    if esc.killed:
        print(f"  !! bench guard fired: {esc.killed}")
    ok = (run is not None and run["starts"] == 1 and run["dsy"] == 0
          and resets == 0 and esc.killed is None)
    print(f"  verdict: {'PASS' if ok else 'CHECK'}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
