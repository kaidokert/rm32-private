#!/usr/bin/env python3
"""Fail-closed soft-arithmetic / unbounded-loop audit for the motor ISR roots.

The M0 has no hardware divider, so a `u32` division inside a motor interrupt
compiles to an `__aeabi_*` libgcc helper: slow and data-dependent. This audit
walks the *reachable* call graph from each motor ISR root in a linked ELF and
fails if any forbidden helper is reachable.

It is deliberately fail-closed:

  * a reachable forbidden arithmetic helper            -> FAIL
  * a reachable indirect call (`blx rN`) not allowlisted -> FAIL
      (an unresolved indirect call defeats reachability analysis, so we refuse
       to certify the root rather than silently under-report)
  * a reachable function containing a backward branch (a loop) whose symbol is
    not in the reviewed-loops allowlist                -> FAIL
      (every loop in interrupt context must be consciously bounded; new ones
       must be reviewed and named, not discovered in the field)

Usage:
    python isr_audit.py --elf path/to/shell-pwm \\
        --root COMP --root COM --root DMA --root GUARD \\
        [--allow-file audit_allow.json] [--objdump arm-none-eabi-objdump]

    python isr_audit.py --elf path/to/elf --list-vectors     # discover roots

Allowlist file (JSON), both keys optional:
    {"indirect_calls": ["sym"], "reviewed_loops": ["sym"]}

Exit status: 0 = every root certified, 1 = violation, 2 = usage/tool error.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import deque

# libgcc/compiler-rt arithmetic helpers that must never be reachable from a
# motor ISR root on this target.
FORBIDDEN = {
    "__aeabi_uidiv", "__aeabi_uidivmod", "__aeabi_idiv", "__aeabi_idivmod",
    "__aeabi_ldivmod", "__aeabi_uldivmod",
    "__udivsi3", "__divsi3", "__umodsi3", "__modsi3",
    "__udivdi3", "__divdi3", "__umoddi3", "__moddi3",
    "__aeabi_fdiv", "__aeabi_ddiv", "__aeabi_dmul", "__aeabi_fmul",
    "__divdf3", "__muldf3", "__divsf3",
}

# Panic machinery: reaching this from an interrupt root is its own hazard
# (unbounded formatting work / abort in ISR context), reported separately.
PANIC = {
    "core::panicking::panic_fmt", "core::panicking::panic",
    "core::panicking::panic_bounds_check", "core::panicking::panic_nounwind_fmt",
    "__rustc::rust_begin_unwind", "rust_begin_unwind",
}

# `<sym+0x..>` / `<sym>` at end of a disassembly line.
# NOTE: Rust trait-impl symbols nest angle brackets, e.g.
#   bl 8002ce4 <<foo::Output as bar::PhaseOutput>::com_step>
# so the capture must be GREEDY to the final '>'; a `[^>]` class silently
# fails on every such call and misreports it as a register-indirect call.
TARGET_RE = re.compile(r"<(.+)>\s*$")
OFFSET_RE = re.compile(r"\+0x[0-9a-f]+$")


def target_symbol(rest: str) -> str | None:
    """Extract the callee symbol from the tail of a disassembly line."""
    m = TARGET_RE.search(rest)
    if not m:
        return None
    return OFFSET_RE.sub("", m.group(1))
# "0800351c <symbol>:" section/function header.
HEADER_RE = re.compile(r"^([0-9a-f]+)\s+<(.+)>:\s*$")
# instruction line: "  8003520:\t4770      \tbx\tlr"
INSN_RE = re.compile(r"^\s*([0-9a-f]+):\s+(?:(?:[0-9a-f]{8}|[0-9a-f]{4}|[0-9a-f]{2})\s+)+\t?\s*(\S+)\s*(.*)$")

DIRECT_CALLS = {"bl", "blx"}          # blx <label> is direct; blx rN is not
BRANCHES = {"b", "b.n", "b.w", "bx", "beq", "bne", "bcs", "bcc", "bmi", "bpl",
            "bvs", "bvc", "bhi", "bls", "bge", "blt", "bgt", "ble", "bal",
            "cbz", "cbnz",
            # llvm-objdump spells the carry conditions `bhs`/`blo`, not
            # `bcs`/`bcc`. Without these a loop closed by an unsigned compare
            # was invisible to the audit (found while adding the E076
            # reachability test).
            "bhs", "blo"}


def objdump_all(objdump: str, elf: str) -> str:
    try:
        out = subprocess.run(
            [objdump, "-d", "-w", "--demangle", elf],
            capture_output=True, text=True, check=True,
        )
    except FileNotFoundError:
        sys.exit(f"error: objdump not found: {objdump}")
    except subprocess.CalledProcessError as exc:
        sys.exit(f"error: objdump failed: {exc.stderr.strip()[:400]}")
    return out.stdout


class Func:
    __slots__ = ("name", "addr", "calls", "indirect", "has_backward_branch", "insns", "cfg")

    def __init__(self, name: str, addr: int) -> None:
        self.name = name
        self.addr = addr
        self.calls: set[str] = set()
        self.indirect = False
        self.has_backward_branch = False
        self.insns = 0
        # (address, base mnemonic, in-function branch destination or None,
        #  operand text) per instruction, for the loop test below.
        self.cfg: list[tuple[int, str, int | None, str]] = []


# Supported ordinary Thumb/M0 instructions. Unrecognized instructions are not
# assumed to fall through safely. This is not a decoder or stack-integrity proof.
ORDINARY = set("adcs adds add adr ands asrs bics cmn cmp cpsid cpsie dmb dsb eors "
               "isb ldm ldmia ldr ldrb ldrh ldrsb ldrsh lsls lsrs mov movs mrs msr "
               "muls mvns neg negs nop orrs pop push rev rev16 revsh rors rsbs sbcs sev "
               "stm stmia str strb strh sub subs sxtb sxth tst uxtb uxth yield".split())


def opaque_transfer(base: str, operands: str) -> bool:
    if base not in ORDINARY | BRANCHES | DIRECT_CALLS:
        return True
    # PC as destination, or a non-return multiple load to PC, needs a target
    # proof the audit does not have. Literal loads *from* PC remain ordinary.
    return (re.match(r"^\s*pc\s*(?:,|$)", operands) is not None
            or (base != "pop" and "{" in operands and re.search(r"\bpc\b", operands) is not None))


def has_real_loop(f: Func) -> bool:
    """True only if some backward branch is part of a cycle.

    A backward branch is a loop only if its target can reach it again. The
    first version of this audit counted every backward jump, and after E076 it
    failed `ADC_COMP` on `cortex_m::interrupt::free`: the compiler had
    tail-merged three exits of a critical-section closure onto a shared block
    at a lower address, which then runs forward to the epilogue. No path leads
    back, so it is not a cycle. `scripts/isr_cycles.py` already applied this
    test (E044); the audit now applies the same one, so a real loop is still
    refused and a tail merge is not.
    """
    idx = {a: i for i, (a, _, _, _) in enumerate(f.cfg)}
    n = len(f.cfg)
    succ: list[list[int]] = [[] for _ in range(n)]
    back: list[tuple[int, int]] = []
    for i, (a, base, dest, ops) in enumerate(f.cfg):
        is_return = (base == "pop" and "pc" in ops) or base == "bx"
        if is_return:
            continue
        if base in BRANCHES:
            if dest is not None and dest in idx:
                succ[i].append(idx[dest])
                if dest <= a:
                    back.append((i, idx[dest]))
            if base != "b" and i + 1 < n:
                succ[i].append(i + 1)
            continue
        if i + 1 < n:
            succ[i].append(i + 1)

    def reaches(src: int, dst: int) -> bool:
        seen, stack = {dst}, [dst]
        while stack:
            u = stack.pop()
            if u == src:
                return True
            for v in succ[u]:
                if v not in seen:
                    seen.add(v)
                    stack.append(v)
        return False

    return any(reaches(s_, d) for s_, d in back)


def prune_unreachable(f: Func) -> None:
    """Ignore decoded padding, but retain reachable out-of-line blocks.

