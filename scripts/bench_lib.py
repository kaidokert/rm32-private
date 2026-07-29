#!/usr/bin/env python3
"""bench_lib — shared bench discipline for ALL rm32 motor scripts.

Every artifact that ate the 07-27/28 session came from ad-hoc scripts
each reinventing (and botching) the same plumbing. This library bakes
the rules in so a script CANNOT silently repeat them:

  1. ENGAGE ONLY FROM A STOPPED ROTOR. rm32 cannot lock onto a coasting
     rotor; a 2.4 s post-reset wait re-engaged into motion and produced
     the entire phantom "engage lottery / wall". `engage_from_stop`
     enforces a full settle (default 8 s) and a gentle 10%-first ramp.
  2. A SAMPLE ONLY COUNTS IF THE MOTOR IS VERIFIABLY LOCKED. old=0 AND
     real current (a locked 40% draws ~1 A; 0.5 A at high duty = NOT
     locked). `verify_locked` applies both gates; the "83% post-ZC"
     retraction happened because samples were taken in safe-mode.
  3. EVERY RUN WATCHES FOR REBOOTS, WITH CAUSE. Boot banners and
     `last reset:` lines are scanned live (not discarded); a mid-run
     reboot is reported with the throttle it happened at.
  4. KILL ON EVERY EXIT PATH. Bench is a context manager: rampdown + 'w'
     spam runs on success, exception, and KeyboardInterrupt alike.
  5. INFO-LINE UNITS ARE SCALED. vbat field = mV*100/752, iraw field =
     mA*100/2686. `Info` exposes .volts / .amps so nobody re-derives
     them wrong ("vbat=1580 means 1.6 V" cost half a day).

Typical use:

    from bench_lib import Bench, reset_board
    reset_board()                      # capture boot banner + cause
    with Bench() as b:
        if not b.engage_from_stop(70): # settle, gentle ramp, verified
            sys.exit("engage failed: " + b.last_engage_report)
        inf = b.sample(70)             # verified-locked sample or None
    # motor killed here no matter what happened above
"""

import re
import subprocess
import threading
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"
DEFAULT_PORT = "COM41"
BAUD = 2_000_000

# Gentle engage ramp proven to lock (07-28: 3/3 from stop, clone-parity
# ci). Coarser/faster ramps drop into the OldRoutine limit cycle.
ENGAGE_RAMP = (10, 15, 20, 25, 30, 40, 55, 70, 80, 90, 100)
STAGE_SECS = 2.6
# Full-stop settle before an engage. 2.4 s (the artifact) is NOT enough;
# 8 s measured reliable.
STOP_SETTLE_SECS = 8.0

# Minimum plausible current (A) for a genuinely locked motor at/above a
# given throttle %. Well below healthy values (40%~1.1 A, 80%~4.2 A) to
# tolerate supply variation, but far above the ~0.5 A safe-mode crawl.
AMP_FLOOR = [(40, 0.7), (55, 1.2), (70, 2.0), (85, 3.0)]


def amp_floor(pct):
    floor = 0.0
    for p, a in AMP_FLOOR:
        if pct >= p:
            floor = a
    return floor


def reset_board(port=DEFAULT_PORT, capture=True):
    """probe-rs reset; if capture, open the port FIRST so the boot
    banner + RCC_CSR reset causes land in the return value instead of
    being lost. Returns (banner_seen, causes)."""
    if not capture:
        subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
                       capture_output=True)
        time.sleep(2.4)
        return None, []
    p = serial.Serial(port, BAUD, timeout=0.05)
    buf = bytearray()
    stop = [False]

    def rd():
        while not stop[0]:
            buf.extend(p.read(8192))

    th = threading.Thread(target=rd)
    th.start()
    time.sleep(0.3)
    subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
                   capture_output=True)
    time.sleep(2.6)
    stop[0] = True
    th.join()
    p.close()
    txt = "".join(chr(b) if 32 <= b < 127 or b == 10 else "." for b in buf)
    causes = [m.strip() for m in re.findall(r"last reset:\s*([a-zA-Z0-9\- ]+)", txt)]
    return "[rm32] boot" in txt, causes


class Info:
    """One parsed `i` line with unit conversions applied."""

    def __init__(self, fields):
        self.raw = fields
        g = lambda k, d=0: int(fields.get(k, d))
        self.old = g("old", 9)
        self.run = g("run")
        self.ci = g("ci")
        self.duty = g("duty")
        self.zc = g("zc")
        self.dsy = g("dsy")
        self.otrip = g("otrip")
        self.killed = g("killed")
        self.volts = g("vbat") * 752 / 100 / 1000  # field -> volts
        self.amps = g("iraw") * 2686 / 100 / 1000  # field -> amps
        self.running = self.old == 0 and self.run == 1

    def __repr__(self):
        m = "RUN" if self.running else ("safe" if self.old == 1 else "?")
        return (f"{m} ci={self.ci} duty={self.duty} zc={self.zc} "
                f"I={self.amps:.1f}A V={self.volts:.2f}V dsy={self.dsy}")


