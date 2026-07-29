#!/usr/bin/env python3
"""Reboot watchdog for bench runs — catches the thing ad-hoc scripts miss.

Every prior script read the serial stream but only parsed `i ` lines, so
mid-run REBOOTS (and their reset cause) were captured into the buffer and
thrown away. This one does the opposite: a reader thread continuously
timestamps the stream, and the main loop scans for `[rm32] boot` markers
and `last reset:` causes WHILE spinning, reporting every reboot with the
throttle it happened at. If the board is browning-out / watchdog-resetting
/ faulting mid-spin-up, that explains "never locks" (each reboot re-arms
the motor into a fresh startup crawl) far better than any control theory.
"""
import argparse
import re
import subprocess
import sys
import threading
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--target", type=int, default=70)
    ap.add_argument("--cooldown", type=int, default=150,
                    help="seconds idle (motor OFF) before the run -- thermal rest")
    a = ap.parse_args()
    stages = [s for s in (20, 25, 30, 40, 55) if s < a.target] + [a.target]

    if a.cooldown > 0:
        print(f"== thermal cooldown: {a.cooldown}s idle (motor off) ==")
        time.sleep(a.cooldown)

    p = serial.Serial(a.port, 2_000_000, timeout=0.02)
    buf = bytearray()
    stop = [False]

    def reader():
        while not stop[0]:
            c = p.read(8192)
            if c:
                buf.extend(c)

    th = threading.Thread(target=reader)
    th.start()
    time.sleep(0.2)
    subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
                   capture_output=True)
    time.sleep(2.5)

    boots = [0]          # count of "[rm32] boot" seen so far
    events = []          # (throttle, cause)
    cur_pct = [0]

    def scan():
        txt = "".join(chr(b) if 32 <= b < 127 or b == 10 else "." for b in buf)
        n = txt.count("[rm32] boot")
        if n > boots[0]:
            # new reboot(s) since last scan -- grab the most recent cause line
            causes = re.findall(r"last reset:\s*([a-zA-Z0-9\- ]+)", txt)
            cause = causes[-1].strip() if causes else "?"
            for _ in range(n - boots[0]):
                events.append((cur_pct[0], cause))
                print(f"  !! REBOOT at throttle={cur_pct[0]}%  cause='{cause}'")
            boots[0] = n

    def hold(pct, secs):
        cur_pct[0] = pct
        t0 = time.time()
        while time.time() - t0 < secs:
            p.write(f"{pct}\n".encode())
            p.flush()
            time.sleep(0.08)
            scan()

    locks = []

    def info(tag):
        n = len(buf)
        p.write(b"i")
        p.flush()
        time.sleep(0.3)
        scan()
        for L in "".join(chr(b) if 32 <= b < 127 else "." for b in buf[n:]).split("\n"):
            if L.startswith("i "):
                f = dict(x.split("=") for x in L.split() if "=" in x)
                try:
                    old = int(f.get("old", 9)); ci = int(f.get("ci", 0))
                    duty = int(f.get("duty", 0)); zc = int(f.get("zc", 0))
                    ia = int(f.get("iraw", 0)) * 2686 / 100 / 1000
                    v = int(f.get("vbat", 0)) * 752 / 100 / 1000
                except ValueError:
                    return
                run = old == 0
                tight = run and ci < 350
                locks.append((tag, run, tight, ci))
                print(f"  [{tag:>4}] {'TIGHT' if tight else 'RUN' if run else 'safe':5s} "
                      f"ci={ci:>5} duty={duty:>4} zc={zc:>5} I={ia:.1f}A V={v:.2f}V")
                return

    print(f"== reboot-watch + lock baseline: spin-up -> {a.target}% (stages {stages}) ==")
    # count the boot from the initial reset as the baseline, don't flag it
    scan()
    boots[0] = max(boots[0], 1)
    try:
        hold(0, 1.0)
        for s in stages:
            hold(s, 3.0)
            info(s)
        hold(a.target, 3.0)
        info(a.target)
    finally:
        for x in (40, 25, 15):
            hold(x, 0.3)
        for _ in range(4):
            p.write(b"w")
            p.flush()
            time.sleep(0.08)
        time.sleep(0.3)
        stop[0] = True
        th.join()
        p.close()

    highest_tight = max((tag for tag, r, t, ci in locks if t), default=None)
    highest_run = max((tag for tag, r, t, ci in locks if r), default=None)
    print(f"\n== LOCK BASELINE ==")
    print(f"  highest TIGHT lock (ci<350): {highest_tight}%   "
          f"highest RUNNING: {highest_run}%")
    print(f"\n== summary: {len(events)} reboot(s) during the spin-up ==")
    from collections import Counter
    for cause, n in Counter(c for _, c in events).most_common():
        pcts = [pct for pct, c in events if c == cause]
        print(f"  cause='{cause}': {n}x  at throttles {sorted(set(pcts))}")
    if not events:
        print("  no reboots detected -- the motor issue is NOT a reboot loop.")
    # final: dump the last boot banner block for the reset-cause detail
    txt = "".join(chr(b) if 32 <= b < 127 or b == 10 else "." for b in buf)
    tail = [l.strip() for l in txt.split("\n")
            if "last reset" in l or "boot" in l or "panic" in l.lower()
            or "HardFault" in l or "guard" in l]
    print("\n-- reset/boot/fault lines seen --")
    for l in tail[-12:]:
        print("   ", l[:100])
    return 0


if __name__ == "__main__":
    sys.exit(main())
