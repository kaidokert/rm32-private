#!/usr/bin/env python3
"""Protocol validation matrix: PWM / DSHOT150 / DSHOT300 / DSHOT600 (+bidir).

For each protocol: switch BF via CLI (set + save → FC reboots), wait for
the ESC to re-detect (its signal-timeout reset loop IS the re-detection
mechanism), verify clean decode with FRESH evidence (settle past the
reboot churn, flush the serial backlog, then demand two consecutive
healthy samples: crc advancing, crc_fail delta zero), spin 25% for 6 s
through the BF motor override, and confirm post-exit recovery. Records a
PASS/FAIL row per protocol.

Bidir phase (--bidir): sets dshot_bidir=ON at DSHOT300, watches for the
self-validating commit (proto=BiDShot), then reads BF's
dshot_telemetry_info for the ESC's GCR responses.

Kill guards: motor restored to 1000 + CLI exited on EVERY path; the
firmware bench guard (OC/vbat, latched) is armed underneath.

Usage: protocol_matrix.py [--bf COM42] [--esc COM41]
       [--protocols DSHOT600,DSHOT150,DSHOT300,PWM] [--bidir] [--pct 25]
"""
import argparse
import re
import sys
import time

import serial

LOOP_RE = re.compile(
    r"proto=(\S+) mode=(\S+) newinput=(\d+).*?"
    r"crc_pass=(\d+) crc_fail=(\d+)"
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
    """One BF CLI session. Reopen after every save/exit (FC reboots)."""

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

    def flush(self):
        """Drop backlog — evidence must be fresh, not pre-switch stale."""
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


def wait_detect(esc, want_proto, seconds=40, settle=8.0):
    """Fresh-evidence detect: settle past the FC-reboot / ESC-reset churn,
    flush the backlog, then demand two consecutive healthy samples (crc
    advancing, crc_fail delta 0). Any RESET/boot line restarts evidence."""
    for _ in esc.lines(settle):
        pass  # burn the churn window
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
            proto, mode, newinput, cp, cf = (
                m.group(1), m.group(2), int(m.group(3)),
                int(m.group(4)), int(m.group(5)),
            )
            last = (proto, mode, newinput, cp, cf)
            if not (proto == want_proto and mode == "Armed" and newinput == 0):
                crc0 = None
                continue
            if want_proto == "PWM":
                if crc0 is None:
                    crc0 = (0, 0)
                    continue
                return True, last
            if crc0 is None:
                crc0 = (cp, cf)
                continue
            if cp > crc0[0] and cf == crc0[1]:
                return True, last
            crc0 = (cp, cf)
    return False, last


def wait_detect_any(esc, seconds=30, settle=6.0):
    for _ in esc.lines(settle):
        pass
    esc.flush()
    deadline = time.time() + seconds
    while time.time() < deadline:
        for line in esc.lines(min(6.0, max(0.1, deadline - time.time()))):
            m = LOOP_RE.search(line)
            if m and m.group(2) == "Armed" and int(m.group(3)) == 0:
                return True, (
                    m.group(1), m.group(2), int(m.group(4)), int(m.group(5)),
                )
    return False, None


def spin_check(bf, esc, pct, hold):
    """Motor override via CLI; verify the ESC went Running (heartbeat
    silence) and recovered after the exit-reboot."""
    value = int(1000 + pct * 10)
    esc.flush()
    bf.enter()
    try:
        bf.cmd(f"motor 0 {value}")
        # During the hold the [loop] heartbeat is suppressed (Running).
        loop_seen = 0
        resets = 0
        for line in esc.lines(hold):
            if "[loop" in line:
                loop_seen += 1
            if "RESET" in line:
                resets += 1
        bf.cmd("motor 0 1000")
    finally:
        try:
            bf.exit_and_close()  # FC reboots; ESC does ONE recovery cycle
        except Exception:
            bf.close()
    # Allow the post-exit reset + re-detect, then demand fresh re-arm.
    ok_rec, info = wait_detect_any(esc, seconds=30, settle=6.0)
    running_ok = loop_seen <= 1 and resets == 0
    return running_ok, ok_rec, loop_seen, resets, info


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bf", default="COM42")
    ap.add_argument("--esc", default="COM41")
    ap.add_argument(
        "--protocols", default="DSHOT600,DSHOT150,DSHOT300,PWM",
        help="comma list; BF names",
    )
    ap.add_argument("--bidir", action="store_true")
    ap.add_argument("--pct", type=float, default=25.0)
    ap.add_argument("--hold", type=float, default=6.0)
    a = ap.parse_args()

    esc = EscLog(a.esc)
    bf = Bf(a.bf)
    rows = []
    try:
        for proto in [p.strip() for p in a.protocols.split(",") if p.strip()]:
            want = "PWM" if proto == "PWM" else "DShot"
            print(f"\n=== {proto} ===", flush=True)
            bf.enter()
            bf.cmd(f"set motor_pwm_protocol = {proto}")
            bf.save_and_close()  # FC reboots
            ok_det, info = wait_detect(esc, want, seconds=40)
            print(f"  detect: {'OK' if ok_det else 'FAIL'} {info}", flush=True)
            if not ok_det:
                rows.append((proto, "FAIL(detect)", info))
                continue
            run_ok, rec_ok, loops, resets, rinfo = spin_check(bf, esc, a.pct, a.hold)
            verdict = "PASS" if (run_ok and rec_ok) else "FAIL(spin/recover)"
            print(
                f"  spin: running={'OK' if run_ok else 'NO'} "
                f"(loops_during_hold={loops} resets={resets}) "
                f"recover={'OK' if rec_ok else 'NO'} {rinfo}",
                flush=True,
            )
            rows.append((proto, verdict, rinfo))

        if a.bidir:
            print("\n=== BIDIR (DSHOT300) ===", flush=True)
            bf.enter()
            bf.cmd("set motor_pwm_protocol = DSHOT300")
            bf.cmd("set dshot_bidir = ON")
            bf.save_and_close()
            # Bidir commit: hint (100 high-idle frames) + 4 inverted-CRC
            # confirms → proto flips to BiDShot.
            ok, info = wait_detect(esc, "BiDShot", seconds=50)
            print(f"  bidir commit: {'OK' if ok else 'FAIL'} {info}", flush=True)
            bf.enter()
            telem = bf.cmd("dshot_telemetry_info", quiet=0.6)
            print("  " + telem.replace("\n", "\n  "), flush=True)
            bf.exit_and_close()
            rows.append(("BIDIR-DSHOT300", "PASS" if ok else "FAIL(commit)", info))
    finally:
        # Kill guard: never leave an override or a CLI session behind.
        try:
            bf.enter()
            bf.cmd("motor 0 1000")
            bf.exit_and_close()
        except Exception:
            bf.close()
        esc.close()

    print("\n=== MATRIX ===")
    for proto, verdict, info in rows:
        print(f"  {proto:16} {verdict:20} {info}")
    return 0 if all("PASS" in r[1] for r in rows) else 1


if __name__ == "__main__":
    sys.exit(main())
