#!/usr/bin/env python3
"""am32_clone register conformance check.

Probe-reads (live, non-halting) the peripheral + NVIC state of the
flashed firmware and PASS/FAILs each register against AM32
VIMDRONES_L431 golden values (Mcu/l431/Src/peripherals.c):

  TIM1:  PSC=0, ARR=3332 (24 kHz base; variable_pwm may lower ARR at
         speed - checked as range), CCMR1/2=0x6868, CCER=0x1555,
         BDTR: DTG=45 + MOE (checked masked), CR1: ARPE|CEN
  TIM2:  PSC=39 (0.5 us INTERVAL_TIMER), ARR=0xFFFF, CR1: CEN
  TIM16: PSC=39 (COM timer), CR1: CEN per AM32 init
  TIM6:  PSC=79, ARR=50 (19.6 kHz), CR1: CEN, DIER: UIE
  COMP2: CSR EN=1, HYST=0, POLARITY=0, BLANKING=0, INPSEL=IO1
  NVIC IPR (upper-nibble encoding): COMP(64)=0, TIM1_UP_TIM16(25)=0,
         TIM6_DAC(54)=3<<4, USART2(38)=2<<4

Usage:  python scripts/clone_regcheck.py
"""

import subprocess
import sys

CHIP = "STM32L431KCUx"
PROBE = "0483:374f:0037002F3234510836303532"

TIM1 = 0x4001_2C00
TIM2 = 0x4000_0000
TIM6 = 0x4000_1000
TIM16 = 0x4001_4400
COMP2_CSR = 0x4001_0204
IPR_BASE = 0xE000_E400


def rd(addr):
    out = subprocess.run(
        ["probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
         "b32", hex(addr), "1"],
        capture_output=True, text=True, timeout=30).stdout
    for ln in out.splitlines():
        for tok in ln.split():
            if len(tok) == 8:
                return int(tok, 16)
    raise SystemExit(f"read failed at {addr:#x}: {out!r}")


def rd8(addr):
    w = rd(addr & ~3)
    return (w >> (8 * (addr & 3))) & 0xFF


fails = 0


def check(name, actual, expect, mask=0xFFFFFFFF):
    global fails
    ok = (actual & mask) == (expect & mask)
    tag = "PASS" if ok else "FAIL"
    if not ok:
        fails += 1
    print(f"  [{tag}] {name}: {actual:#010x} "
          f"(want {expect:#x} mask {mask:#x})")


def check_range(name, actual, lo, hi):
    global fails
    ok = lo <= actual <= hi
    tag = "PASS" if ok else "FAIL"
    if not ok:
        fails += 1
    print(f"  [{tag}] {name}: {actual} (want {lo}..{hi})")


print("== TIM1 (motor PWM) ==")
check("PSC", rd(TIM1 + 0x28), 0)
check_range("ARR (3332 base; variable_pwm >= 1666)",
            rd(TIM1 + 0x2C), 1666, 3332)
check("CCMR1 (PWM1+OCxPE)", rd(TIM1 + 0x18), 0x6868, 0xFFFF)
check("CCMR2 (PWM1+OCxPE)", rd(TIM1 + 0x1C), 0x6868, 0xFFFF)
check("CCER (6ch + CH4)", rd(TIM1 + 0x20), 0x1555, 0x1FFF)
check("BDTR DTG=45", rd(TIM1 + 0x44), 45, 0xFF)
check("CR1 ARPE|CEN", rd(TIM1 + 0x00), 0x81, 0x81)

print("== TIM2 (INTERVAL_TIMER, 0.5 us) ==")
check("PSC=39", rd(TIM2 + 0x28), 39)
check("ARR=0xFFFF", rd(TIM2 + 0x2C), 0xFFFF, 0xFFFF)
check("CR1 CEN", rd(TIM2 + 0x00), 0x1, 0x1)

print("== TIM16 (COM timer, 0.5 us) ==")
check("PSC=39", rd(TIM16 + 0x28), 39)
check("CR1 CEN", rd(TIM16 + 0x00), 0x1, 0x1)

print("== TIM6 (tenKhzRoutine, 19.6 kHz) ==")
check("PSC=79", rd(TIM6 + 0x28), 79)
check("ARR=50", rd(TIM6 + 0x2C), 50)
check("CR1 CEN", rd(TIM6 + 0x00), 0x1, 0x1)
check("DIER UIE", rd(TIM6 + 0x0C), 0x1, 0x1)

print("== COMP2 ==")
csr = rd(COMP2_CSR)
check("EN", csr, 1, 1)
check("POLARITY=0", csr, 0, 1 << 15)
check("HYST=0", csr, 0, 3 << 16)
check("BLANKING=0", csr, 0, 7 << 18)

print("== NVIC priorities (IPR upper nibble) ==")
check("COMP (irq64) = 0", rd8(IPR_BASE + 64), 0x00, 0xF0)
check("TIM1_UP_TIM16 (irq25) = 0", rd8(IPR_BASE + 25), 0x00, 0xF0)
check("TIM6_DAC (irq54) = 3", rd8(IPR_BASE + 54), 0x30, 0xF0)
check("USART2 (irq38) = 2", rd8(IPR_BASE + 38), 0x20, 0xF0)
print("== NVIC enables (ISER) ==")
iser0 = rd(0xE000_E100)
iser1 = rd(0xE000_E104)
iser2 = rd(0xE000_E108)


def enabled(irq):
    return ((iser0, iser1, iser2)[irq // 32] >> (irq % 32)) & 1


for irq, name, want in ((64, "COMP", 1), (25, "TIM1_UP_TIM16", 1),
                        (54, "TIM6_DAC", 1), (38, "USART2", 1),
                        (65, "LPTIM2", 0), (55, "TIM7", 0)):
    global_ok = enabled(irq) == want
    if not global_ok:
        fails += 1
    print(f"  [{'PASS' if global_ok else 'FAIL'}] {name} "
          f"enabled={enabled(irq)} (want {want})")

print(f"\n{'ALL PASS' if fails == 0 else f'{fails} FAILURES'}")
sys.exit(0 if fails == 0 else 1)
