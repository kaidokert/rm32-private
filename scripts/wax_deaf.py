#!/usr/bin/env python3
"""Bedrock analog test — arc-vs-VALUE at the 100% battery wall.

The whole analog case (deaf window = comparator doesn't flip, therefore
CPU-load-independent, therefore NOT a budget problem) rests on ONE
PSU-60%-era observation. This re-confirms it at the actual battery wall.

Instrument: WAXWING ring, one row per 20 kHz tick, frozen on the first
dropout (desync/orbit) at wall duty by the firmware's freeze-on-fall.
Each row: phase-A (JDR1), phase-B (JDR2), TIM2.CNT (0.5 us), and a packed
T1S word whose bit15 = the COMP VALUE bit sampled the SAME tick (1 =
comparator flipped to post-ZC level), bits12-14 = commutation step,
bits0-11 = TIM1.CNT.

Procedure: 'J' arms the injected burst so the ring writes; climb to the
wall and hold; the firmware freezes the ring on the first wall dropout;
'x' dumps the 1024-row post-mortem; we decode whether the phase arc
crossed neutral through a window where bit15 (COMP VALUE) never flipped.

Kill guards (project rule): 'w' on EVERY exit path; walk the throttle
down before kill (coast-rectification spike guard); abort on the
firmware's own '!! BENCH' kill line.

Usage: wax_deaf.py --hold 92 --wall-secs 30
"""
import argparse
import subprocess
import sys
import threading
import time

import serial

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"


def reset_and_capture_cause(port):
    """Open BEFORE reset so the boot banner + RCC_CSR reset-cause is
    caught. Returns (causes, banner_seen)."""
    p = serial.Serial(port, 2_000_000, timeout=0.05)
    buf = bytearray()
    stop = [False]

    def rd():
        while not stop[0]:
            buf.extend(p.read(4096))

    th = threading.Thread(target=rd)
    th.start()
    time.sleep(0.3)
    subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE],
                   capture_output=True)
    time.sleep(3.0)
    stop[0] = True
    th.join()
    p.close()
    txt = "".join(chr(b) if 32 <= b < 127 or b == 10 else "." for b in buf)
    causes = [l.strip() for l in txt.split("\n") if "last reset:" in l]
    banner = "[rm32] boot" in txt or "rm32" in txt
    return causes, banner, buf


class Bench:
    def __init__(self, port):
        self.p = serial.Serial(port, 2_000_000, timeout=0.05)
        self.raw = bytearray()

    def cmd(self, b, settle=0.2):
        self.p.write(b)
        self.p.flush()
        time.sleep(settle)
        self.raw += self.p.read(8192)

    def dwell(self, pct, secs, watch_kill=True):
        """Hold throttle; return True if the firmware kill line or a wax
        freeze line was seen during the dwell."""
        t0 = time.time()
        froze = False
        while time.time() - t0 < secs:
            self.p.write(f"{pct}\n".encode())
            self.p.flush()
            chunk = self.p.read(65536)
            self.raw += chunk
            if b"!! BENCH" in chunk:
                print("  ABORT: firmware kill line seen")
                return "kill"
            if b"FROZEN" in chunk and not froze:
                froze = True
                print(f"  [wax] FROZEN detected at t={time.time()-t0:.1f}s")
                return "froze"  # stop the hold; ring is preserved, dump now
            time.sleep(0.1)
        return "froze" if froze else "timeout"

    def spinup(self, target):
        """Patient staged spin-up: BEMF must lock at each stage before the
        next, else the motor drops to OldRoutine and the reclimb clamp
        pins duty low (the too-fast-climb failure). ~3 s/stage locks
        cleanly (verified: 20%%->Running, ci falls smoothly with throttle)."""
        stages = [s for s in (20, 25, 30, 40, 55, 70, 80) if s < target] + [target]
        for pct in stages:
            r = self.dwell(pct, 3.0)
            if r == "kill":
                return "kill"
        return "ok"

    def rampdown(self):
        for pct in (60, 40, 25, 15):
            self.dwell(pct, 0.4)

    def kill(self):
        self.p.write(b"w")
        self.p.flush()
        time.sleep(0.2)
        self.raw += self.p.read(65536)

    def drain(self, secs=3.0):
        t0 = time.time()
        while time.time() - t0 < secs:
            self.raw += self.p.read(400000)
            time.sleep(0.1)

    def wx_dump(self, secs=4.0):
        """Continuous read until 'WX END' — the 20 KB burst overflows the
        OS serial buffer if we sleep-then-read. The FIRST 'x' after a
        freeze re-arms the ring on completion, so we get exactly one shot;
        it must capture the whole thing."""
        self.p.reset_input_buffer()
        self.p.write(b"x")
        self.p.flush()
        t0 = time.time()
        while time.time() - t0 < secs:
            chunk = self.p.read(200000)
            if chunk:
                self.raw += chunk
                if b"WX END" in bytes(self.raw[-400000:]):
                    break


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--hold", type=int, default=92,
                    help="wall hold throttle %% (climb target)")
    ap.add_argument("--wall-secs", type=int, default=30,
                    help="max seconds to hold at the wall waiting for a dropout")
    ap.add_argument("--out", default="captures/wax_deaf.bin")
    a = ap.parse_args()

    import os
    os.makedirs(os.path.dirname(a.out), exist_ok=True)

    print("=== reset + reset-cause capture ===")
    causes, banner, boot = reset_and_capture_cause(a.port)
    print(f"  banner_seen={banner}")
    for c in causes:
        print(f"  {c}")
    if not banner:
        print("  WARN: no rm32 banner — board may be stuck in bootloader DFU.")
        print("  (restore bootloader bin + power-cycle if this persists)")

    b = Bench(a.port)
    result = "unknown"
    try:
        b.dwell(0, 1.0)
        # Patient spin-up to 70% FIRST (BEMF lock), THEN arm WAXWING.
        # 'Z' is intentionally NOT sent: its binary 5B-A9 stream shares
        # PB6 with the WX text dump and corrupts the hex (A/B garbage).
        print("=== patient spin-up to 70% ===")
        if b.spinup(70) == "kill":
            result = "kill-on-spinup"
        else:
            b.cmd(b"J")   # arm injected burst -> WAXWING ring writes
            print(f"=== climb to wall {a.hold}% ===")
            if b.spinup(a.hold) == "kill":
                result = "kill-on-climb"
            else:
                print(f"=== wall hold {a.hold}% up to {a.wall_secs}s (await dropout) ===")
                result = b.dwell(a.hold, a.wall_secs)
    finally:
        try:
            b.rampdown()
        except Exception:
            pass
        b.kill()

    # Dump the frozen WAXWING ring (continuous read — one shot).
    print("=== WX dump ===")
    b.wx_dump(4.0)
    got_end = b"WX END" in bytes(b.raw)
    b.cmd(b"i", settle=0.3)
    b.p.close()
    print(f"  WX END captured: {got_end}")

    with open(a.out, "wb") as f:
        f.write(bytes(boot) + bytes(b.raw))
    print(f"result={result}  wrote {a.out} ({len(b.raw)} bytes)")
    print("decode: scripts/wax_decode.py " + a.out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