def verify_locked(info, pct):
    """Rule 2: a sample counts only when verifiably locked. Returns
    (ok, reason)."""
    if info is None:
        return False, "no info line"
    if not info.running:
        return False, f"not Running (old={info.old} run={info.run})"
    if info.zc < 9000 and info.ci >= 600:
        return False, f"loose (ci={info.ci}, zc={info.zc})"
    if info.amps < amp_floor(pct):
        return False, (f"current implausible for {pct}% "
                       f"({info.amps:.1f}A < {amp_floor(pct):.1f}A floor)")
    return True, "locked"


class Bench:
    """Serial bench session with reboot watching and guaranteed kill."""

    def __init__(self, port=DEFAULT_PORT):
        self.p = serial.Serial(port, BAUD, timeout=0.05)
        self.buf = bytearray()
        self.boots_seen = 0
        self.reboot_events = []  # (throttle, cause)
        self.kill_line_seen = False
        self.cur_pct = 0
        self.last_engage_report = ""

    # -- context manager: rule 4 --------------------------------------
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        try:
            self.rampdown_kill()
        finally:
            self.p.close()
        return False  # never swallow exceptions

    # -- stream scanning: rule 3 --------------------------------------
    def _scan(self):
        txt = "".join(chr(b) if 32 <= b < 127 or b == 10 else "."
                      for b in self.buf)
        n = txt.count("[rm32] boot")
        if n > self.boots_seen:
            causes = re.findall(r"last reset:\s*([a-zA-Z0-9\- ]+)", txt)
            cause = causes[-1].strip() if causes else "?"
            for _ in range(n - self.boots_seen):
                self.reboot_events.append((self.cur_pct, cause))
                print(f"  !! REBOOT at throttle={self.cur_pct}% cause='{cause}'")
            self.boots_seen = n
        if "!! BENCH" in txt and not self.kill_line_seen:
            self.kill_line_seen = True
            k = txt.find("!! BENCH")
            print("  !! " + txt[k:k + 70].strip())

    # -- primitives ----------------------------------------------------
    def hold(self, pct, secs):
        """Hold a throttle, resending at ~10 Hz, scanning for reboots."""
        self.cur_pct = pct
        t0 = time.time()
        while time.time() - t0 < secs:
            self.p.write(f"{pct}\n".encode())
            self.p.flush()
            got = self.p.read(16384)
            if got:
                self.buf.extend(got)
                self._scan()
            time.sleep(0.08)

    def cmd(self, byte, settle=0.2):
        self.p.write(byte)
        self.p.flush()
        time.sleep(settle)
        self.buf.extend(self.p.read(16384))
        self._scan()

    def info(self):
        """Request and parse one `i` line (None on parse failure)."""
        n = len(self.buf)
        self.cmd(b"i", settle=0.3)
        seg = "".join(chr(b) if 32 <= b < 127 else "." for b in self.buf[n:])
        for line in seg.split("\n"):
            if line.startswith("i "):
                fields = dict(x.split("=") for x in line.split() if "=" in x)
                try:
                    return Info(fields)
                except ValueError:
                    return None
        return None

    def sample(self, pct):
        """Rule 2 sample: Info if verifiably locked at pct, else None
        (reason printed)."""
        inf = self.info()
        ok, reason = verify_locked(inf, pct)
        if not ok:
            print(f"  [sample@{pct}%] REJECTED: {reason}"
                  + (f" ({inf})" if inf else ""))
            return None
        return inf

    # -- engage: rules 1 + 2 ------------------------------------------
    def engage_from_stop(self, target, settle=STOP_SETTLE_SECS,
                         stage_secs=STAGE_SECS, verbose=True):
        """Full-stop settle, then the gentle proven ramp to `target`.
        Verifies lock at the target; returns bool, detail in
        last_engage_report."""
        self.hold(0, settle)
        stages = [s for s in ENGAGE_RAMP if s < target] + [target]
        for s in stages:
            self.hold(s, stage_secs)
        inf = self.info()
        ok, reason = verify_locked(inf, target)
        self.last_engage_report = f"{reason}" + (f" ({inf})" if inf else "")
        if verbose:
            print(f"  engage->{target}%: "
                  f"{'OK ' if ok else 'FAIL '}{self.last_engage_report}")
        return ok

    # -- kill: rule 4 ---------------------------------------------------
    def rampdown_kill(self):
        """Walk the throttle down (coast-rectification guard), then kill."""
        try:
            for pct in (60, 40, 25, 15):
                if pct < self.cur_pct:
                    self.hold(pct, 0.35)
        except Exception:
            pass
        for _ in range(5):
            try:
                self.p.write(b"w")
                self.p.flush()
            except Exception:
                break
            time.sleep(0.1)
