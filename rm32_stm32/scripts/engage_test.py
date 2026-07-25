#!/usr/bin/env python3
"""Engage determinism test: N arm->throttle->check cycles (lottery rule:
judge engage only with n>=4). PASS per attempt = interrupt-mode lock
(old=0, run=1) with f_e above the polling plateau. Kill guard on exit."""
import argparse, re, time
import serial

ap = argparse.ArgumentParser()
ap.add_argument("--port", default="COM41")
ap.add_argument("--n", type=int, default=5)
ap.add_argument("--pct", type=int, default=20)
a = ap.parse_args()
RE = re.compile(rb"i step=\d+ old=(\d+) run=(\d+) ci=\d+ avg=(\d+) ")
p = serial.Serial(a.port, 2_000_000, timeout=0.05)
wins = 0
try:
    for att in range(a.n):
        t0 = time.time()
        while time.time() - t0 < 1.6:
            p.write(b"0\n"); p.flush(); time.sleep(0.1)
        t0 = time.time()
        while time.time() - t0 < 6.0:
            p.write(f"{a.pct}\n".encode()); p.flush(); time.sleep(0.3)
        p.read(65536); p.write(b"i"); p.flush(); time.sleep(0.4)
        m = None
        for m in RE.finditer(p.read(65536)): pass
        if m:
            old, run, avg = int(m.group(1)), int(m.group(2)), int(m.group(3))
            fe = 2e6 / (6 * avg) if avg else 0
            ok = run == 1 and old == 0 and fe > 400
            wins += ok
            print(f"attempt {att+1}: {'LOCK' if ok else 'fail'} "
                  f"(run={run} old={old} f_e={fe:.0f} Hz)")
        else:
            print(f"attempt {att+1}: no readback")
        p.write(b"s\n"); p.flush(); time.sleep(1.2)
finally:
    p.write(b"w"); p.flush(); time.sleep(0.2); p.close()
print(f"== {wins}/{a.n} interrupt-mode locks")
