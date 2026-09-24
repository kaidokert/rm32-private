#!/usr/bin/env python3
"""Sample the live drive registers over SWD while a powered run holds.

Notebook E096: compare what the oracle and firmware50 actually program into
TIM1 (PWM mode, output enables, compares, dead time), COMP2 and the EXTI edge
selection during a closed-loop hold. `probe-rs read` reads through the debug
port without halting the core (checked at idle first, E096), so the run under
test is not disturbed beyond the bus access itself.

Usage:
    python scripts/sample_regs.py --delay 14 --count 20 --out captures/<day>/<label>.txt
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import time

CHIP = "STM32G071RBTx"
PROBE = "0483:374b:066CFF343433464757233430"

# (label, address, words)
BLOCKS = [
    ("TIM1", 0x40012C00, 26),  # CR1 .. AF2
    ("COMP", 0x40010200, 2),  # COMP1_CSR, COMP2_CSR
    ("EXTI_RF", 0x40021800, 2),  # RTSR1, FTSR1
    ("EXTI_IMR1", 0x40021880, 1),
]

TIM1_NAMES = [
    "CR1", "CR2", "SMCR", "DIER", "SR", "EGR", "CCMR1", "CCMR2", "CCER", "CNT",
    "PSC", "ARR", "RCR", "CCR1", "CCR2", "CCR3", "CCR4", "BDTR", "DCR", "DMAR",
    "OR1", "CCMR3", "CCR5", "CCR6", "AF1", "AF2",
]


def read(addr: int, words: int) -> list[str]:
    out = subprocess.run(
        ["probe-rs", "read", "--chip", CHIP, "--probe", PROBE, "b32", hex(addr), str(words)],
        capture_output=True,
        text=True,
    )
    return out.stdout.split()[-words:] if out.returncode == 0 else ["ERR"] * words


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--delay", type=float, default=14.0)
    ap.add_argument("--count", type=int, default=20)
    ap.add_argument("--out", type=pathlib.Path, required=True)
    args = ap.parse_args()
    time.sleep(args.delay)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", encoding="utf-8", newline="\n") as fh:
        for i in range(args.count):
            t = time.monotonic()
            row = []
            for label, addr, words in BLOCKS:
                vals = read(addr, words)
                if label == "TIM1":
                    row += [f"{n}={v}" for n, v in zip(TIM1_NAMES, vals)]
                else:
                    row += [f"{label}{k}={v}" for k, v in enumerate(vals)]
            fh.write(f"REGS i={i} t={t:.2f} " + " ".join(row) + "\n")
            fh.flush()
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
