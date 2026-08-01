#!/usr/bin/env python3
"""Downthrottle-blip / double-slam test — the clone re-qual's signature
move, through BF DSHOT300 (unidir).

Sequence per slam cycle: 100% hold, slam to 10%, hold, slam back to
100%, hold, slam to stop. The parity question: do downthrottle
transitions produce desyncs/resets (the historical 'downthrottle blip')
or ride clean like the clone's re-qual artifact (dsy~0 through double
slams at 3145 Hz)?

Verdict from [sr] counter deltas (dsy/exc/cm) + resets, exactly like
bf_ladder. Kill-guarded.

Usage: bf_slam.py [--bf COM42] [--esc COM41] [--cycles 3]
"""
import argparse
import re
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?crc_pass=(\d+) crc_fail=(\d+)"
)
SR_RE = re.compile(
    r"\[sr n=(\d+) ci=(\d+) ma=(\d+) mv=(\d+).*?dsy=(\d+) exc=(\d+) wex=(\d+) cm=(\d+)\]"
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

    def motor(self, value):
        self.p.write(f"motor 0 {value}\n".encode())

    def save_and_close(self):
        try:
            self.p.write(b"save\n")
            time.sleep(0.5)
        finally:
            self.p.close()
            self.p = None

    def cmd(self, c, quiet=0.4):
        self.p.write((c + "\n").encode())
        return drain(self.p, quiet)

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


def read_sr(esc, seconds=8):
    last = None
    for line in esc.lines(seconds):
        m = SR_RE.search(line)
        if m:
            last = {
                "dsy": int(m.group(5)),
                "exc": int(m.group(6)),
                "wex": int(m.group(7)),
                "cm": int(m.group(8)),
            }
    return last


def wait_ready(esc, seconds=40, settle=8.0):
    for _ in esc.lines(settle):
        pass
    esc.flush()
    deadline = time.time() + seconds
    crc0 = None
    while time.time() < deadline:
        for line in esc.lines(min(6.0, max(0.1, deadline - time.time()))):
            if "RESET:" in line or "] boot" in line:
                crc0 = None
                continue
            m = LOOP_RE.search(line)
            if not m or m.group(1) != "DShot" or m.group(2) != "Armed":
                continue
            cp, cf = int(m.group(4)), int(m.group(5))
            if crc0 is None:
                crc0 = (cp, cf)
                continue
            if cp > crc0[0] and cf == crc0[1]:
                return True
            crc0 = (cp, cf)
    return False


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--cycles", type=int, default=3)
    a = ap.parse_args()

    esc = EscLog(a.esc)
    bf = Bf(a.bf)
    try:
        bf.enter()
        cur = bf.cmd("get dshot_bidir") + bf.cmd("get motor_pwm_protocol")
        if ("dshot_bidir = OFF" not in cur) or ("= DSHOT300" not in cur):
            bf.cmd("set motor_pwm_protocol = DSHOT300")
            bf.cmd("set dshot_bidir = OFF")
            bf.save_and_close()
            print("[bf] unidir DSHOT300 set", flush=True)
        else:
            bf.exit_and_close()
            print("[bf] already unidir DSHOT300", flush=True)
        if not wait_ready(esc):
            print("not ready")
            return 1
        pre = read_sr(esc, 6)
        print(f"pre:  {pre}", flush=True)
        if pre is None:
            return 1

        esc.flush()
        resets = 0
        bf.enter()
        try:
            # Engage gently first (clone protocol: slams happen FROM a
            # running state). 10% for 5 s establishes lock — same engage
            # the flight ladder locks 1.7M comms with. The slam FLOOR is
            # 10% (never a dead stop mid-test): the item-5 question is
            # downthrottle-transition behavior, not the engage lottery.
            bf.motor(1100)
            for line in esc.lines(5.0):
                if "RESET:" in line or "] boot" in line:
                    resets += 1
            for n in range(a.cycles):
                # 100% hold, slam to 10%, hold, slam back — from running.
                for value, hold in (
                    (2000, 2.5), (1100, 2.0), (2000, 2.0), (1100, 1.5),
                ):
                    bf.motor(value)
                    for line in esc.lines(hold):
                        if "RESET:" in line or "] boot" in line:
                            resets += 1
                print(f"cycle {n + 1} done (resets so far: {resets})",
                      flush=True)
            bf.motor(1000)
        finally:
            try:
                bf.cmd("motor 0 1000")
            except Exception:
                pass
            bf.exit_and_close()
        time.sleep(2)
        post = read_sr(esc, 8)
        print(f"post: {post}", flush=True)
    finally:
        try:
            if bf.p is None:
                bf.enter()
            bf.cmd("motor 0 1000")
            bf.exit_and_close()
        except Exception:
            bf.close()
        esc.close()

    d_dsy = post["dsy"] - pre["dsy"] if post else -1
    d_exc = post["exc"] - pre["exc"] if post else -1
    d_cm = post["cm"] - pre["cm"] if post else -1
    print("\n=== DOUBLE-SLAM ===")
    print(f"  cycles={a.cycles} comms={d_cm} dsy={d_dsy} exc={d_exc} "
          f"resets={resets}")
    print("  (clone re-qual artifact rode double slams at dsy~0)")
    # cm must show real locked commutations or the dsy=0 is vacuous
    # (a churned run never enters interrupt mode and counts nothing).
    verdict = post is not None and d_dsy == 0 and resets == 0 and d_cm > 100_000
    print(f"  verdict: {'PASS' if verdict else 'CHECK'}")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
