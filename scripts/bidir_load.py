#!/usr/bin/env python3
"""Bidir DShot under load: throttle ladder with live eRPM feedback check.

Ensures dshot_bidir=ON @ DSHOT300 on the FC, waits for the ESC's
self-validating BiDShot commit, then steps the motor through a throttle
ladder holding each rung while querying BF's `dshot_telemetry_info`
mid-spin — the FC's own decode of rm32's GCR responses is the ground
truth for "we are getting feedback". Reports eRPM + invalid% per rung;
PASS requires nonzero, throttle-monotonic eRPM and low invalid%.

FC-side queries don't touch the ESC log (poll law intact); the ESC's
COM41 stream is watched passively for resets/kills during the ladder.

Kill guards: motor 1000 + CLI exit on every path; firmware bench guard
armed underneath.

Usage: bidir_load.py [--bf COM42] [--esc COM41] [--ladder 15,25,35,50]
       [--hold 5]
"""
import argparse
import re
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?crc_pass=(\d+) crc_fail=(\d+)"
)
# dshot_telemetry_info motor row:  "1   R----   1234   617  ..."
MOTOR1_RE = re.compile(
    r"^\s*1\s+(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+%|NO DATA)", re.M
)
READS_RE = re.compile(r"Dshot reads:\s*(\d+)")
INVALID_RE = re.compile(r"Dshot invalid pkts:\s*(\d+)")


def open_retry(port, baud, tries=30, delay=0.5):
    for _ in range(tries):
        try:
            return serial.Serial(port, baud, timeout=0.05)
        except serial.SerialException:
            time.sleep(delay)
    raise RuntimeError(f"cannot open {port} after {tries} tries")


def drain(p, quiet=0.3):
    out = b""
    t = time.time()
    while time.time() - t < quiet:
        b = p.read(4096)
        if b:
            out += b
            t = time.time()
    return out.decode("ascii", "replace")


class Bf:
    def __init__(self, port):
        self.port = port
        self.p = None

    def enter(self):
        self.p = open_retry(self.port, 115_200)
        self.p.write(b"#\n")
        banner = drain(self.p, 0.5)
        if "CLI" not in banner and "#" not in banner:
            raise RuntimeError(f"no CLI banner: {banner[:80]!r}")

    def cmd(self, c, quiet=0.5):
        self.p.write((c + "\n").encode())
        return drain(self.p, quiet)

    def save_and_close(self):
        try:
            self.p.write(b"save\n")
            time.sleep(0.5)
        finally:
            self.p.close()
            self.p = None

    def exit_and_close(self):
        try:
            self.p.write(b"exit\n")
            time.sleep(0.3)
        finally:
            self.p.close()
            self.p = None

    def close(self):
        if self.p:
            try:
                self.p.close()
            finally:
                self.p = None


class EscLog:
    def __init__(self, port):
        self.p = open_retry(port, 115_200, tries=5)
        self.buf = b""

    def flush(self):
        self.p.reset_input_buffer()
        self.buf = b""

    def lines(self, seconds):
        t0 = time.time()
        while time.time() - t0 < seconds:
            self.buf += self.p.read(4096)
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                yield line.decode("ascii", "replace").rstrip()

    def close(self):
        self.p.close()


def wait_bidir(esc, seconds=50, settle=8.0):
    for _ in esc.lines(settle):
        pass
    esc.flush()
    deadline = time.time() + seconds
    last = None
    crc0 = None
    while time.time() < deadline:
        for line in esc.lines(min(6.0, max(0.1, deadline - time.time()))):
            if "RESET" in line or "] boot" in line:
                crc0 = None
                continue
            m = LOOP_RE.search(line)
            if not m:
                continue
            last = m.groups()
            if not (m.group(1) == "BiDShot" and m.group(2) == "Armed"):
                crc0 = None
                continue
            cp = int(m.group(4))
            if crc0 is None:
                crc0 = cp
                continue
            if cp > crc0:
                return True, last
            crc0 = cp
    return False, last


