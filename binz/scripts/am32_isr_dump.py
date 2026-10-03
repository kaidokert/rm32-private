#!/usr/bin/env python3
"""Dump AM32's three critical ISRs (G071 build) as source-interleaved disassembly.

For each root handler, walks the static call graph (`bl` / tail-call `b` targets that are
function symbols) and writes every reached function with `objdump -d -S -l`, so the
reference shows exactly the instructions the ISR can execute, not just its top frame.

    python scripts/am32_isr_dump.py [--elf PATH] [--am32 DIR] [--out DIR]

Run objdump with cwd = the AM32 tree so DWARF-relative source paths resolve. Fails loudly
if a root symbol is missing or a function disassembles to nothing.
"""
import argparse, hashlib, pathlib, re, subprocess, sys

ROOTS = {
    "comp": ("ADC1_COMP_IRQHandler", "BEMF comparator edge -> interruptRoutine (zero-cross accept + arm COM timer)"),
    "com": ("TIM14_IRQHandler", "COM_TIMER (TIM14) update -> PeriodElapsedCallback (commutate + ZC bookkeeping)"),
    "tenkhz": ("TIM6_DAC_LPTIM1_IRQHandler", "20 kHz control loop -> tenKhzRoutine"),
}
OBJDUMP = "arm-none-eabi-objdump"
NM = "arm-none-eabi-nm"


def run(cmd, cwd):
    r = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"FAILED: {' '.join(cmd)}\n{r.stderr}")
    return r.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--am32", default="E:/m/robot/esc/AM32")
    ap.add_argument("--elf", default="obj/AM32_DRV8304H_G071_2.20.elf")
    ap.add_argument("--out", default="am32_g071_isr_ref")
    ap.add_argument("--max-depth", type=int, default=6)
    ap.add_argument("--root", action="append", default=[],
                    help="key=SYMBOL=description; replaces the AM32 roots (e.g. for a firmware50 ELF)")
    a = ap.parse_args()
    roots = ROOTS
    if a.root:
        roots = {}
        for spec in a.root:
            k, s, d = (spec.split("=", 2) + [""])[:3]
            roots[k] = (s, d)
    am32 = pathlib.Path(a.am32)
    elf = (am32 / a.elf).resolve()
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)

    # Function symbols with sizes: name -> (addr, size).
    syms = {}
    for line in run([NM, "-S", "--defined-only", str(elf)], am32).splitlines():
        p = line.split()
        if len(p) == 4 and p[2] in "TtWw":
            syms.setdefault(p[3], (int(p[0], 16), int(p[1], 16)))
    by_addr = {v[0]: k for k, v in syms.items()}

    def disasm(fn):
        txt = run([OBJDUMP, "-d", "-S", "-l", "--no-show-raw-insn", f"--disassemble={fn}", str(elf)], am32)
        body = txt[txt.find(f"<{fn}>:"):] if f"<{fn}>:" in txt else ""
        if not body:
            sys.exit(f"FAILED: {fn} disassembled to nothing")
        return body

    insn_re = re.compile(r"^\s+([0-9a-f]+):\s+(\S+)(?:\s+(.*))?$")

    def callees(body):
        seen = []
        for line in body.splitlines():
            m = insn_re.match(line)
            if not m:
                continue
            op, args = m.group(2), m.group(3) or ""
            if op in ("bl", "blx", "b", "b.n", "b.w"):
                t = re.search(r"<([^>+]+)>", args)
                if t and t.group(1) in syms and "+" not in args.split("<")[1]:
                    seen.append(t.group(1))
        return seen

    def count(body):
        return sum(1 for l in body.splitlines() if insn_re.match(l))

    sha = hashlib.sha256(elf.read_bytes()).hexdigest()
    summary = [f"ELF {elf.name} sha256 {sha}", ""]
    for key, (root, what) in roots.items():
        if root not in syms:
            sys.exit(f"FAILED: root {root} not in the ELF")
        order, depth_of, edges = [], {root: 0}, {}
        queue = [root]
        while queue:
            fn = queue.pop(0)
            order.append(fn)
            body = disasm(fn)
            cs = []
            for c in callees(body):
                if c == fn or c in cs:
                    continue
                cs.append(c)
                if c not in depth_of and depth_of[fn] < a.max_depth:
                    depth_of[c] = depth_of[fn] + 1
                    queue.append(c)
            edges[fn] = cs
        path = out / f"{key}_{root}.lst"
        with path.open("w", newline="\n") as f:
            f.write(f"; AM32 G071 reference ISR: {root} -- {what}\n")
            f.write(f"; ELF {elf.name}  sha256 {sha}\n; objdump -d -S -l, call tree walked to depth {a.max_depth}\n;\n")
            f.write("; call tree (function: instructions, size bytes -> callees)\n")
            for fn in order:
                f.write(f";   {'  ' * depth_of[fn]}{fn}: {count(disasm(fn))} insns, {syms[fn][1]} B"
                        f"{' -> ' + ', '.join(edges[fn]) if edges[fn] else ''}\n")
            f.write(";\n\n")
            for fn in order:
                f.write(f"; ===================== {fn} =====================\n")
                f.write(disasm(fn))
                f.write("\n")
        summary.append(f"[{key}] {root}: {what}")
        for fn in order:
            summary.append(f"  {'  ' * depth_of[fn]}{fn}: {count(disasm(fn))} insns, {syms[fn][1]} B"
                           f"{' -> ' + ', '.join(edges[fn]) if edges[fn] else ''}")
        summary.append(f"  -> {path}")
        summary.append("")
    (out / "call_trees.txt").write_text("\n".join(summary) + "\n", newline="\n")
    print("\n".join(summary))


if __name__ == "__main__":
    main()
