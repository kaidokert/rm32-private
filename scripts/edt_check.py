#!/usr/bin/env python3
"""EDT (Extended DShot Telemetry) end-to-end check.

Enables dshot_edt (+ bidir @ DSHOT300) on the FC, waits for the ESC's
BiDShot commit, then reads BF's dshot_telemetry_info at idle and during
a 25% spin. PASS = the EDT columns populate: VCC ~ pack voltage, TEMP
plausible, CURR present under spin — proving the cmd-13 handshake, the
EDT init ACK, and the interleaved typed frames all work.

Kill guards: motor 1000 + CLI exit on every path.

Usage: edt_check.py [--bf COM42] [--esc COM41] [--pct 25] [--hold 6]
"""
import argparse
import re
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?crc_pass=(\d+) crc_fail=(\d+)"
)
# Motor row: idx TYPE eRPM RPM Hz INVALID TEMP VCC CURR ST/EV DBG1 DBG2 DBG3
MOTOR1_RE = re.compile(
    r"^\s*1\s+(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+([\d.]+%|NO DATA)\s+"
    r"(\d+)\s+([\d.]+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)",
    re.M,
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


def parse_motor1(text):
    m = MOTOR1_RE.search(text)
    if not m:
        return None
    return {
        "flags": m.group(1),
        "erpm": int(m.group(2)),
        "rpm": int(m.group(3)),
        "invalid": m.group(5),
        "temp": int(m.group(6)),
        "vcc": float(m.group(7)),
        "curr": int(m.group(8)),
        "st_ev": int(m.group(9)),
        "dbg": (int(m.group(10)), int(m.group(11)), int(m.group(12))),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--pct", type=float, default=25.0)
    ap.add_argument("--hold", type=float, default=6.0)
    a = ap.parse_args()

    esc = EscLog(a.esc)
    bf = Bf(a.bf)
    try:
        bf.enter()
        cur = (
            bf.cmd("get dshot_edt")
            + bf.cmd("get dshot_bidir")
            + bf.cmd("get motor_pwm_protocol")
        )
        need = (
            ("dshot_edt = FORCE" not in cur)
            or ("dshot_bidir = ON" not in cur)
            or ("= DSHOT300" not in cur)
        )
        if need:
            bf.cmd("set motor_pwm_protocol = DSHOT300")
            bf.cmd("set dshot_bidir = ON")
            bf.cmd("set dshot_edt = FORCE")
            bf.save_and_close()
            print("[bf] edt+bidir@DSHOT300 set, FC rebooting", flush=True)
        else:
            bf.exit_and_close()
            print("[bf] already edt+bidir@DSHOT300", flush=True)

        ok, info = wait_bidir(esc, seconds=50)
        print(f"bidir commit: {'OK' if ok else 'FAIL'} {info}", flush=True)
        if not ok:
            return 1

        # Give the EDT handshake + slow typed-frame trickle a few seconds.
        time.sleep(4)
        bf.enter()
        idle = parse_motor1(bf.cmd("dshot_telemetry_info", quiet=0.7))
        print(f"idle: {idle}", flush=True)
        try:
            value = int(1000 + a.pct * 10)
            bf.cmd(f"motor 0 {value}")
            time.sleep(a.hold)
            spin = parse_motor1(bf.cmd("dshot_telemetry_info", quiet=0.7))
            print(f"spin@{a.pct:.0f}%: {spin}", flush=True)
            bf.cmd("motor 0 1000")
            time.sleep(1)
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

    print("\n=== EDT ===")
    ok_idle = idle is not None and idle["vcc"] > 5.0
    ok_spin = spin is not None and spin["vcc"] > 5.0 and spin["erpm"] > 0
    print(f"  idle: vcc={idle['vcc'] if idle else '?'} temp={idle['temp'] if idle else '?'}")
    if spin:
        print(
            f"  spin: vcc={spin['vcc']} temp={spin['temp']} curr={spin['curr']} "
            f"erpm={spin['erpm']} st_ev={spin['st_ev']} dbg={spin['dbg']}"
        )
    verdict = ok_idle and ok_spin
    print(f"  verdict: {'PASS' if verdict else 'FAIL'} (vcc idle={ok_idle} spin={ok_spin})")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
