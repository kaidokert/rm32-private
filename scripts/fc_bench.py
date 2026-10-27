#!/usr/bin/env python3
"""Shared plumbing for the FC-driven bench scripts (bf_ladder, bf_slam).

Bench topology (docs/gc_LAB_NOTEBOOK.md, 2026-10-27): Betaflight on the
F411 drives L431 on `motor 0` and binz on `motor 1`; the relay picks
which board owns the one motor. Each board logs on its own serial port
(bench_ports.py roles).

Evidence is the ESC's own `[run]` session summary (rm32 debuguart, all
chips), printed after the stop: one start, desync counters, locked
commutations. Nothing is queried from the ESC during a run.
"""
import re
import subprocess
import sys
import time
from pathlib import Path

import serial

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
from bench_ports import port  # noqa: E402

INDEX = {"l431": 0, "binz": 1}
RUN_RE = re.compile(
    r"\[run\] starts=(\d+) ms=(\d+) zc_pk=(\d+) ecom_min=(\d+) erpm_pk=(\d+) "
    r"dsy=(\d+) fast=(\d+) cur=(\d+) slow=(\d+) old=(\d+) orbit=(\d+) comms=(\d+)"
)
RUN_KEYS = ("starts", "ms", "zc_pk", "ecom_min", "erpm_pk", "dsy", "fast",
            "cur", "slow", "old", "orbit", "comms")


def parse_run(line):
    m = RUN_RE.search(line)
    return dict(zip(RUN_KEYS, map(int, m.groups()))) if m else None


def route_motor(board):
    """Relay the motor to `board`; raise unless the relay confirms it."""
    py = sys.executable
    for verb in ("set", "check"):
        r = subprocess.run([py, str(HERE / "relay.py"), verb, board],
                           capture_output=True, text=True)
        if r.returncode:
            raise RuntimeError(f"relay {verb} {board} failed: {r.stdout}{r.stderr}")
    time.sleep(0.5)


def open_retry(dev, baud, tries=30, delay=0.5):
    for _ in range(tries):
        try:
            return serial.Serial(dev, baud, timeout=0.05)
        except serial.SerialException:
            time.sleep(delay)
    raise RuntimeError(f"cannot open {dev} after {tries} tries")


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
    """One BF CLI session. `save`/`exit` reboot the FC: reopen after."""

    def __init__(self, dev, index):
        self.dev = dev
        self.index = index
        self.p = None

    def enter(self):
        self.p = open_retry(self.dev, 115_200)
        self.p.write(b"#\n")
        banner = drain(self.p, 0.5)
        if "CLI" not in banner and "#" not in banner:
            raise RuntimeError(f"no CLI banner: {banner[:80]!r}")

    def cmd(self, c, quiet=0.4):
        self.p.write((c + "\n").encode())
        return drain(self.p, quiet)

    def motor(self, value):
        self.p.write(f"motor {self.index} {value}\n".encode())

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

    def stop_and_exit(self):
        """Kill guard: motor to 1000 and leave the CLI, on any state."""
        try:
            if self.p is None:
                self.enter()
            self.motor(1000)
            time.sleep(0.2)
            self.exit_and_close()
        except Exception:
            self.close()

    def ensure_unidir_dshot300(self):
        """The re-qual standard runs unidir DSHOT300. Returns True if the
        FC was reconfigured (and is rebooting)."""
        self.enter()
        cur = self.cmd("get dshot_bidir") + self.cmd("get motor_pwm_protocol")
        if ("dshot_bidir = OFF" in cur) and ("= DSHOT300" in cur):
            self.exit_and_close()
            return False
        self.cmd("set motor_pwm_protocol = DSHOT300")
        self.cmd("set dshot_bidir = OFF")
        self.save_and_close()
        return True

    def restore_bench_standard(self):
        """Bench standard between campaigns: bidir DSHOT300."""
        try:
            self.enter()
            self.motor(1000)
            self.cmd("set motor_pwm_protocol = DSHOT300")
            self.cmd("set dshot_bidir = ON")
            self.save_and_close()
        except Exception:
            self.close()


class EscLog:
    """Passive ESC log reader: timestamps every line to `logfile`, counts
    resets (last-words + boot banners), keeps the first BENCH KILL and
    every `[run]` summary."""

    def __init__(self, dev, logfile=None):
        self.p = open_retry(dev, 115_200, tries=5)
        self.buf = b""
        self.log = open(logfile, "w", encoding="ascii", errors="replace") if logfile else None
        self.resets = 0
        self.killed = None
        self.runs = []

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
                if text and self.log:
                    self.log.write(f"{time.time():.2f} {text}\n")
                    self.log.flush()
                if "RESET:" in text or "] boot" in text or "last reset:" in text:
                    self.resets += 1
                if "BENCH KILL" in text and self.killed is None:
                    self.killed = text
                run = parse_run(text)
                if run:
                    self.runs.append(run)
                yield text

    def watch(self, seconds):
        for _ in self.lines(seconds):
            pass

    def close(self):
        if self.log:
            self.log.close()
        self.p.close()


def wait_ready(esc, seconds=45, settle=8.0, quiet=6.0):
    """Ready = no reset or boot line for `quiet` seconds after the
    FC-reboot churn. Chip-independent: the binz [loop] heartbeat is too
    slow at idle to key on."""
    esc.watch(settle)
    deadline = time.time() + seconds
    while time.time() < deadline:
        n0 = esc.resets
        esc.watch(quiet)
        if esc.resets == n0:
            return True
    return False


def wait_run(esc, seconds=6.0):
    """Collect the `[run]` line that follows a stop; None if absent."""
    n0 = len(esc.runs)
    t0 = time.time()
    while time.time() - t0 < seconds and len(esc.runs) == n0:
        esc.watch(0.2)
    return esc.runs[-1] if len(esc.runs) > n0 else None
