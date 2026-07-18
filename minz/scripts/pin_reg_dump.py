"""Dump the pin/PWM/COMP register state relevant to the phase-B
asymmetry question: GPIOA/GPIOB config + TIM1 + COMP1/2. Motor must
be OFF (probe-rs read halts the core briefly).

Usage: python scripts/pin_reg_dump.py --tag minz
Diff two dumps:  fc captures\\pins_minz.txt captures\\pins_am32.txt
"""

import argparse
import pathlib
import subprocess

PROBE = "0483:374f:0037002F3234510836303532"
CHIP = "STM32L431KCUx"

REGS = [
    ("GPIOA.MODER", 0x48000000), ("GPIOA.OTYPER", 0x48000004),
    ("GPIOA.OSPEEDR", 0x48000008), ("GPIOA.PUPDR", 0x4800000C),
    ("GPIOA.AFRL", 0x48000020), ("GPIOA.AFRH", 0x48000024),
    ("GPIOB.MODER", 0x48000400), ("GPIOB.OTYPER", 0x48000404),
    ("GPIOB.OSPEEDR", 0x48000408), ("GPIOB.PUPDR", 0x4800040C),
    ("GPIOB.AFRL", 0x48000420), ("GPIOB.AFRH", 0x48000424),
    ("TIM1.CR1", 0x40012C00), ("TIM1.CR2", 0x40012C04),
    ("TIM1.CCMR1", 0x40012C18), ("TIM1.CCMR2", 0x40012C1C),
    ("TIM1.CCER", 0x40012C20), ("TIM1.PSC", 0x40012C28),
    ("TIM1.ARR", 0x40012C2C), ("TIM1.BDTR", 0x40012C44),
    ("COMP1.CSR", 0x40010200), ("COMP2.CSR", 0x40010204),
]


def rd(addr):
    out = subprocess.run(
        ["probe-rs", "read", "--chip", CHIP, "--probe", PROBE,
         "b32", f"0x{addr:08x}", "1"],
        capture_output=True, text=True, check=True).stdout
    for tok in out.split():
        try:
            return int(tok, 16)
        except ValueError:
            pass
    return None


def pin2(val, pin):
    return (val >> (pin * 2)) & 3


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tag", required=True)
    a = ap.parse_args()
    lines = []
    vals = {}
    for name, addr in REGS:
        v = rd(addr)
        vals[name] = v
        lines.append(f"{name:14s} = {v:08x}")
    # decoded per-pin view for the six motor pins + comp inputs
    lines.append("-- per-pin (MODER/OSPEEDR/PUPDR, 2-bit fields) --")
    for label, port, pin in [
        ("PA7  A-lo", "GPIOA", 7), ("PA8  A-hi", "GPIOA", 8),
        ("PA9  B-hi", "GPIOA", 9), ("PB0  B-lo", "GPIOB", 0),
        ("PA10 C-hi", "GPIOA", 10), ("PB1  C-lo", "GPIOB", 1),
        ("PA4  cmpA", "GPIOA", 4), ("PA5  cmpB", "GPIOA", 5),
        ("PB7  cmpC", "GPIOB", 7), ("PB4  neut", "GPIOB", 4),
    ]:
        m = pin2(vals[f"{port}.MODER"], pin)
        s = pin2(vals[f"{port}.OSPEEDR"], pin)
        pu = pin2(vals[f"{port}.PUPDR"], pin)
        afr = vals[f"{port}.AFRL"] if pin < 8 else vals[f"{port}.AFRH"]
        af = (afr >> ((pin % 8) * 4)) & 0xF
        lines.append(f"{label}: MODER={m} OSPEED={s} PUPD={pu} AF={af}")
    out = pathlib.Path("captures") / f"pins_{a.tag}.txt"
    out.write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    print(f"-> {out}")


if __name__ == "__main__":
    main()
