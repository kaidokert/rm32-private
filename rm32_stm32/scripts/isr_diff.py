#!/usr/bin/env python3
"""ISR-root disassembly dump and diff (generic-core refactor gate, S0.7).

    python scripts/isr_diff.py dump --elf <rm32_firmware> --out captures/gc/s00/isr_l431
    python scripts/isr_diff.py diff captures/gc/s00/isr_l431 captures/gc/s03/isr_l431

`dump` writes, for every interrupt/exception handler the image actually
defines (a vector symbol whose address is not DefaultHandler's; Reset,
HardFault and main excluded), the whole static call tree of that root as
normalized assembly:

  * addresses and raw encodings dropped;
  * functions named by discovery order (F0 = the root, F1, F2, ... in the
    order the tree first calls them), so renames and crate moves do not
    show; the real names are written to <ROOT>.names for reading;
  * branch targets inside a function as offsets from its start;
  * literal-pool words that point into flash or SRAM become ADDR (static
    placement moves with any unrelated change); every other constant,
    peripheral addresses included, is kept.

`diff` compares two dumps root by root and exits 1 on any difference (a
root present on one side only is a difference). A HOST-class step must
diff empty against its predecessor; a commit diffed against itself is
empty by construction.

`removals` is the CODE-step check ("the diff shows only removals"). A
removal re-runs register allocation over the whole function, so a line
diff (and even a per-mnemonic count) grows in places nothing was added.
The check therefore judges work, per root, and exits 1 on any of:

  * a new root, or a function the old tree did not call (LLVM's per-build
    `.llvm.<n>` name suffix ignored);
  * more instructions in the root than before;
  * more calls (bl/blx), barriers (dmb/dsb/isb), exclusive accesses
    (ldrex/strex) or interrupt masking (cpsid/cpsie/msr);
  * a constant that was not there before (peripheral addresses and every
    other immediate; flash/SRAM pointers are already ADDR).

Other per-mnemonic growth is printed as re-allocation noise for reading.

    python scripts/isr_diff.py removals captures/gc/s03.2b/isr_pub_stm32l431 captures/gc/s03.3/isr_pub_stm32l431
"""

from __future__ import annotations

import argparse
import collections
import difflib
import pathlib
import re
import subprocess
import sys

OBJDUMP = "arm-none-eabi-objdump"
NM = "arm-none-eabi-nm"
NOT_ROOTS = {"Reset", "HardFault", "_HardFault", "DefaultHandler", "DefaultHandler_",
             "DefaultPreInit", "main"}
HEADER = re.compile(r"^([0-9a-f]{8}) <(.+)>:$")
LINE = re.compile(r"^\s*([0-9a-f]+):\s+(\S+)\s*(.*)$")
TARGET = re.compile(r"(?:0x)?[0-9a-f]+ <([^>]+?)(?:\+(0x[0-9a-f]+))?>")


def is_ram_or_flash(v: int) -> bool:
    return 0x0800_0000 <= v < 0x0810_0000 or 0x2000_0000 <= v < 0x2004_0000


def load(elf: str) -> dict[str, tuple[int, list[tuple[int, str, str]]]]:
    out = subprocess.run([OBJDUMP, "-d", "--no-show-raw-insn", elf],
                         capture_output=True, text=True, check=True).stdout
    funcs: dict[str, tuple[int, list[tuple[int, str, str]]]] = {}
    cur = None
    for ln in out.splitlines():
        m = HEADER.match(ln)
        if m:
            cur = m.group(2)
            funcs[cur] = (int(m.group(1), 16), [])
            continue
        m = LINE.match(ln)
        if cur and m:
            funcs[cur][1].append((int(m.group(1), 16), m.group(2), m.group(3)))
    # Cut each body at its symbol size: objdump disassembles the alignment
    # padding up to the next symbol as instructions, so a layout shift alone
    # would add or drop "code" after a noreturn call.
    sizes: dict[str, int] = {}
    for ln in subprocess.run([NM, "-S", elf], capture_output=True, text=True,
                             check=True).stdout.splitlines():
        parts = ln.split()
        if len(parts) == 4 and parts[2] in "Tt":
            sizes[parts[3]] = int(parts[1], 16)
    for name, (start, body) in funcs.items():
        if name in sizes:
            end = (start & ~1) + sizes[name]
            body[:] = [ins for ins in body if ins[0] < end]
    return funcs


def roots(elf: str) -> list[str]:
    syms = {}
    for ln in subprocess.run([NM, elf], capture_output=True, text=True,
                             check=True).stdout.splitlines():
        parts = ln.split()
        if len(parts) == 3 and parts[1] in "Tt":
            syms[parts[2]] = int(parts[0], 16)
    default = syms.get("DefaultHandler_", syms.get("DefaultHandler"))
    if default is None:
        raise SystemExit(f"{elf}: no DefaultHandler symbol — not a cortex-m-rt image?")
    return sorted(n for n, a in syms.items()
                  if re.fullmatch(r"[A-Z][A-Za-z0-9_]*", n) and n not in NOT_ROOTS
                  and a != default)


