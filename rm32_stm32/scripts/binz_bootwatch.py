#!/usr/bin/env python3
"""binz boot watcher: reset the G071 over SWD, listen on the bench UART for
--secs, and classify the boot from the raw bytes (byte-safe: the line carries
pre-boot garbage that breaks text consoles on Windows).

    python scripts/binz_bootwatch.py --secs 8

Prints the count of main-loop entries (`irqs enabled`), watchdog resets
(`last reset: indep-watchdog`) and boot banners, the last printed `[...]`
line before each reset, and a verdict: BOOTS (one banner, main loop
entered, no watchdog reset) or LOOP. Exit status 0 = BOOTS. No drive
command is ever sent.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import threading
import time

import serial

PROBE = "0483:374b:066CFF343433464757233430"
CHIP = "STM32G071RBTx"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--secs", type=float, default=8.0)
    a = ap.parse_args()
    s = serial.Serial(a.port, 115200, timeout=0.05)
    s.read(100000)
    buf = bytearray()
    stop = threading.Event()

    def rx() -> None:
        while not stop.is_set():
            buf.extend(s.read(4096))

    t = threading.Thread(target=rx, daemon=True)
    t.start()
    time.sleep(0.3)
    subprocess.run(["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE], check=True)
    time.sleep(a.secs)
    stop.set()
    t.join()
    s.close()
    txt = buf.decode("latin-1")
    banners = txt.count("[rm32] init done")
    entered = txt.count("irqs enabled, entering main loop")
    wdg = txt.count("last reset: indep-watchdog")
    print(f"banners={banners} main_loop_entries={entered} watchdog_resets={wdg}")
    # The last bracketed line printed before each subsequent banner.
    parts = txt.split("[rm32] init done")
    for i, p in enumerate(parts[:-1][1:4], 1):
        lines = re.findall(r"\[[a-z0-9]+\][^\n\r]*", p)
        if lines:
            print(f"  boot {i} last line: {lines[-1][:100]}")
    ok = banners == 1 and entered >= 1 and wdg == 0
    print("BOOTS" if ok else "LOOP")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
