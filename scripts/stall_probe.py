#!/usr/bin/env python3
"""Dual-harness stuck-rotor latch probe (rust rm32 vs C AM32 reference).

Drives the same injected-stall sequences through both blackbox
harnesses and prints the latch variables side by side. Focus: after
the latch fires (bemf_timeout_happened > threshold -> input forced 0),
do the clear conditions (zc>1000 | stick release; zc>100 && raw<200)
REOPEN the latch while throttle is still applied — the oscillation
observed on the bench at 5% throttle (raw=146 < 200).

Harnesses run SEQUENTIALLY (two concurrent piped children deadlock on
Windows handle inheritance); a crashed harness reports DEAD rows.

Usage: stall_probe.py [--rust EXE] [--c EXE]
"""
import argparse
import re
import subprocess
import sys
import threading

FIELDS = ["bemf_timeout_happened", "adjusted_input", "input", "running",
          "zero_crosses", "alloff_count", "newinput"]


class H:
    def __init__(self, exe, name):
        self.name = name
        self.p = subprocess.Popen([exe], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True)
        banner = self.p.stdout.readline().strip()
        assert banner == "ready", f"{name}: unexpected banner {banner!r}"

    def cmd(self, line):
        self.p.stdin.write(line + "\n")
        self.p.stdin.flush()
        # Watchdog: a hung child (observed: AM32 harness infinite-loops
        # in T2) blocks readline forever on Windows — kill it after 10 s
        # so readline returns EOF and the caller marks the row DEAD.
        w = threading.Timer(10.0, self.p.kill)
        w.start()
        try:
            out = self.p.stdout.readline().strip()
        finally:
            w.cancel()
        if not out:
            raise OSError("harness dead or hung (killed by watchdog)")
        return out

    def state(self, line=None):
        out = self.cmd(line or "state")
        d = {}
        for m in re.finditer(r"(\w+)=(-?\d+)", out):
            d[m.group(1)] = int(m.group(2))
        return d

    def close(self):
        try:
            self.p.stdin.write("quit\n")
            self.p.stdin.flush()
        except OSError:
            pass
        self.p.terminate()


def run_one(exe, name, steps):
    h = H(exe, name)
    out = []
    try:
        h.cmd("config armed=1 inputSet=1 dshot=1")
        h.cmd("config eeprom.stuck_rotor_protection=1")
        h.cmd("load_eeprom")
        for _tag, line in steps:
            try:
                if line.startswith(("tick", "ticks")):
                    out.append(h.state(line))
                else:
                    h.cmd(line)
                    out.append(h.state())
            except OSError:
                rc = h.p.poll()
                out.append({"DEAD": rc if rc is not None else -1})
                break
    finally:
        h.close()
    while len(out) < len(steps):
        out.append({"DEAD": -999})
    return out


def show(tag, r, c):
    def fmt(d):
        if "DEAD" in d:
            return f"HARNESS DEAD (rc={d['DEAD']})"
        return " ".join(f"{f}={d.get(f, '?')}" for f in FIELDS)
    print(f"  {tag}")
    print(f"    rust: {fmt(r)}")
    print(f"    c   : {fmt(c)}")


def run_scenario(rust_exe, c_exe, title, steps):
    print(f"\n=== {title} ===", flush=True)
    rs_all = run_one(rust_exe, "rust", steps)
    cs_all = run_one(c_exe, "c", steps)
    for (tag, line), rs, cs in zip(steps, rs_all, cs_all):
        show(f"{tag:34} [{line}]", rs, cs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rust", default="../target/release/rm32_harness.exe")
    ap.add_argument("--c",
                    default="E:/m/robot/esc/am32_reqcheck/build/Debug/am32_harness.exe")
    a = ap.parse_args()

    run_scenario(a.rust, a.c, "T1: latch holds at 5% throttle, zc=5", [
        ("baseline",           "ticks 3 throttle=146"),
        ("inject bemf=101",    "config bemf_timeout_happened=101"),
        ("tick -> latch",      "ticks 1"),
        ("tick x50",           "ticks 50"),
        ("tick x500",          "ticks 500"),
    ])

    run_scenario(a.rust, a.c, "T2: latch vs zc=150 at 5% throttle", [
        ("baseline",           "ticks 3 throttle=146"),
        ("zc=150 + bemf=101",  "config zero_crosses=150 bemf_timeout_happened=101"),
        ("tick -> latch?",     "ticks 1"),
        ("tick x50",           "ticks 50"),
        ("tick x500",          "ticks 500"),
    ])

    run_scenario(a.rust, a.c, "T3: latch vs zc=150 at 25% throttle", [
        ("baseline",           "ticks 3 throttle=500"),
        ("zc=150 + bemf=101",  "config zero_crosses=150 bemf_timeout_happened=101"),
        ("tick -> latch?",     "ticks 1"),
        ("tick x500",          "ticks 500"),
    ])
    return 0


if __name__ == "__main__":
    sys.exit(main())