def readable(name: str) -> str:
    """Mangled name without crate disambiguators / legacy hashes."""
    return re.sub(r"Cs[0-9A-Za-z]+_|17h[0-9a-f]{16}E", "", name)


def dump_root(funcs, root: str) -> tuple[list[str], list[str]]:
    label: dict[str, str] = {root: "F0"}
    order = [root]
    text: list[str] = []
    i = 0
    while i < len(order):
        name = order[i]
        i += 1
        start, body = funcs[name]
        text.append(f"{label[name]}:")
        fstart = len(text)
        movw: dict[str, int] = {}  # register -> text index of its last movw
        for _, mn, ops in body:
            ops = ops.split("@")[0].split(";")[0].strip()
            if mn == ".word":
                try:
                    v = int(ops, 16)
                except ValueError:
                    v = None
                if v is not None and is_ram_or_flash(v & ~1):
                    ops = "ADDR"
            else:
                def sub(m: re.Match) -> str:
                    tgt, off = m.group(1), m.group(2) or "0x0"
                    if tgt == name:
                        return f".+{off}"
                    if tgt in funcs and off == "0x0":
                        if tgt not in label:
                            label[tgt] = f"F{len(label)}"
                            order.append(tgt)
                        return label[tgt]
                    return f"<{readable(tgt)}+{off}>"
                ops = TARGET.sub(sub, ops)
            text.append(f"    {mn} {ops}".rstrip())
            # v7-M builds addresses as movw lo / movt hi: mask the pair when
            # it forms a flash/SRAM address (a .word in the literal pool on
            # v6-M is masked above).
            m = re.fullmatch(r"(movw|movt)(?:\.w)?\s+(\w+),\s*#(\S+)", f"{mn} {ops}")
            if m:
                reg, imm = m.group(2), int(m.group(3), 0)
                if m.group(1) == "movw":
                    movw[reg] = len(text) - 1
                elif reg in movw and movw[reg] >= fstart:
                    j = movw.pop(reg)
                    lo = int(re.search(r"#(\S+)", text[j]).group(1), 0)
                    if is_ram_or_flash((imm << 16) | lo):
                        text[j] = re.sub(r"#\S+", "#ADDR", text[j])
                        text[-1] = re.sub(r"#\S+", "#ADDR", text[-1])
    names = [f"{label[n]} {readable(n)}" for n in order]
    return text, names


def cmd_dump(a) -> int:
    funcs = load(a.elf)
    out = pathlib.Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rs = roots(a.elf)
    if not rs:
        raise SystemExit(f"{a.elf}: no ISR roots found")
    for r in rs:
        text, names = dump_root(funcs, r)
        (out / f"{r}.s").write_text("\n".join(text) + "\n")
        (out / f"{r}.names").write_text("\n".join(names) + "\n")
    (out / "roots.txt").write_text("\n".join(rs) + "\n")
    print(f"{len(rs)} roots -> {out}: " + ", ".join(
        f"{r}({sum(1 for ln in (out / f'{r}.s').read_text().splitlines() if ln.startswith('    '))})"
        for r in rs))
    return 0


def cmd_diff(a) -> int:
    da, db = pathlib.Path(a.a), pathlib.Path(a.b)
    ra = set((da / "roots.txt").read_text().split())
    rb = set((db / "roots.txt").read_text().split())
    differ = False
    for r in sorted(ra ^ rb):
        print(f"root {r}: only in {da if r in ra else db}")
        differ = True
    for r in sorted(ra & rb):
        x = (da / f"{r}.s").read_text().splitlines()
        y = (db / f"{r}.s").read_text().splitlines()
        if x != y:
            differ = True
            d = list(difflib.unified_diff(x, y, f"{da.name}/{r}", f"{db.name}/{r}", lineterm="", n=2))
            print(f"root {r}: {sum(ln.startswith('+') for ln in d[2:])} added / "
                  f"{sum(ln.startswith('-') for ln in d[2:])} removed lines")
            if not a.summary:
                print("\n".join(d))
    print("ISR diff: " + ("DIFFERENT" if differ else "identical"))
    return 1 if differ else 0


_FLAG_S = {"movs", "adds", "subs", "lsls", "lsrs", "asrs", "ands", "orrs", "eors",
           "muls", "mvns", "rsbs", "bics", "adcs", "sbcs", "rors", "negs"}


