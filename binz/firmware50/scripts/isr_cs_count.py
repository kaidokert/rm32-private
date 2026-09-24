#!/usr/bin/env python3
"""Count interrupt-masking instructions in each motor ISR root (E130+).

For every root: the number of `mrs ..., primask`, `cpsid i` and
`msr primask, ...` instructions in its own body, plus any `bl` it makes (a
callee would hide more). With `--list`, each `cpsid` is printed with the
source line that produced it (from `objdump -S`), so every remaining critical
section can be named.

Usage: python scripts/isr_cs_count.py --elf ELF [--list]
"""

from __future__ import annotations

import argparse
import glob
import os
import re
import subprocess

ROOTS = ["ADC_COMP", "TIM16", "DMA1_CHANNEL1", "TIM6_DAC_LPTIM1"]


def objdump() -> str:
    hits = glob.glob(os.path.expanduser(r"~/.rustup/toolchains/*/lib/rustlib/*/bin/llvm-objdump*"))
    return hits[0]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args()
    for root in ROOTS:
        out = subprocess.run(
            [objdump(), "-d", "-S", "--no-show-raw-insn", f"--disassemble-symbols={root}", a.elf],
            capture_output=True, text=True,
        ).stdout.splitlines()
        insns = [l for l in out if re.match(r"\s*[0-9a-f]+:\s", l)]
        mrs = sum(1 for l in insns if re.search(r"\bmrs\s+r\d+, primask", l))
        cps = sum(1 for l in insns if re.search(r"\bcpsid\s+i", l))
        msr = sum(1 for l in insns if re.search(r"\bmsr\s+primask", l))
        bls = sum(1 for l in insns if re.search(r"\sbl\s", l))
        print(f"{root:<16} insns={len(insns):4d} mrs={mrs:3d} cpsid={cps:3d} msr={msr:3d} bl={bls}")
        if a.list:
            src: list[str] = []
            for l in out:
                if l.startswith(";"):
                    src.append(l[1:].strip())
                    continue
                if re.search(r"\bcpsid\s+i", l):
                    # the nearest preceding source lines that are not cortex-m internals
                    ctx = [s for s in src if s and "cortex-m" not in s and "cortex_m" not in s and "asm!" not in s]
                    print("   cpsid <-", " | ".join(ctx[-3:])[:200])
                if re.match(r"\s*[0-9a-f]+:\s", l):
                    src = src[-6:]
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
