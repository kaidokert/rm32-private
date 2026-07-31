#!/usr/bin/env python3
"""Mux-switching ABAB: scan ch8/11/17 (base) vs ch8x3 (zero switching).

The decisive injector experiment: conversions continue in both arms;
only CHANNEL SWITCHING differs. Verbatim detector (no 'T') so the
jitter manifests as storm onset. 888 arms send 'U' (guard defeat —
their voltage/temp raws are ch8 garbage); base arms keep the guard.
Flashes the arm's ELF before each rep. Fresh-pack protocol.

Usage: mux_ab.py [port]
"""
import subprocess
import sys
import time

from bench_lib import Bench, reset_board

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
ELVES = {"BASE": "../captures/elf_base", "888": "../captures/elf_888"}


def flash(elf):
    r = subprocess.run(["probe-rs", "download", "--chip", CHIP, "--probe",
                        PROBE, "--binary-format", "elf", elf],
                       capture_output=True, text=True)
    return "Finished" in (r.stdout + r.stderr)


def rep(port, arm, cap=10.0):
    if not flash(ELVES[arm]):
        return ("ABORT", "flash failed")
    identity, _ = reset_board(port)
    if identity != "rm32":
        return ("ABORT", f"identity={identity}")
    with Bench(port) as b:
        if arm == "888":
            b.cmd(b"U", settle=0.25)   # guard defeat (garbage raws)
        if not b.engage_from_stop(55):
            return ("ABORT", "engage: " + b.last_engage_report)
        for p in (70, 80, 90, 93, 96):
            b.hold(p, 2.2)
        s96 = b.info()
        print(f"  96%: {s96}")
        if s96 is None or not s96.running:
            return ("ABORT", "no lock at 96")
        d0 = s96.dsy
        t0 = time.time()
        last = None
        while time.time() - t0 < cap:
            b.hold(100, 0.8)
            if arm == "BASE" and (b.kill_line_seen or b.reboot_events):
                return ("ABORT", "kill/reboot")
            inf = b.info()
            if inf is None:
                continue
            last = inf
            if inf.dsy - d0 > 120:
                print(f"  [storm] {inf}")
                return ("STORM", f"{time.time()-t0:.1f}")
        print(f"  [end] {last}")
        held = last is not None and last.running and last.duty >= 1990
        return ("HELD" if held else "FELL", f"{cap:.1f}")


def main():
    port = sys.argv[1] if len(sys.argv) > 1 else "COM41"
    seq = ["888", "BASE", "BASE", "888"]
    res = []
    for arm in seq:
        print(f"== arm {arm} ==")
        v, t = rep(port, arm)
        res.append((arm, v, t))
        print(f"  -> {v} t={t}\n")
        time.sleep(4)
    print("== mux-switching ABAB (verbatim detector; onset s) ==")
    for arm, v, t in res:
        print(f"  {arm:5s}: {v:6s} t={t}")
    print("\n888=HELD & BASE=STORM => channel switching IS the injector")
    print("both STORM => switching exonerated; injector is conversions/")
    print("start-timing or non-ADC")
    return 0


if __name__ == "__main__":
    sys.exit(main())