def mnemonics(path: pathlib.Path) -> collections.Counter:
    c: collections.Counter = collections.Counter()
    for ln in path.read_text().splitlines():
        if not ln.startswith("    ") or not ln.strip():
            continue
        m = ln.split()[0]
        for sfx in (".w", ".n"):
            if m.endswith(sfx):
                m = m[: -len(sfx)]
        if m in _FLAG_S:
            m = m[:-1]
        c[m] += 1
    return c


def names(path: pathlib.Path) -> set[str]:
    # LLVM's per-build `.llvm.<n>` suffix is not a different function.
    if not path.exists():
        return set()
    return {re.sub(r"\.llvm\.\d+$", "", ln.split(None, 1)[1])
            for ln in path.read_text().splitlines() if " " in ln}


_WORK = ("bl", "blx", "dmb", "dsb", "isb", "cpsid", "cpsie", "msr")


# APB/AHB1, GPIO (AHB2 on F0/L4/G4), ADC/IOPORT (0x5000_0000), and the
# system-control space. Not the whole 0x4000_0000 region: jump-table words
# padded with the Thumb nop 0x46c0 land there.
_PERIPH = ((0x4000_0000, 0x4010_0000), (0x4800_0000, 0x4801_0000),
           (0x5000_0000, 0x5007_0000), (0xE000_0000, 0xE010_0000))


def _peripheral(v: int) -> bool:
    return any(lo <= v < hi for lo, hi in _PERIPH)


def constants(path: pathlib.Path) -> set[str]:
    """Peripheral / system-control addresses the root touches: v6-M literal
    words and v7-M movt high halves. Field offsets, literal-pool and
    jump-table words move with any layout change and are not work."""
    c: set[str] = set()
    for ln in path.read_text().splitlines():
        parts = ln.split()
        if len(parts) < 2:
            continue
        if parts[0] == ".word" and parts[1] != "ADDR" and _peripheral(int(parts[1], 16)):
            c.add(parts[1])
        elif parts[0] == "movt" and parts[-1].startswith("#") and parts[-1] != "#ADDR":
            hi = int(parts[-1][1:], 0)
            if _peripheral(hi << 16):
                c.add(f"0x{hi:04x}xxxx")
    return c


def cmd_removals(a) -> int:
    da, db = pathlib.Path(a.a), pathlib.Path(a.b)
    ra = set((da / "roots.txt").read_text().split())
    rb = set((db / "roots.txt").read_text().split())
    bad = False
    for r in sorted(rb - ra):
        print(f"root {r}: NEW root")
        bad = True
    for r in sorted(ra - rb):
        print(f"root {r}: removed")
    for r in sorted(ra & rb):
        x, y = mnemonics(da / f"{r}.s"), mnemonics(db / f"{r}.s")
        tx, ty = sum(x.values()), sum(y.values())
        fails = []
        if ty > tx:
            fails.append(f"instructions {tx} -> {ty}")
        for m in sorted(y):
            if (m in _WORK or m.startswith(("ldrex", "strex"))) and y[m] > x[m]:
                fails.append(f"{m}: {x[m]} -> {y[m]}")
        for f in sorted(names(db / f"{r}.names") - names(da / f"{r}.names")):
            fails.append(f"new function: {f}")
        cx, cy = constants(da / f"{r}.s"), constants(db / f"{r}.s")
        for k in sorted(set(cy) - set(cx)):
            fails.append(f"new constant: {k}")
        verdict = "same" if x == y and not fails else ("removals only" if not fails else "ADDS")
        print(f"root {r}: {tx} -> {ty} instructions ({ty - tx:+d}), {verdict}")
        for f in fails:
            print(f"    {f}")
        noise = {m: (x[m], y[m]) for m in y if y[m] > x[m] and not
                 (m in _WORK or m.startswith(("ldrex", "strex")))}
        if noise and a.verbose:
            print("    re-allocation: " + ", ".join(f"{m} {o}->{n}" for m, (o, n) in sorted(noise.items())))
        bad |= bool(fails)
    print("ISR removals: " + ("FAIL" if bad else "PASS"))
    return 1 if bad else 0


def main() -> int:
    ap = argparse.ArgumentParser()
    sp = ap.add_subparsers(dest="cmd", required=True)
    d = sp.add_parser("dump")
    d.add_argument("--elf", required=True)
    d.add_argument("--out", required=True)
    f = sp.add_parser("diff")
    f.add_argument("a")
    f.add_argument("b")
    f.add_argument("--summary", action="store_true", help="per-root counts only")
    rm = sp.add_parser("removals")
    rm.add_argument("a")
    rm.add_argument("b")
    rm.add_argument("-v", "--verbose", action="store_true", help="print re-allocation noise")
    a = ap.parse_args()
    return {"dump": cmd_dump, "diff": cmd_diff, "removals": cmd_removals}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
