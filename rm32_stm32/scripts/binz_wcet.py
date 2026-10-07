#!/usr/bin/env python3
"""binz envelope-climb WCET gate: G071 ISR roots of a linked rm32 ELF.

For every motor ISR root this prints
  * the static instruction count of its call tree (all paths), next to AM32's
    G071 reference (binz/firmware50/isr_ref/am32_g071/call_trees.txt), and the
    goal's <= 2x ratio;
  * the longest-path cycle bound from scripts/isr_cycles.py (firmware50's
    method) at 0 wait states and with the 2-wait-state fetch model;
  * for a rung's eHz: COM + COMP worst case against 50 % of one commutation
    step (1 / (6 * eHz)).

Excluded paths, each listed so it is reviewed rather than hidden:
  * panic machinery (never returns: the IWDG resets the chip);
  * `take_isr_state` — the one-time lazy move of the ISR state into its
    static on the very first ISR entry (the boot TIM6 tick).

Reviewed loop bounds (`BOUNDS`) — every one has its source reason. A new loop
makes isr_cycles refuse (fail-closed); review it and add it here.

    python scripts/binz_wcet.py --elf target/thumbv6m-none-eabi/release/rm32_firmware --ehz 735
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
OBJDUMP = (
    r"C:\Users\kaido\.rustup\toolchains\1.80-x86_64-pc-windows-msvc"
    r"\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-objdump.exe"
)

TERMINAL = [
    "panic", "rust_begin_unwind", "unwrap_failed", "expect_failed",
    "assert_failed", "slice_index", "slice_end_index", "slice_start_index",
    "take_isr_state",
]

# symbol substring -> loop trip bounds (in back-edge address order)
BOUNDS = {
    # rm32::commutation::Commutation::record_interval: sum of the 6-slot ring.
    "record_interval": "6",
    # isr_logic::bemf_zero_cross: persistence filter, filter_level <= 12
    # (AM32 map(avg, 100, 500, 3, 12); 12 at zero_crosses < 100).
    "bemf_zero_cross": "12",
    # dshot::erpm_to_12bit: exponent search i = 15..9 (7 trips).
    "erpm_to_12bit": "7",
    # dshot::encode_gcr_frame: nibble checksum (3) then 20 GCR bits.
    "encode_gcr_frame": "3,20",
    # SharedState fetch_update CAS loops: M0 critical-section CAS cannot fail,
    # so 1 trip; 2 for margin.
    "SharedState": "2",
    # compiler_builtins::mem::memcpy from tone::note_for: a 6-byte Note copy;
    # no loop can exceed 6 trips. (Tones abort while the motor runs.)
    "mem6memcpy": "6,6,6,6",
}

# Per-root overrides. COM (TIM14) and COMP run at NVIC priority 0 and cannot
# preempt each other, so nothing can write a SharedState atomic between a
# fetch_update's load and its critical-section CAS: the CAS cannot fail and
# each loop runs exactly once. TIM6 (priority 2) is preempted by COM/COMP
# writes, so it keeps the retry bound of 2.
ROOT_BOUNDS = {
    "TIM14": {"SharedState": "1"},
    "ADC_COMP": {"SharedState": "1"},
}

# AM32 G071 static instruction counts (call_trees.txt, sha 02592720...).
AM32 = {"ADC_COMP": 131, "TIM14": 674, "TIM6_DAC_LPTIM1": 1637}
ROOTS = ["TIM14", "ADC_COMP", "TIM6_DAC_LPTIM1", "DMA1_CHANNEL1"]
PRIO = {"TIM14": 0, "ADC_COMP": 0, "DMA1_CHANNEL1": 1, "TIM6_DAC_LPTIM1": 2}


def load(elf: str) -> dict[str, list[str]]:
    out = subprocess.run([OBJDUMP, "-d", elf], capture_output=True, text=True).stdout
    funcs: dict[str, list[str]] = {}
    cur = None
    for ln in out.splitlines():
        m = re.match(r"^([0-9a-f]+) <(.+)>:$", ln)
        if m:
            cur = m.group(2)
            funcs[cur] = []
        elif cur and re.match(r"^\s*[0-9a-f]+:\s+[0-9a-f]{4}", ln) and ".word" not in ln \
                and ".short" not in ln:
            funcs[cur].append(ln)
    return funcs


INSN = re.compile(r"^\s*([0-9a-f]+):\s+(?:[0-9a-f]{2,4}\s?){1,2}\s+(\S+)\s*(.*)$")
COND = {"eq", "ne", "cs", "hs", "cc", "lo", "mi", "pl", "vs", "vc",
        "hi", "ls", "ge", "lt", "gt", "le"}


def is_terminal(name: str) -> bool:
    return any(t in name for t in TERMINAL)


def reachable(lines: list[str]) -> list[str]:
    """Instructions reachable from the function entry when a BL into a
    terminal callee ends the path (same rule as isr_cycles --terminal): code
    only reached after the lazy-init move or a panic is not counted."""
    insns = []
    for ln in lines:
        m = INSN.match(ln)
        if m:
            insns.append((int(m.group(1), 16), m.group(2).lower(), m.group(3), ln))
    if any(mn == "add" and ops.startswith("pc") for _, mn, ops, _ in insns):
        return [ln for *_, ln in insns]  # jump table: count everything
    ix = {a: i for i, (a, _, _, _) in enumerate(insns)}
    succ: list[list[int]] = [[] for _ in insns]
    for i, (a, mn, ops, _) in enumerate(insns):
        nxt = [i + 1] if i + 1 < len(insns) else []
        t = re.search(r"0x([0-9a-f]+)", ops)
        tgt = ix.get(int(t.group(1), 16)) if t else None
        if mn in ("b", "b.n", "b.w"):
            succ[i] = [tgt] if tgt is not None else []
        elif mn.startswith("b") and mn[1:3] in COND and len(mn) <= 5:
            succ[i] = nxt + ([tgt] if tgt is not None else [])
        elif (mn == "pop" and "pc" in ops) or mn == "bx":
            succ[i] = []
        elif mn == "bl":
            c = re.search(r"<([^>+]+)", ops)
            succ[i] = [] if (c and is_terminal(c.group(1))) else nxt
        else:
            succ[i] = nxt
    seen = {0} if insns else set()
    stack = [0] if insns else []
    while stack:
        u = stack.pop()
        for v in succ[u]:
            if v not in seen:
                seen.add(v)
                stack.append(v)
    return [insns[i][3] for i in sorted(seen)]


def tree(funcs: dict[str, list[str]], root: str) -> dict[str, list[str]]:
    """Reachable call tree: function -> its reachable instructions."""
    seen: dict[str, list[str]] = {}
    stack = [root]
    while stack:
        f = stack.pop()
        if f in seen or is_terminal(f):
            continue
        body = reachable(funcs.get(f, []))
        seen[f] = body
        for ln in body:
            m = re.search(r"\bbl\s+\S+\s+<([^>+]+)", ln)
            if m:
                stack.append(m.group(1))
    return seen


def filter_level(ehz: float) -> int:
    """AM32 filter_level at a rung (main.c: map(average_interval, 100, 500,
    3, 12), 2 when commutation_interval < 50); average_interval in 2 MHz
    interval-timer counts per step. firmware50's WCET method: the
    persistence-filter read count follows speed."""
    avg = 2e6 / (6 * ehz)
    if avg < 50:
        return 2
    x = min(max(avg, 100), 500)
    return int(3 + (x - 100) * 9 / 400)


def cycles(elf: str, root: str, fetch: bool, filt: int | None = None) -> int:
    cmd = [sys.executable, str(HERE / "isr_cycles.py"), "--elf", elf, "--root", root]
    for t in TERMINAL:
        cmd += ["--terminal", t]
    bounds = dict(BOUNDS)
    bounds.update(ROOT_BOUNDS.get(root, {}))
    if filt is not None:
        bounds["bemf_zero_cross"] = str(filt)
    # isr_cycles takes the FIRST matching --callee-bound: overrides first.
    for sym, b in sorted(bounds.items(), key=lambda kv: kv[0] not in ROOT_BOUNDS.get(root, {})):
        cmd += ["--callee-bound", f"{sym}={b}"]
    if fetch:
        cmd += ["--fetch-model", "--per-insn", "0.25", "--loop-hit"]
    out = subprocess.run(cmd, capture_output=True, text=True).stdout
    m = re.search(r"longest path\s+(\d+)", out)
    if not m:
        raise SystemExit(f"{root}: isr_cycles refused:\n{out}")
    return int(m.group(1))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--ehz", type=float, action="append", default=[],
                    help="rung eHz for the COM+COMP budget line (repeatable)")
    args = ap.parse_args()
    funcs = load(args.elf)
    res = {}
    print(f"{'root':16} {'prio':>4} {'insns':>6} {'AM32':>6} {'ratio':>6} "
          f"{'cyc@0ws':>8} {'cyc@2ws':>8} {'us@2ws':>7}")
    for r in ROOTS:
        n = sum(len(body) for body in tree(funcs, r).values())
        c0, c2 = cycles(args.elf, r, False), cycles(args.elf, r, True)
        res[r] = c2
        a = AM32.get(r)
        ratio = f"{n / a:.2f}" if a else "-"
        print(f"{r:16} {PRIO[r]:>4} {n:>6} {a or '-':>6} {ratio:>6} {c0:>8} {c2:>8} "
              f"{c2 / 64:>7.1f}")
    for ehz in args.ehz:
        filt = filter_level(ehz)
        comp = cycles(args.elf, "ADC_COMP", True, filt)
        crit = res["TIM14"] + comp
        step_us = 1e6 / (6 * ehz)
        pct = 100 * (crit / 64) / step_us
        print(f"rung {ehz:.0f} eHz: step {step_us:.1f} us, COM {res['TIM14'] / 64:.1f} + "
              f"COMP {comp / 64:.1f} us (filter {filt}) = {crit / 64:.1f} us "
              f"= {pct:.0f} % of step -> {'PASS' if pct < 50 else 'FAIL'} (< 50 %)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
