#!/usr/bin/env python3
"""Time the boot sequence from the bench UART (gc S2.2: startup-tune length).

    python scripts/boot_timing.py --board l431 [--runs 3]

Resets the chip with probe-rs (no flashing), timestamps every boot line on
the host, and prints the reset -> line offsets plus the startup tune length
(`led done` -> `tone done`). The motor is energised only as a buzzer during
the tune; nothing is armed (no throttle is sent).
"""

from __future__ import annotations

import argparse
import statistics
import subprocess
import sys
import time

import serial

BOARDS = {
    "l431": {"port": "COM18", "baud": 2_000_000, "chip": "STM32L431KCUx",
             "probe": "0483:374f:0037002F3234510836303532"},
    "binz": {"port": "COM41", "baud": 115_200, "chip": "STM32G071RBTx",
             "probe": "0483:374b:066CFF343433464757233430"},
}
MARKS = ("[rm32] boot", "[rm32] led done", "[rm32] tone done", "[rm32] wdg ON")


def one(b: dict, secs: float) -> dict[str, float]:
    with serial.Serial(b["port"], b["baud"], timeout=0.05) as s:
        s.reset_input_buffer()
        subprocess.run(["probe-rs", "reset", "--chip", b["chip"], "--probe", b["probe"]],
                       check=True, capture_output=True)
        t0 = time.time()
        seen: dict[str, float] = {}
        buf = b""
        while time.time() - t0 < secs:
            buf += s.read(4096)
            *lines, buf = buf.split(b"\n")
            now = time.time() - t0
            for ln in lines:
                txt = ln.decode(errors="replace").strip()
                for m in MARKS:
                    if txt.startswith(m) and m not in seen:
                        seen[m] = now
    return seen


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--board", choices=sorted(BOARDS), required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--secs", type=float, default=6.0)
    a = ap.parse_args()
    tunes = []
    for k in range(a.runs):
        seen = one(BOARDS[a.board], a.secs)
        print(f"run {k + 1}: " + "  ".join(f"{m.split('] ')[1]}={seen[m]:.3f}s"
                                           for m in MARKS if m in seen))
        missing = [m for m in MARKS if m not in seen]
        if missing:
            print(f"  missing marks: {missing}")
            return 1
        tunes.append(seen[MARKS[2]] - seen[MARKS[1]])
    print(f"startup tune: {' / '.join(f'{t * 1000:.0f}' for t in tunes)} ms "
          f"(median {statistics.median(tunes) * 1000:.0f} ms)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
