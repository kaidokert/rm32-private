#!/usr/bin/env python3
"""Protocol robustness soak — FC-reboot axis.

For each protocol, cycle: FC reboot (CLI exit) → ESC sees the signal
gap, resets, re-detects, re-arms → verify fresh-evidence detection
(proto match, Armed, crc advancing for DShot). Counts successes and
re-detect latency. Idle-only (no spins) — safe on a marginal supply.

The cold-boot and battery-replug axes need a human hand; this covers
the FC-reboot axis unattended.

Usage: bf_soak.py [--bf COM42] [--esc COM41] [--cycles 5]
       [--protocols DSHOT300,DSHOT600,DSHOT150,PWM]
"""
import argparse
import re
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?crc_pass=(\d+) crc_fail=(\d+)"
)


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

    def cmd(self, c, quiet=0.4):
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
        self.killed = None  # first "BENCH KILL" line seen, if any

    def flush(self):
        self.p.reset_input_buffer()
        self.buf = b""

    def lines(self, seconds):
        t0 = time.time()
        while time.time() - t0 < seconds:
            self.buf += self.p.read(4096)
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                text = line.decode("ascii", "replace").rstrip()
                if "BENCH KILL" in text and self.killed is None:
                    self.killed = text
                yield text

    def close(self):
        self.p.close()


def wait_ready(esc, want, seconds=40):
    """Fresh evidence, measured from call: returns (ok, latency_s)."""
    t0 = time.time()
    esc.flush()
    deadline = t0 + seconds
    crc0 = None
    while time.time() < deadline:
        for line in esc.lines(min(6.0, max(0.1, deadline - time.time()))):
            if "RESET:" in line or "] boot" in line:
                crc0 = None
                continue
            m = LOOP_RE.search(line)
            if not m:
                continue
            if not (m.group(1) == want and m.group(2) == "Armed"
                    and int(m.group(3)) == 0):
                crc0 = None
                continue
            if want == "PWM":
                return True, time.time() - t0
            cp, cf = int(m.group(4)), int(m.group(5))
            if crc0 is None:
                crc0 = (cp, cf)
                continue
            if cp > crc0[0] and cf == crc0[1]:
                return True, time.time() - t0
            crc0 = (cp, cf)
    return False, time.time() - t0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--cycles", type=int, default=5)
    ap.add_argument("--protocols", default="DSHOT300,DSHOT600,DSHOT150,PWM")
    a = ap.parse_args()

    esc = EscLog(a.esc)
    bf = Bf(a.bf)
    rows = []
    try:
        for proto in [p.strip() for p in a.protocols.split(",") if p.strip()]:
            want = "PWM" if proto == "PWM" else "DShot"
            bf.enter()
            bf.cmd(f"set motor_pwm_protocol = {proto}")
            bf.cmd("set dshot_bidir = OFF")
            bf.save_and_close()
            ok0, lat0 = wait_ready(esc, want)
            oks, lats = (1 if ok0 else 0), [lat0]
            for _ in range(a.cycles - 1):
                bf.enter()
                bf.exit_and_close()  # FC reboot = the soak stimulus
                ok, lat = wait_ready(esc, want)
                oks += 1 if ok else 0
                lats.append(lat)
            print(
                f"{proto:9} {oks}/{a.cycles} re-detects, latency "
                f"min/med/max = {min(lats):.1f}/{sorted(lats)[len(lats)//2]:.1f}/{max(lats):.1f}s",
                flush=True,
            )
            rows.append((proto, oks, a.cycles))
    finally:
        bf.close()
        esc.close()

    total = sum(r[1] for r in rows)
    want_total = sum(r[2] for r in rows)
    print(f"\n=== SOAK: {total}/{want_total} ===")
    return 0 if total == want_total else 1


if __name__ == "__main__":
    sys.exit(main())