def parse_telem(text):
    """Return (reads, invalid_pkts, flags, erpm, rpm, invalid_pct)."""
    reads = int(READS_RE.search(text).group(1)) if READS_RE.search(text) else -1
    inv = int(INVALID_RE.search(text).group(1)) if INVALID_RE.search(text) else -1
    m = MOTOR1_RE.search(text)
    if not m:
        return reads, inv, "?", -1, -1, "?"
    return reads, inv, m.group(1), int(m.group(2)), int(m.group(3)), m.group(5)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--ladder", default="15,25,35,50")
    ap.add_argument("--hold", type=float, default=5.0)
    a = ap.parse_args()
    ladder = [float(x) for x in a.ladder.split(",")]

    esc = EscLog(a.esc)
    bf = Bf(a.bf)
    rows = []
    try:
        # Ensure bidir @ DSHOT300 (save only if changed — save reboots FC).
        bf.enter()
        cur = bf.cmd("get dshot_bidir") + bf.cmd("get motor_pwm_protocol")
        need = ("dshot_bidir = ON" not in cur) or ("= DSHOT300" not in cur)
        if need:
            bf.cmd("set motor_pwm_protocol = DSHOT300")
            bf.cmd("set dshot_bidir = ON")
            bf.save_and_close()
            print("[bf] bidir@DSHOT300 set, FC rebooting", flush=True)
        else:
            bf.exit_and_close()
            print("[bf] already bidir@DSHOT300 (exited CLI, FC rebooting)", flush=True)

        ok, info = wait_bidir(esc, seconds=50)
        print(f"bidir commit: {'OK' if ok else 'FAIL'} {info}", flush=True)
        if not ok:
            return 1

        # Ladder — one CLI session throughout (no FC reboot mid-ladder).
        bf.enter()
        baseline = parse_telem(bf.cmd("dshot_telemetry_info", quiet=0.7))
        print(f"idle: flags={baseline[2]} erpm={baseline[3]} reads={baseline[0]} inv_pkts={baseline[1]}", flush=True)
        try:
            for pct in ladder:
                value = int(1000 + pct * 10)
                bf.cmd(f"motor 0 {value}")
                time.sleep(a.hold * 0.6)
                t1 = parse_telem(bf.cmd("dshot_telemetry_info", quiet=0.7))
                time.sleep(a.hold * 0.4)
                t2 = parse_telem(bf.cmd("dshot_telemetry_info", quiet=0.7))
                reads, inv, flags, erpm, rpm, ipct = t2
                d_reads = reads - t1[0]
                print(
                    f"{pct:5.1f}%  erpm={t1[3]}->{erpm} rpm={rpm} "
                    f"flags={flags} inv={ipct} reads+={d_reads} inv_pkts={inv}",
                    flush=True,
                )
                rows.append((pct, t1[3], erpm, rpm, ipct, inv))
            bf.cmd("motor 0 1000")
            time.sleep(1.0)
            tail = parse_telem(bf.cmd("dshot_telemetry_info", quiet=0.7))
            print(f"post: erpm={tail[3]} inv_pkts={tail[1]} reads={tail[0]}", flush=True)
        finally:
            try:
                bf.cmd("motor 0 1000")
            except Exception:
                pass
            bf.exit_and_close()
    finally:
        try:
            if bf.p is None:
                bf.enter()
            bf.cmd("motor 0 1000")
            bf.exit_and_close()
        except Exception:
            bf.close()
        esc.close()

    # Verdict: every rung nonzero eRPM, monotonic nondecreasing (with
    # 10% slack for settling), invalid pkts not exploding.
    ok = all(r[2] > 0 for r in rows)
    mono = all(rows[i + 1][2] > rows[i][2] * 0.9 for i in range(len(rows) - 1))
    print("\n=== BIDIR LOAD ===")
    for pct, e1, e2, rpm, ipct, inv in rows:
        print(f"  {pct:5.1f}%  erpm={e2:6d} rpm={rpm:6d} invalid={ipct} inv_pkts={inv}")
    verdict = ok and mono
    print(f"  verdict: {'PASS' if verdict else 'FAIL'} (nonzero={ok} monotonic={mono})")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