Do not stop parsing at the first return: LLVM can place live branch targets
after an epilogue. Walk from the symbol entry before deriving call edges.
"""
    if not f.cfg:
        return
    positions = {row[0]: i for i, row in enumerate(f.cfg)}
    seen, pending = set(), [0]
    while pending:
        i = pending.pop()
        if i in seen:
            continue
        seen.add(i)
        _, base, dest, operands = f.cfg[i]
        if opaque_transfer(base, operands):
            continue
        if (base == "pop" and "pc" in operands) or base == "bx":
            continue
        if base in BRANCHES and dest in positions:
            pending.append(positions[dest])
        if base != "b" and i + 1 < len(f.cfg):
            pending.append(i + 1)
    f.cfg = [row for i, row in enumerate(f.cfg) if i in seen]
    f.insns = len(f.cfg)
    f.calls.clear()
    f.indirect = False
    for _, base, destination, operands in f.cfg:
        if opaque_transfer(base, operands):
            f.indirect = True
            continue
        target = target_symbol(operands)
        # Entry-only pruning cannot certify a call into another symbol's
        # interior. Preserve that full target as unresolved, rather than
        # silently audit a different entry path.
        match = TARGET_RE.search(operands)
        interior = match.group(1) if match and OFFSET_RE.search(match.group(1)) else None
        if interior and (base in DIRECT_CALLS or (target != f.name and base in BRANCHES)):
            f.calls.add(interior)
            continue
        if base in DIRECT_CALLS:
            if target:
                f.calls.add(target)
            else:
                # Includes register aliases (ip/r12) and unannotated numeric
                # calls: no resolvable edge means no clean certificate.
                f.indirect = True
        elif base == "bx":
            f.indirect = operands.strip() != "lr" or f.indirect
        elif base in BRANCHES:
            if target and target != f.name:
                f.calls.add(target)
            elif destination not in positions:
                # Unannotated external tail transfers are calls too. A missing
                # internal destination also cannot be silently discarded.
                f.indirect = True


def parse(disasm: str) -> dict[str, Func]:
    funcs: dict[str, Func] = {}
    cur: Func | None = None
    for line in disasm.splitlines():
        head = HEADER_RE.match(line)
        if head:
            addr, name = int(head.group(1), 16), head.group(2)
            cur = funcs.setdefault(name, Func(name, addr))
            continue
        if cur is None:
            continue
        insn = INSN_RE.match(line)
        if not insn:
            continue
        here, mnem, rest = int(insn.group(1), 16), insn.group(2), insn.group(3)
        cur.insns += 1
        base = mnem.split(".")[0]
        tgt = target_symbol(rest)
        dest_in_fn: int | None = None
        if base in DIRECT_CALLS:
            if tgt:
                cur.calls.add(tgt)
            elif re.match(r"^r\d", rest.strip()):
                cur.indirect = True          # blx rN — unresolvable statically
        elif base in BRANCHES:
            if tgt and tgt != cur.name:
                # tail-call style branch into another symbol is an edge too
                cur.calls.add(tgt)
            m = re.search(r"\b([0-9a-f]{4,8})\b", rest)
            if m:
                try:
                    dest = int(m.group(1), 16)
                except ValueError:
                    dest = here
                dest_in_fn = dest
        cur.cfg.append((here, base, dest_in_fn, rest))
    for f in funcs.values():
        prune_unreachable(f)
        f.has_backward_branch = has_real_loop(f)
    return funcs


def reachable(funcs: dict[str, Func], root: str) -> tuple[set[str], list[str]]:
    """BFS from root. Returns (reachable symbols, missing symbols)."""
    seen, missing, q = set(), [], deque([root])
    while q:
        name = q.popleft()
        if name in seen:
            continue
        seen.add(name)
        fn = funcs.get(name)
        if fn is None:
            missing.append(name)
            continue
        for callee in fn.calls:
            if callee not in seen:
                q.append(callee)
    return seen, missing


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--elf", required=True)
    ap.add_argument("--root", action="append", default=[],
                    help="motor ISR root symbol (repeat; e.g. COMP COM DMA guard)")
    ap.add_argument("--allow-file")
    ap.add_argument("--objdump", default="arm-none-eabi-objdump")
    ap.add_argument("--list-vectors", action="store_true",
                    help="list plausible interrupt-vector symbols and exit")
    ap.add_argument("-v", "--verbose", action="store_true")
    args = ap.parse_args()

    if not shutil.which(args.objdump):
        sys.exit(f"error: objdump not on PATH: {args.objdump}")

    funcs = parse(objdump_all(args.objdump, args.elf))
    if not funcs:
        sys.exit("error: no functions parsed from disassembly")

    if args.list_vectors:
        # cortex-m-rt vectors are upper-case-ish, no '::', and are plain symbols.
        cands = sorted(n for n in funcs
                       if "::" not in n and re.fullmatch(r"[A-Z][A-Z0-9_]{2,}", n))
        print(f"{len(cands)} candidate vector symbols:")
        for n in cands:
            print(f"  {n:<32} insns={funcs[n].insns}")
        return 0

    if not args.root:
        sys.exit("error: at least one --root is required (or use --list-vectors)")

    allow = {"indirect_calls": [], "reviewed_loops": [], "panic_paths": []}
    if args.allow_file:
        try:
            with open(args.allow_file) as fh:
                allow.update(json.load(fh))
        except OSError as exc:
            sys.exit(f"error: cannot read allow file: {exc}")
    allow_indirect = set(allow.get("indirect_calls", []))
    allow_loops = set(allow.get("reviewed_loops", []))
    allow_panic = set(allow.get("panic_paths", []))

    failures: list[str] = []
    print(f"ISR-root audit: {args.elf}")
    for root in args.root:
        if root not in funcs:
            failures.append(f"{root}: root symbol NOT FOUND in ELF (fail-closed)")
            print(f"\n  [{root}] NOT FOUND")
            continue
        seen, missing = reachable(funcs, root)
        bad_arith = sorted(seen & FORBIDDEN)
        panics = sorted((seen & PANIC) - allow_panic)
        indirect = sorted(n for n in seen
                          if n in funcs and funcs[n].indirect and n not in allow_indirect)
        # Panic machinery is reported as its own class; don't double-report it
        # here just because it loops internally.
        loops = sorted(n for n in seen
                       if n in funcs and funcs[n].has_backward_branch
                       and n not in allow_loops and n not in PANIC)
        # A callee we cannot see (e.g. from a stripped/veneer symbol) is
        # unresolved reachability -> fail closed.
        unresolved = sorted(n for n in missing if n not in FORBIDDEN)

        ok = not (bad_arith or panics or indirect or loops or unresolved)
        print(f"\n  [{root}] reachable={len(seen)}  {'OK' if ok else 'FAIL'}")
        if args.verbose:
            for n in sorted(seen):
                print(f"      {n}")
        if bad_arith:
            failures.append(f"{root}: forbidden arithmetic reachable: {bad_arith}")
            print(f"      forbidden arithmetic: {bad_arith}")
        if panics:
            failures.append(f"{root}: panic path reachable: {panics}")
            print(f"      panic path reachable: {panics}")
        if indirect:
            failures.append(f"{root}: unresolved indirect call in: {indirect}")
            print(f"      indirect call (not allowlisted): {indirect}")
        if loops:
            failures.append(f"{root}: unreviewed loop in: {loops}")
            print(f"      loop not in reviewed_loops: {loops}")
        if unresolved:
            failures.append(f"{root}: unresolved callee symbols: {unresolved}")
            print(f"      unresolved callees: {unresolved}")

    print()
    if failures:
        print(f"AUDIT FAILED ({len(failures)} finding(s)):")
        for f in failures:
            print(f"  - {f}")
        return 1
    print(f"AUDIT PASSED: {len(args.root)} root(s) certified clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
