#!/usr/bin/env python3
"""Idle peripheral-register snapshot of the binz bench (NUCLEO-G071RB +
DRV8304H) over SWD, and a diff of two snapshots.

Stage-1 bring-up instrument: dump firmware50 at idle, flash rm32, dump
again, diff. Reads go through `probe-rs read` (no halt), but a read still
steals bus cycles — use it at IDLE only, never during a powered run.

    python scripts/binz_regdump.py dump --out captures/fw50_idle.txt
    python scripts/binz_regdump.py diff captures/fw50_idle.txt captures/rm32_idle.txt

Fails loudly: any block that cannot be read aborts the dump (no partial
snapshot that diffs as "equal").
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

CHIP = "STM32G071RBTx"
PROBE = "0483:374b:066CFF343433464757233430"

GPIO_REGS = ["MODER", "OTYPER", "OSPEEDR", "PUPDR", "IDR", "ODR", "BSRR", "LCKR", "AFRL", "AFRH"]
TIM1_REGS = [
    "CR1", "CR2", "SMCR", "DIER", "SR", "EGR", "CCMR1", "CCMR2", "CCER", "CNT",
    "PSC", "ARR", "RCR", "CCR1", "CCR2", "CCR3", "CCR4", "BDTR", "DCR", "DMAR",
    "OR1", "CCMR3", "CCR5", "CCR6", "AF1", "AF2",
]
BASIC_TIM = ["CR1", "CR2", "SMCR", "DIER", "SR", "EGR", "CCMR1", "CCMR2", "CCER", "CNT", "PSC", "ARR"]

# (block, base address, register names); one word per name.
BLOCKS: list[tuple[str, int, list[str]]] = [
    ("FLASH", 0x40022000, ["ACR"]),
    ("RCC", 0x40021000, ["CR", "ICSCR", "CFGR", "PLLCFGR"]),
    ("RCC_EN", 0x40021034, ["IOPENR", "AHBENR", "APBENR1", "APBENR2"]),
    ("GPIOA", 0x50000000, GPIO_REGS),
    ("GPIOB", 0x50000400, GPIO_REGS),
    ("GPIOC", 0x50000800, GPIO_REGS),
    ("GPIOD", 0x50000C00, GPIO_REGS),
    ("TIM1", 0x40012C00, TIM1_REGS),
    ("TIM2", 0x40000000, BASIC_TIM),
    ("TIM6", 0x40001000, BASIC_TIM),
    ("TIM14", 0x40002000, BASIC_TIM),
    ("TIM16", 0x40014400, BASIC_TIM),
    ("TIM17", 0x40014800, BASIC_TIM),
    ("COMP", 0x40010200, ["COMP1_CSR", "COMP2_CSR"]),
    ("EXTI", 0x40021800, ["RTSR1", "FTSR1", "SWIER1", "RPR1", "FPR1"]),
    ("EXTI_IMR", 0x40021880, ["IMR1", "EMR1"]),
    ("ADC", 0x40012400, ["ISR", "IER", "CR", "CFGR1", "CFGR2", "SMPR", "_18", "_1C", "AWD1TR", "AWD2TR", "CHSELR"]),
    ("ADC_CCR", 0x40012708, ["CCR"]),
    ("DMA1", 0x40020000, ["ISR", "IFCR"]
        + [f"C{c}{r}" for c in range(1, 6) for r in ("CR", "NDTR", "PAR", "MAR", "_RES")]),
    ("DMAMUX", 0x40020800, [f"C{c}CR" for c in range(0, 7)]),
    ("USART3", 0x40004800, ["CR1", "CR2", "CR3", "BRR"]),
    ("IWDG", 0x40003000, ["KR", "PR", "RLR", "SR", "WINR"]),
    ("NVIC_ISER", 0xE000E100, ["ISER"]),
    ("NVIC_IPR", 0xE000E400, [f"IPR{i}" for i in range(8)]),
]

# Volatile by nature: excluded from the diff verdict (still dumped).
VOLATILE = {
    "TIM1.CNT", "TIM2.CNT", "TIM6.CNT", "TIM14.CNT", "TIM16.CNT", "TIM17.CNT",
    "TIM6.SR", "TIM14.SR", "TIM16.SR", "TIM17.SR", "TIM2.SR",
    "GPIOA.IDR", "GPIOB.IDR", "GPIOC.IDR", "GPIOD.IDR",
    "GPIOA.BSRR", "GPIOB.BSRR", "GPIOC.BSRR", "GPIOD.BSRR",
    "ADC.ISR", "DMA1.ISR", "IWDG.KR", "EXTI.RPR1", "EXTI.FPR1",
}


def read_block(base: int, n: int) -> list[int]:
    out = subprocess.run(
        ["probe-rs", "read", "--chip", CHIP, "--probe", PROBE, "b32", hex(base), str(n)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if out.returncode != 0:
        raise SystemExit(f"probe-rs read {hex(base)} failed: {out.stderr.strip()}")
    words = out.stdout.split()[-n:]
    if len(words) != n:
        raise SystemExit(f"short read at {hex(base)}: {out.stdout!r}")
    return [int(w, 16) for w in words]


def dump(path: pathlib.Path) -> None:
    lines = []
    for name, base, regs in BLOCKS:
        vals = read_block(base, len(regs))
        for r, v in zip(regs, vals):
            if r.startswith("_"):
                continue
            lines.append(f"{name}.{r} 0x{v:08X}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")
    print(f"wrote {len(lines)} registers to {path}")


def load(path: pathlib.Path) -> dict[str, int]:
    d = {}
    for ln in path.read_text().splitlines():
        k, v = ln.split()
        d[k] = int(v, 16)
    return d


def diff(a: pathlib.Path, b: pathlib.Path) -> int:
    da, db = load(a), load(b)
    if set(da) != set(db):
        raise SystemExit(f"register sets differ: {sorted(set(da) ^ set(db))}")
    n = 0
    for k in da:
        if da[k] != db[k]:
            tag = " (volatile)" if k in VOLATILE else ""
            print(f"{k:20s} {a.stem}=0x{da[k]:08X}  {b.stem}=0x{db[k]:08X}  xor=0x{da[k] ^ db[k]:08X}{tag}")
            n += k not in VOLATILE
    print(f"{n} non-volatile differences of {len(da)} registers")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("dump")
    d.add_argument("--out", type=pathlib.Path, required=True)
    f = sub.add_parser("diff")
    f.add_argument("a", type=pathlib.Path)
    f.add_argument("b", type=pathlib.Path)
    args = ap.parse_args()
    if args.cmd == "dump":
        dump(args.out)
        return 0
    return diff(args.a, args.b)


if __name__ == "__main__":
    sys.exit(main())
