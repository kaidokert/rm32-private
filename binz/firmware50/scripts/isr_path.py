#!/usr/bin/env python3
"""Executed-path instruction count for an ISR, from an `objdump -d -S -l` listing.

    python scripts/isr_path.py LISTING SPEC [--show]

LISTING is a dump from `am32_isr_dump.py` (one or more functions, source-interleaved).
SPEC declares the path: one decision per line, `#` comments allowed:

    <entry> FUNCTION_NAME          where to start (first line of the spec)
    <hexaddr> T                    this conditional branch is taken
    <hexaddr> N                    ... not taken
    <hexaddr> T*k                  taken the first k times it executes, then not taken (loops)
    <hexaddr> N*k                  not taken the first k times, then taken (loop exits)
    <hexaddr> ->HEX                indirect jump (mov/add/ldr pc) goes to HEX
    <hexaddr> call                 follow a `bl` into its target (default: count the bl, skip the callee)

The walk FAILS LOUDLY at any conditional branch or indirect jump the spec does not declare,
printing the source line it belongs to, so every decision on the path is explicit and
auditable. It ends at the first return (`pop {..pc}`, `bx lr`) of the entry function.
Counts instructions (not cycles). `bl` into a callee is followed only when declared `call`;
the callee's instructions are then counted up to its return.
"""
import re, sys

INSN = re.compile(r"^\s+([0-9a-f]+):\s+(\S+)(?:\s+(.*))?$")
SRC = re.compile(r"^(?:[A-Za-z]:)?[^\s:]*[\\/][^\s:]+:(\d+)")
FUNC = re.compile(r"^<([^>]+)>:")
CONDS = {"beq", "bne", "bcs", "bcc", "bhs", "blo", "bmi", "bpl", "bvs", "bvc",
         "bhi", "bls", "bge", "blt", "bgt", "ble"}


def load(path):
    insns, order, func_at = {}, [], {}
    cur_func, last_loc, last_text = None, "", ""
    for line in open(path, encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        m = FUNC.match(line)
        if m:
            cur_func = m.group(1)
            continue
        m = INSN.match(line)
        if m:
            a = int(m.group(1), 16)
            op, args = m.group(2), (m.group(3) or "")
            if op.startswith("."):
                continue
            if a not in insns:
                insns[a] = dict(op=op, args=args, loc=last_loc, text=last_text, func=cur_func)
                order.append(a)
                func_at.setdefault(cur_func, a)
            continue
        if SRC.match(line.strip()):
            last_loc = line.strip().split("\\")[-1].split("/")[-1]
        elif line.strip() and not line.startswith(";"):
            last_text = line.strip()
    order.sort()
    nxt = {a: order[i + 1] for i, a in enumerate(order[:-1])}
    return insns, nxt, func_at


def target(args):
    m = re.match(r"([0-9a-f]+)\b", args)
    return int(m.group(1), 16) if m else None


def main():
    listing, spec_path = sys.argv[1], sys.argv[2]
    show = "--show" in sys.argv
    insns, nxt, func_at = load(listing)
    decisions, entry = {}, None
    for raw in open(spec_path, encoding="utf-8"):
        s = raw.split("#", 1)[0].strip()
        if not s:
            continue
        a, d = s.split(None, 1)
        if a == "<entry>":
            entry = d.strip()
            continue
        decisions[int(a, 16)] = d.strip()
    assert entry in func_at, f"entry {entry} not in listing"
    pc, n, seen, stack = func_at[entry], 0, {}, []
    per_func = {}
    while True:
        i = insns.get(pc)
        if i is None:
            sys.exit(f"FAIL: walked to {pc:08x}, not an instruction in the listing")
        n += 1
        per_func[i["func"]] = per_func.get(i["func"], 0) + 1
        seen[pc] = seen.get(pc, 0) + 1
        op, args = i["op"], i["args"]
        if show:
            print(f"{n:4d} {pc:08x} {op:8s} {args[:40]:40s} ; {i['loc']}")
        if seen[pc] > 10_000:
            sys.exit(f"FAIL: runaway loop at {pc:08x}")
        base = op.split(".")[0]
        # Returns.
        if (base == "pop" and "pc" in args) or (base == "bx" and args.startswith("lr")):
            if stack:
                pc = stack.pop()
                continue
            break
        # Indirect jumps.
        if (base in ("mov", "add") and args.startswith("pc")) or (base == "ldr" and args.startswith("pc")) or base == "bx":
            d = decisions.get(pc)
            if not d or not d.startswith("->"):
                sys.exit(f"FAIL: undeclared indirect jump at {pc:08x} ({op} {args}) ; {i['loc']} :: {i['text']}")
            pc = int(d[2:], 16)
            continue
        if base in ("bl", "blx"):
            t = target(args)
            if decisions.get(pc) == "call" and t in insns:
                stack.append(nxt[pc])
                pc = t
                continue
            pc = nxt[pc]
            continue
        if base == "b":
            pc = target(args)
            continue
        if base in CONDS:
            d = decisions.get(pc)
            if d is None:
                sys.exit(f"FAIL: undeclared branch at {pc:08x} ({op} {args}) seen {seen[pc]}x ; {i['loc']} :: {i['text']}")
            if d.startswith("T*"):
                taken = seen[pc] <= int(d[2:])
            elif d.startswith("N*"):
                taken = seen[pc] > int(d[2:])
            elif d in ("T", "N"):
                taken = d == "T"
            else:
                sys.exit(f"FAIL: bad decision {d!r} at {pc:08x}")
            pc = target(args) if taken else nxt[pc]
            continue
        pc = nxt[pc]
    print(f"EXECUTED {n} instructions from {entry}")
    for f, c in per_func.items():
        print(f"  {c:5d}  {f}")
    unused = sorted(set(decisions) - set(seen))
    if unused:
        print("WARNING: declared decisions never reached: " + ", ".join(f"{a:08x}" for a in unused))


if __name__ == "__main__":
    main()
