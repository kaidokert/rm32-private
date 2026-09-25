"""Instruction-class ratchet for the ISR roots AND the foreground hot path.

Why this exists (E303). E291 fixed a real overflow in `BusDepth::observe` by
scaling both sides of a cross-product by 1/5, written inline as
`rhs * (self.fracs[i] / 5)`. `fracs` is a struct field, so the compiler could
not fold it: on thumbv6m it lowered to `bl __aeabi_uidiv` **inside** the
four-iteration loop, and `scan_pass` calls `observe` twice per scan -- eight
soft divisions per ADC scan at ~9.9 kHz, in the foreground that hosts every bus
judgement. Measured: `loop_iters_closed` fell 48% and the worst foreground gap
grew 49%.

**Nothing in the tree could see it.**

* `isr_diff.py` compares the four ISR roots and says so in its own docstring:
  thread mode is invisible to it. `BusDepth::observe` is thread mode.
* `isr_audit.py` and `WCET_ESTIMATES.md` cover ISR roots only.
* I read the disassembly twice, stopped at `movs r1, #5` without following the
  `bl` two instructions later, and reported the fix as verified.

So: count instruction CLASSES per symbol and refuse when they move. A division
appearing anywhere in a hot path becomes a build failure rather than something
a reader has to notice. Per [[feedback-instrument-must-fail-loudly]] the check
cannot pass by landing on the expected answer, because it asserts a baseline it
did not compute from the image under test.

Classes counted, and why each:

* `div`    -- `__aeabi_uidiv`/`idiv`/`uidivmod`/`idivmod`. No hardware divide on
              M0+, so each is a call of tens of cycles. The E303 class.
* `mul`    -- `muls` and `__aeabi_lmul`. Cheap but not free, and a widening
              multiply where a narrow one was is a silent cost
              ([[reference-m0-optlevel-s-softdiv]]).
* `irq`    -- `cpsid`/`cpsie`. Critical sections: a new one in a root or in the
              scan path changes the preemption structure, which is what the
              atomic stop/arm work exists to protect.
* `excl`   -- `ldrex`/`strex`. Should never appear on this part.
* `helper` -- any `bl` to an `__aeabi_*` or `mem*` symbol.
* `insns`  -- total, as a coarse drift signal with a little slack.

Usage:
    python scripts/insn_ratchet.py --elf path/to.elf
    python scripts/insn_ratchet.py --elf path/to.elf --bless
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess

BASELINE = pathlib.Path(__file__).parent.parent / "captures" / "insn_baseline.json"

# The four motor ISR roots, plus the foreground symbols `isr_diff.py`
# structurally cannot see. The scan path is here because that is exactly where a
# per-scan cost landed with no existing gate noticing.
# The four ISR roots are reached through their VECTOR symbols, not their Rust
# names: LTO inlines `roots::comp_root` and friends into the `#[interrupt]`
# shims, so only `ADC_COMP` / `TIM16` / `TIM6_DAC_LPTIM1` / `DMA1_CHANNEL1`
# survive -- the same four `isr_diff.py` uses. Most protection helpers are
# inlined too; `BusDepth::observe` is the one that stays a symbol, and it is the
# one E303 was about.
WATCHED = (
    "ADC_COMP",
    "TIM16",
    "TIM6_DAC_LPTIM1",
    "DMA1_CHANNEL1",
    "protection::BusDepth::observe",
)

# A thin match is a silent pass: the first version of this script found 1 of 8
# symbols and cheerfully blessed a one-symbol baseline. Refuse instead.
MIN_SYMBOLS = 5

CLASSES = ("insns", "div", "mul", "irq", "excl", "helper")

# **This toolchain does not emit `__aeabi_uidiv`.** It calls
# `compiler_builtins::int::specialized_div_rem::u32_div_rem` (and the u64
# variant). The first version of this regex matched only the `__aeabi_*` names
# and would therefore have missed the exact defect the script exists to catch --
# a check that cannot see its own motivating bug.
DIV = re.compile(r"__aeabi_u?idiv(mod)?|u(32|64)_div_rem|__udivsi3|__divsi3")
MUL = re.compile(r"\bmuls?\b|__aeabi_lmul")
IRQ = re.compile(r"\bcps(id|ie)\b")
EXCL = re.compile(r"\b(ldrex|strex)\b")
HELPER = re.compile(r"\bbl\b.*(__aeabi_|memcpy|memset|memmove)")
BODY = re.compile(r"^\s+[0-9a-f]+:\s")


def text_symbols(elf: pathlib.Path) -> list[tuple[int, str]]:
    out = subprocess.run(["arm-none-eabi-nm", "-C", str(elf)],
                         capture_output=True, text=True, check=False).stdout
    got: list[tuple[int, str]] = []
    for line in out.splitlines():
        parts = line.split(" ", 2)
        if len(parts) == 3 and parts[1] in ("t", "T"):
            try:
                got.append((int(parts[0], 16), parts[2].strip()))
            except ValueError:
                pass
    return sorted(got)


def count(elf: pathlib.Path) -> dict[str, dict[str, int]]:
    syms = text_symbols(elf)
    addrs = sorted(a for a, _ in syms)
    result: dict[str, dict[str, int]] = {}
    for want in WATCHED:
        hit = next(((a, n) for a, n in syms if want in n), None)
        if hit is None:
            continue
        addr, _name = hit
        nxt = next((a for a in addrs if a > addr), addr + 512)
        end = min(nxt, addr + 4096)
        out = subprocess.run(
            ["arm-none-eabi-objdump", "-d",
             f"--start-address=0x{addr:08x}", f"--stop-address=0x{end:08x}",
             str(elf)],
            capture_output=True, text=True, check=False).stdout
        body = [l for l in out.splitlines() if BODY.match(l)]
        result[want] = {
            "insns": len(body),
            "div": sum(1 for l in body if DIV.search(l)),
            "mul": sum(1 for l in body if MUL.search(l)),
            "irq": sum(1 for l in body if IRQ.search(l)),
            "excl": sum(1 for l in body if EXCL.search(l)),
            "helper": sum(1 for l in body if HELPER.search(l)),
        }
    return result


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--bless", action="store_true",
                    help="write current counts as the baseline; only with a "
                         "stated reason in the notebook")
    ap.add_argument("--insn-slack", type=int, default=8,
                    help="allowed total-instruction drift per symbol "
                         "(default 8); the hazard classes get NO slack")
    args = ap.parse_args()

    elf = pathlib.Path(args.elf)
    if not elf.exists():
        print(f"no such ELF: {elf}")
        return 2
    now = count(elf)
    if len(now) < MIN_SYMBOLS:
        print(f"REFUSED: found {len(now)} of {len(WATCHED)} watched symbols "
              f"(need >= {MIN_SYMBOLS}).")
        print("  Either this is the wrong ELF, or a symbol was renamed or "
              "inlined and the ratchet would be silently measuring almost "
              "nothing.")
        print("  Found: " + ", ".join(sorted(now)))
        return 2

    if args.bless or not BASELINE.exists():
        BASELINE.parent.mkdir(parents=True, exist_ok=True)
        BASELINE.write_text(json.dumps(now, indent=1, sort_keys=True) + "\n",
                            encoding="utf-8")
        verb = "blessed" if args.bless else "created"
        print(f"{verb} baseline from {elf.name}: {len(now)} symbols")
        for k, v in sorted(now.items()):
            print(f"  {k:<44} {v}")
        return 0

    base = json.loads(BASELINE.read_text(encoding="utf-8"))
    fails: list[str] = []
    print(f"{'symbol':<44}{'insns':>12}{'div':>7}{'mul':>7}"
          f"{'irq':>7}{'excl':>7}{'helper':>8}")
    for k in sorted(set(base) | set(now)):
        b, n = base.get(k), now.get(k)
        if n is None:
            fails.append(f"{k}: in the baseline, ABSENT from this ELF")
            continue
        if b is None:
            fails.append(f"{k}: in this ELF but NOT the baseline (bless to accept)")
            print(f"{k:<44}{'(new)':>12}")
            continue
        cells = []
        for c in CLASSES:
            d = n[c] - b[c]
            if c == "insns":
                if abs(d) > args.insn_slack:
                    fails.append(f"{k}: insns {b[c]} -> {n[c]} ({d:+d}), "
                                 f"beyond +-{args.insn_slack}")
            elif d != 0:
                # Hazard classes ratchet at zero drift in BOTH directions: a
                # count that fell is also a changed hot path and wants a reason.
                fails.append(f"{k}: {c} {b[c]} -> {n[c]} ({d:+d})")
            cells.append(f"{n[c]}" + ("" if d == 0 else f" ({d:+d})"))
        print(f"{k:<44}{cells[0]:>12}{cells[1]:>7}{cells[2]:>7}"
              f"{cells[3]:>7}{cells[4]:>7}{cells[5]:>8}")

    if fails:
        print(f"\nRATCHET FAILED ({len(fails)}):")
        for f in fails:
            print(f"  {f}")
        print("\nA hazard-class change is a build failure, not a note. If it is "
              "intended,\nsay why in the notebook and re-run with --bless.")
        return 1
    print("\nratchet OK: no hazard-class drift in any watched symbol")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
