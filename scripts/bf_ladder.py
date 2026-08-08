#!/usr/bin/env python3
"""Flight-grade envelope re-qual: 2% ladder to 100% under BF DSHOT300.

The parity standard (ladder_quiet.py) re-run through the PRODUCTION
input path: BF CLI motor overrides step 10->100->10 in 2% rungs with
1.5 s dwells, ESC log captured passively (poll law: zero ESC-side
queries during the ladder — the heartbeat is silent while Running
anyway). Verdict from post-run counter deltas in the [sr] heartbeat
line (dsy / exc / wex / cm, debuguart-gated parity counters) plus the
onboard flight-recorder rings read via probe-rs BEFORE the CLI exit
(the exit reboots the FC -> ESC recovery reset wipes the rings).

Parity standard to beat (bench-UART, 07-31): dsy=0, exc<=2, ~1.7M
comms over the same ladder.

Kill guards: motor 1000 + CLI exit on every path; firmware bench guard
(OC/vbat latched) armed underneath.

Usage: bf_ladder.py [--bf COM42] [--esc COM41] [--dwell 1.5]
       [--top 100] [--no-rings]
"""
import argparse
import re
import subprocess
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?crc_pass=(\d+) crc_fail=(\d+)"
)
SR_RE = re.compile(
    r"\[sr n=(\d+) ci=(\d+) ma=(\d+) mv=(\d+).*?dsy=(\d+) exc=(\d+) wex=(\d+) cm=(\d+)\]"
)
PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
# From arm-none-eabi-nm on the current debuguart ELF (rebuild + re-check
# if the build changes materially).
SR_HEAD_ADDR = "0x20001efc"
SR_CI_ADDR = "0x20000c38"
SR_MA_ADDR = "0x20001278"
SR_MV_ADDR = "0x200018b8"
SR_N = 800


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
    def __init__(self, port, logfile=None):
        self.p = open_retry(port, 115_200, tries=5)
        self.buf = b""
        self.log = (
            open(logfile, "w", encoding="ascii", errors="replace")
            if logfile
            else None
        )
        self.events = 0  # RESET last-words + boot banners, whole session
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
                if text and self.log:
                    self.log.write(f"{time.time():.2f} {text}\n")
                    self.log.flush()
                if "RESET:" in text or "] boot" in text:
                    self.events += 1
                if "BENCH KILL" in text and self.killed is None:
                    self.killed = text
                yield text

    def close(self):
        if self.log:
            self.log.close()
        self.p.close()


def wait_ready(esc, seconds=45, settle=8.0):
    """Fresh-evidence: DShot armed with crc advancing, crc_fail stable."""
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
            if not (m.group(1) == "DShot" and m.group(2) == "Armed"):
                crc0 = None
                continue
            cp, cf = int(m.group(4)), int(m.group(5))
            if crc0 is None:
                crc0 = (cp, cf)
                continue
            if cp > crc0[0] and cf == crc0[1]:
                return True, last
            crc0 = (cp, cf)
    return False, last


def read_sr_counters(esc, seconds=8):
    """Read the newest [sr ...] heartbeat line (idle only)."""
    last = None
    for line in esc.lines(seconds):
        m = SR_RE.search(line)
        if m:
            last = {
                "n": int(m.group(1)),
                "dsy": int(m.group(5)),
                "exc": int(m.group(6)),
                "wex": int(m.group(7)),
                "cm": int(m.group(8)),
            }
    return last


