#!/usr/bin/env python3
"""Worst-case cycle count for an ISR root, from the linked ELF.
(Copied verbatim from binz/firmware50/scripts/isr_cycles.py, rm32-sagcheck
worktree, 2026-10-02 — the WCET method of firmware50 WCET_ESTIMATES.md.)

Goal gate 5 asks for the *longest-path* cycle count of each motor ISR root.
Once the zero-crossing decision moved into `ADC_COMP` (notebook E044) that
handler stopped being straight-line code, so its cost can no longer be read
off by summing instructions: it has branches, a bounded loop, and a call.

Method, stated so the number can be checked:

* Disassemble the root with llvm-objdump and build its control-flow graph.
* Cost each instruction at Cortex-M0+ rates with zero flash wait states (the
  G071 runs 64 MHz with 2 wait states, so real cost is *higher*; the wait-state
  factor is reported separately rather than guessed into the table):
    - ALU / MOV / CMP / shifts ............ 1
    - LDR / STR (any addressing) .......... 2
    - LDM / STM / PUSH / POP .............. 1 + registers
    - POP including PC .................... 1 + registers + 2 (pipeline refill)
    - MUL (M0+ single-cycle multiplier) ... 1
    - taken branch ........................ 3; not-taken conditional .. 1
    - BL .................................. 4, plus the callee's worst path
    - MRS / MSR ........................... 4 (architecturally 3-4)
    - CPSID / CPSIE ....................... 1
* The longest path is the maximum-cost path from entry to any return.
* A backward branch is a loop. Its trip count is **not** inferred: it must be
  supplied (`--loop-bound`), and the script refuses to report a figure for a
  root with an unbounded loop -- the same fail-closed rule as the audit.
* Callees are costed by the same method, recursively, and cached.

This is a static upper bound on the instruction path, not a measurement. It
excludes exception entry/exit (12 cycles in, 10-12 out on M0+, reported as a
separate line) and interrupt latency jitter.

Flash fetch model (`--fetch-model`, WCET_ESTIMATES.md). Off by default, and
then every figure is the 0-wait-state one above, unchanged. When on, with
`--wait-states` W:
    - every literal-pool load (`ldr rX, [pc, ...]`) ... + W (a flash data read)
    - every taken branch .............................. + W (target refetch)
    - every instruction ............................... + W * `--per-insn`
      (0.25 = one miss per 64-bit line of thumb16 code, 1 = every one misses)
    - `--loop-hit`: a bounded loop's extra trips pay no fetch penalty (the
      body is in the instruction cache after the first trip)
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from functools import lru_cache

OBJDUMP_DEFAULT = (
    r"C:\Users\kaido\.rustup\toolchains\1.80-x86_64-pc-windows-msvc"
    r"\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-objdump.exe"
)

LINE = re.compile(r"^\s*([0-9a-f]+):\s+(?:[0-9a-f]{2,4}\s?){1,2}\s+(\S+)\s*(.*)$")
COND = {"eq", "ne", "cs", "hs", "cc", "lo", "mi", "pl", "vs", "vc",
        "hi", "ls", "ge", "lt", "gt", "le"}


def load(objdump: str, elf: str) -> dict[str, list[tuple[int, str, str]]]:
    out = subprocess.run([objdump, "-d", elf], capture_output=True, text=True).stdout
    funcs: dict[str, list[tuple[int, str, str]]] = {}
    cur = None
    for raw in out.splitlines():
        m = re.match(r"^([0-9a-f]+) <(.+)>:$", raw)
        if m:
            cur = m.group(2)
            funcs[cur] = []
            continue
        if cur is None:
            continue
        lm = LINE.match(raw)
        if lm:
            funcs[cur].append((int(lm.group(1), 16), lm.group(2), lm.group(3)))
    return funcs


def regcount(ops: str) -> int:
    m = re.search(r"\{([^}]*)\}", ops)
    if not m:
        return 1
    n = 0
    for part in m.group(1).split(","):
        part = part.strip()
        if "-" in part:
            a, b = part.split("-")
            n += int(b.strip()[1:]) - int(a.strip()[1:]) + 1
        elif part:
            n += 1
    return n


def cost(mn: str, ops: str) -> int:
    m = mn.lower().rstrip(".nw")
    if m.startswith(("ldr", "str")):
        return 2
    if m in ("push",):
        return 1 + regcount(ops)
    if m in ("pop",):
        extra = 2 if "pc" in ops else 0
        return 1 + regcount(ops) + extra
    if m.startswith(("ldm", "stm")):
        return 1 + regcount(ops)
    if m in ("mrs", "msr"):
        return 4
    if m in ("cpsid", "cpsie", "nop"):
        return 1
    if m == "bl":
        return 4
    return 1  # ALU, mov, cmp, shifts, muls


def target(ops: str) -> int | None:
    m = re.search(r"0x([0-9a-f]+)", ops)
    return int(m.group(1), 16) if m else None



def _topo_order(n, succ, back_edges):
    """Topological order of the graph with `back_edges` removed.

    Raises if what remains is not a DAG: a cycle here means a real loop was not
    classified as one, and a longest path over it is meaningless rather than
    merely wrong. Refusing is the point -- see the note at the relaxation.
    """
    indeg = [0] * n
    for i in range(n):
        for j in succ[i]:
            if (i, j) not in back_edges and j < n:
                indeg[j] += 1
    from collections import deque

    q = deque(i for i in range(n) if indeg[i] == 0)
    order = []
    while q:
        i = q.popleft()
        order.append(i)
        for j in succ[i]:
            if (i, j) in back_edges or j >= n:
                continue
            indeg[j] -= 1
            if indeg[j] == 0:
                q.append(j)
    if len(order) != n:
        raise SystemExit(
            f"isr_cycles: {n - len(order)} instruction(s) left in a cycle after "
            "back-edge removal -- a loop was not classified, so the longest path "
            "would be meaningless. Refusing rather than under-reporting."
        )
    return order


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--root", required=True)
    ap.add_argument("--objdump", default=OBJDUMP_DEFAULT)
    ap.add_argument(
        "--loop-bound",
        type=int,
        action="append",
        default=[],
        help="trip count for a reviewed loop, in order of back-edges found",
    )
    ap.add_argument(
        "--callee-bound",
        action="append",
        default=[],
        metavar="SYMBOL=N[,N...]",
        help="trip counts for reviewed loops inside a callee whose (mangled or "
        "demangled) name contains SYMBOL; without one a callee loop is still "
        "refused (fail-closed)",
    )
    ap.add_argument(
        "--terminal",
        action="append",
        default=["panic", "rust_begin_unwind", "unwrap_failed", "expect_failed",
                 "assert_failed", "slice_index", "slice_end_index", "slice_start_index"],
        help="(rm32 addition) substring of a callee that never returns (panic "
        "machinery): a BL to it ends the path instead of costing a return. "
        "Panic sites are listed separately with --list-terminal so they are "
        "reviewed, not hidden.",
    )
    ap.add_argument(
        "--zero-callee",
        action="append",
        default=[],
        help="(rm32 addition) substring of a callee costed at 0 cycles while the "
        "path continues — for a MODE-specific bound that excludes branches "
        "the mode cannot reach (each exclusion must be justified by the caller)",
    )
    ap.add_argument("--list-terminal", action="store_true",
                    help="(rm32 addition) print every BL into a terminal callee")
    ap.add_argument("--wait-states", type=int, default=2)
    ap.add_argument("--fetch-model", action="store_true",
                    help="charge flash wait states on fetches (see docstring)")
    ap.add_argument("--per-insn", type=float, default=0.0,
                    help="fetch misses per instruction, with --fetch-model")
    ap.add_argument("--loop-hit", action="store_true",
                    help="extra loop trips hit the instruction cache")
    args = ap.parse_args()
    ws_fetch = args.wait_states if args.fetch_model else 0

    callee_bounds: list[tuple[str, tuple[int, ...]]] = []
    for spec in args.callee_bound:
        sym, _, nums = spec.partition("=")
        callee_bounds.append((sym, tuple(int(n) for n in nums.split(",") if n)))

    def bounds_for(name: str) -> tuple[int, ...]:
        for sym, b in callee_bounds:
            if sym in name:
                return b
        return ()

    def is_terminal(name: str) -> bool:
        return any(t in name for t in args.terminal)

    funcs = load(args.objdump, args.elf)
    terminal_sites: list[str] = []
    if args.root not in funcs:
        sys.exit(f"root {args.root} not found")

    @lru_cache(maxsize=None)
    def worst(name: str, bounds: tuple[int, ...]) -> tuple[int, int]:
        insns = funcs.get(name)
        if not insns:
            # Library helpers without disassembly here: charge a conservative
            # flat cost rather than zero, and say so.
            return (40, 0)
        addr_ix = {a: i for i, (a, _, _) in enumerate(insns)}
        n = len(insns)
        back_edges = []
        succ: list[list[int]] = [[] for _ in range(n)]
        for i, (a, mn, ops) in enumerate(insns):
            m = mn.lower()
            base = m[:-2] if len(m) > 2 and m[-2:] in COND and m.startswith("b") else m
            is_b = base == "b" or (m.startswith("b") and m[1:3] in COND and len(m) <= 4)
            if m == "b" or m == "b.n" or m == "b.w":
                t = target(ops)
                if t in addr_ix:
                    succ[i].append(addr_ix[t])
                    if t <= a:
                        back_edges.append((i, addr_ix[t]))
                continue
            if is_b and m != "bl":
                t = target(ops)
                if i + 1 < n:
                    succ[i].append(i + 1)
                if t in addr_ix:
                    succ[i].append(addr_ix[t])
                    if t <= a:
                        back_edges.append((i, addr_ix[t]))
                continue
            if m == "pop" and "pc" in ops:
                continue
            if m == "bl":
                callee = re.search(r"<([^>+]+)", ops)
                if callee and is_terminal(callee.group(1)):
                    terminal_sites.append(f"{name} @ {a:#x} -> {callee.group(1)}")
                    continue  # never returns: the path ends here
            if m == "bx":
                continue
            if i + 1 < n:
                succ[i].append(i + 1)

        # A backward branch is a loop only if its target can reach it again.
        #
        # The first version of this tool counted every backward jump as a loop,
        # and failed ADC_COMP on two. One was real (the 12-read persistence
        # filter). The other was the compiler tail-merging the accept path onto
        # a shared return trampoline that happened to sit at a lower address:
        # `b 0x8000ab0`, where 0x8000ab0 is itself `b` to the epilogue. No path
        # leads back from there, so it is not a cycle. Reachability, not
        # direction, is what makes a loop.
        def reaches(src: int, dst: int) -> bool:
            seen = {dst}
            stack = [dst]
            while stack:
                u = stack.pop()
                if u == src:
                    return True
                for v in succ[u]:
                    if v not in seen:
                        seen.add(v)
                        stack.append(v)
            return False

        back_edges = [(s, d) for (s, d) in back_edges if reaches(s, d)]

        # Loop bodies are costed once per trip; the longest path treats each
        # back-edge as taken (bound - 1) extra times.
        if len(back_edges) > len(bounds):
            raise SystemExit(
                f"{name}: {len(back_edges)} loop(s) found but only "
                f"{len(bounds)} --loop-bound given; refusing to report "
                f"an unbounded figure (fail-closed)"
            )

        def insn_cost(i: int) -> int:
            a, mn, ops = insns[i]
            c = cost(mn, ops)
            if ws_fetch:
                c += ws_fetch * args.per_insn
            if mn.lower().startswith("ldr") and "[pc" in ops:
                c += ws_fetch
            if mn.lower() == "bl":
                callee = re.search(r"<([^>+]+)", ops)
                if (callee and not is_terminal(callee.group(1))
                        and not any(z in callee.group(1) for z in args.zero_callee)):
                    c += worst(callee.group(1), bounds_for(callee.group(1)))[0]
            return c

        # Longest path on the DAG obtained by removing back-edges.
        #
        # **Relaxed in topological order, not instruction order.** Index order
        # is a valid topological order only if every surviving edge goes
        # forward, and the compiler is free to place a tail-merged or outlined
        # block *after* the epilogue and jump backwards into the body -- which
        # is not a loop and is not removed as a back-edge. A single pass in
        # index order then never propagates that block's cost into the
        # instructions that follow it, and the tool silently under-reports.
        #
        # That happened, and it took two independent reviews to catch: adding a
        # ten-instruction instrument to `ADC_COMP` moved one `EventWatch` block
        # out of line, created four backward non-loop edges, and the reported
        # longest path *fell* by 45 cycles at 0 WS. The honest figures were 18
        # cycles lower, not 45, and the difference was entirely this loop
        # (E209 SS3, E210 SS2). A gate that can quietly report an improvement
        # for a change that costs cycles is worse than no gate.
        be = set(back_edges)
        order = _topo_order(n, succ, be)
        best = [-(10**9)] * n
        best[0] = insn_cost(0)
        for i in order:
            if best[i] < 0:
                continue
            for j in succ[i]:
                if (i, j) in be:
                    continue
                taken = (3 + ws_fetch) if j != i + 1 else 1
                cand = best[i] + taken - 1 + insn_cost(j)
                if cand > best[j]:
                    best[j] = cand
        path = max(b for b in best if b >= 0)

        # Add the extra trips of each bounded loop at its body cost.
        extra = 0
        for (k, (src, dst)), bound in zip(enumerate(back_edges), bounds):
            if args.loop_hit:
                body = sum(cost(insns[i][1], insns[i][2]) for i in range(dst, src + 1)) + 3
            else:
                body = sum(insn_cost(i) for i in range(dst, src + 1)) + 3 + ws_fetch
            extra += body * max(bound - 1, 0)
        return (path + extra, len(back_edges))

    total, loops = worst(args.root, tuple(args.loop_bound))
    if args.list_terminal:
        for site in sorted(set(terminal_sites)):
            print(f"terminal             {site}")
    ws = args.wait_states
    print(f"root                 {args.root}")
    print(f"loops (back-edges)   {loops}, bounds supplied {args.loop_bound}")
    if args.fetch_model:
        print(f"longest path         {total:.0f} cycles @ {ws} wait states, "
              f"per-insn {args.per_insn}, loop-hit {args.loop_hit}")
    else:
        print(f"longest path         {total} cycles @ 0 wait states")
    print(f"                     {total / 64:.2f} us @ 64 MHz")
    print(f"exception entry+exit ~24 cycles (0.38 us), not included above")
    print(f"flash wait states    {ws} configured; instruction fetch not modelled, "
          f"real cost is higher than the figure above")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
