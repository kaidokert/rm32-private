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
"""

from __future__ import annotations

import argparse
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
    a = ap.parse_args()
    return cmd_dump(a) if a.cmd == "dump" else cmd_diff(a)


if __name__ == "__main__":
    sys.exit(main())