def probe_read(addr, words, width):
    out = subprocess.run(
        ["probe-rs", "read", width, addr, str(words), "--chip", CHIP,
         "--probe", PROBE],
        capture_output=True, text=True, timeout=30,
    )
    return [int(w, 16) for w in out.stdout.split()]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument("--dwell", type=float, default=1.5)
    ap.add_argument("--top", type=int, default=100)
    ap.add_argument("--no-rings", action="store_true")
    ap.add_argument("--esclog", default="bf_ladder_esc.log")
    a = ap.parse_args()

    esc = EscLog(a.esc, logfile=a.esclog)
    bf = Bf(a.bf)
    rings = None
    try:
        bf.enter()
        cur = bf.cmd("get dshot_bidir") + bf.cmd("get motor_pwm_protocol")
        if ("dshot_bidir = OFF" not in cur) or ("= DSHOT300" not in cur):
            bf.cmd("set motor_pwm_protocol = DSHOT300")
            bf.cmd("set dshot_bidir = OFF")
            bf.save_and_close()
            print("[bf] unidir DSHOT300 set, FC rebooting", flush=True)
        else:
            bf.exit_and_close()
            print("[bf] already unidir DSHOT300", flush=True)

        ok, info = wait_ready(esc)
        print(f"ready: {'OK' if ok else 'FAIL'} {info}", flush=True)
        if not ok:
            return 1
        pre = read_sr_counters(esc, seconds=6)
        print(f"pre:  {pre}", flush=True)
        if pre is None:
            print("no [sr] line — wrong firmware?", flush=True)
            return 1

        rungs = list(range(10, a.top + 1, 2)) + list(range(a.top - 2, 9, -2))
        print(f"== ladder 10->{a.top}->10, {len(rungs)} rungs, "
              f"dwell {a.dwell}s ==", flush=True)
        esc.flush()
        bf.enter()
        resets_during = 0
        try:
            t0 = time.time()
            for pct in rungs:
                bf.p.write(f"motor 0 {1000 + pct * 10}\n".encode())
                # Passive ESC watch during the dwell (no queries). Count
                # BOTH last-words lines and boot banners — a hard reset
                # (IWDG/panic) prints only the banner.
                for line in esc.lines(a.dwell):
                    if "RESET:" in line or "] boot" in line:
                        resets_during += 1
            bf.p.write(b"motor 0 1000\n")
            elapsed = time.time() - t0
            print(f"ladder done in {elapsed:.0f}s resets={resets_during}",
                  flush=True)
            # Idle: heartbeat resumes; read post counters while the CLI is
            # still open (no FC reboot => rings intact).
            time.sleep(2)
            post = read_sr_counters(esc, seconds=8)
            print(f"post: {post}", flush=True)
            if not a.no_rings:
                head = probe_read(SR_HEAD_ADDR, 1, "b32")[0]
                ci = probe_read(SR_CI_ADDR, SR_N, "b16")
                ma = probe_read(SR_MA_ADDR, SR_N, "b16")
                mv = probe_read(SR_MV_ADDR, SR_N, "b16")
                rings = (head, ci, ma, mv)
                print(f"rings read: head={head}", flush=True)
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

    print("\n=== FLIGHT-GRADE RE-QUAL (BF DSHOT300) ===")
    d_dsy = post["dsy"] - pre["dsy"] if post else -1
    d_exc = post["exc"] - pre["exc"] if post else -1
    d_cm = post["cm"] - pre["cm"] if post else -1
    print(f"  esc_events_total={esc.events} (RESET/boot, whole session)")
    print(f"  comms={d_cm} dsy={d_dsy} exc>25%={d_exc} "
          f"({(d_exc * 1000 / d_cm) if d_cm > 0 else -1:.2f}/1k) "
          f"wex={post['wex'] / 10 if post else -1:.1f}% "
          f"resets_during={resets_during}")
    print("  (bench-UART parity standard: dsy=0, exc=2, ~1.7M comms)")
    if rings:
        head, ci, ma, mv = rings
        n = min(head, SR_N)
        start = head % SR_N if head > SR_N else 0
        # Plateau summary: min ci (=max speed), max current, min vbat.
        seq = [(ci[(start + k) % SR_N], ma[(start + k) % SR_N],
                mv[(start + k) % SR_N]) for k in range(n)]
        run = [x for x in seq if 0 < x[0] < 12000]
        if run:
            print(f"  recorder: {len(run)} in-run samples; "
                  f"min_ci={min(x[0] for x in run)} "
                  f"max_mA={max(x[1] for x in run)} "
                  f"min_mV={min(x[2] for x in run)}")
    if esc.killed:
        print(f"  !! bench guard fired: {esc.killed}")
    verdict = (
        post is not None
        and d_dsy == 0
        and resets_during == 0
        and d_cm > 500_000
        and esc.killed is None
    )
    print(f"  verdict: {'PASS' if verdict else 'CHECK'}")
    return 0 if verdict else 1


if __name__ == "__main__":
    sys.exit(main())
