#!/usr/bin/env python3
"""Structural metrics for the firmware50 refactor goal (notebook E109+).

Reports, for the production binary and the library:

* bin line count (goal: under 1500);
* functions longer than 100 lines (goal: none), by a brace-depth scan;
* `unsafe` occurrences in the bin outside `hw/` (goal: <= 10, each justified);
* `.bits(` register writes outside `hw/` (goal: none);
* bare `static mut` (goal: none).

It is a text scan, not a parser: good enough to track a refactor's progress
step by step, and conservative (it over-counts `unsafe` in comments).

Usage:
    python scripts/structure_report.py
"""

from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
BIN = ROOT / "bin" / "shell-pwm.rs"
# The production binary is its root file plus the modules it declares (E121:
# `bin/board.rs`, shared with the diagnostic `edge-capture`).
BIN_FILES = [BIN, ROOT / "bin" / "board.rs"]
SRC = ROOT / "src"

FN_RE = re.compile(r"^\s*(pub(\([^)]*\))?\s+)?(const\s+)?(unsafe\s+)?fn\s+(\w+)")


def code_lines(text: str) -> list[str]:
    return text.splitlines()


def long_functions(path: pathlib.Path, limit: int = 100) -> list[tuple[str, int, int]]:
    lines = code_lines(path.read_text(encoding="utf-8"))
    out = []
    i = 0
    while i < len(lines):
        m = FN_RE.match(lines[i])
        if not m:
            i += 1
            continue
        name = m.group(5)
        # find the opening brace of the body
        depth = 0
        started = False
        j = i
        while j < len(lines):
            s = re.sub(r"//.*", "", lines[j])
            s = re.sub(r'"(\\.|[^"\\])*"', '""', s)
            for ch in s:
                if ch == "{":
                    depth += 1
                    started = True
                elif ch == "}":
                    depth -= 1
            if started and depth == 0:
                break
            if not started and s.rstrip().endswith(";"):
                break  # declaration without a body
            j += 1
        length = j - i + 1
        if started and length > limit:
            out.append((name, i + 1, length))
        i = j + 1 if started else i + 1
    return out


def count(pattern: str, text: str) -> int:
    return len(re.findall(pattern, text))


def main() -> int:
    bin_text = "\n".join(p.read_text(encoding="utf-8") for p in BIN_FILES if p.exists())
    src_files = sorted(SRC.rglob("*.rs"))
    hw_files = [p for p in src_files if "hw" in p.relative_to(SRC).parts]
    non_hw_src = [p for p in src_files if p not in hw_files]

    print(f"bin_lines={len(code_lines(bin_text))} (goal < 1500)")
    unsafe_bin = count(r"\bunsafe\b", re.sub(r"//.*", "", bin_text))
    print(f"bin_unsafe={unsafe_bin} (goal <= 10)")
    # Code only: a doc sentence naming `static mut` is not a declaration.
    def code(text: str) -> str:
        return re.sub(r"//.*", "", text)

    bits_bin = count(r"\.bits\(", code(bin_text))
    bits_src = sum(count(r"\.bits\(", code(p.read_text(encoding='utf-8'))) for p in non_hw_src)
    print(f"bits_writes_outside_hw: bin={bits_bin} src={bits_src} (goal 0)")
    sm_bin = count(r"\bstatic\s+mut\b", code(bin_text))
    sm_src = sum(count(r"\bstatic\s+mut\b", code(p.read_text(encoding='utf-8'))) for p in src_files)
    print(f"static_mut: bin={sm_bin} src={sm_src} (goal 0)")
    longs = [x for p in BIN_FILES if p.exists() for x in long_functions(p)]
    for p in src_files:
        longs += [(f"{p.name}:{n}", l, ln) for (n, l, ln) in long_functions(p)]
    print(f"functions_over_100_lines={len(longs)} (goal 0)")
    for name, line, length in sorted(longs, key=lambda t: -t[2]):
        print(f"  {length:5d}  {name} (line {line})")
    rc = bss_ceiling()
    return rc


# The G071 has 36 KB of RAM and no stack guard: when `.bss` grows, the stack
# silently overlaps it. E185 lost an evening to exactly that -- E180's wider
# chain `Beat` took the chain image's `.bss` to 32 888 B, leaving under 4 KB,
# and every run of that image died in under a millisecond with a comparator
# storm while the cause looked like a timer register. The rule now fails here,
# at the desk, on any image that leaves less than this much for the stack.
RAM_BYTES = 36 * 1024
STACK_FLOOR = 8 * 1024


def bss_ceiling() -> int:
    """Check every built image's `.bss` against the stack floor."""
    import shutil
    import subprocess

    size = shutil.which("arm-none-eabi-size")
    out = pathlib.Path("target/thumbv6m-none-eabi/release")
    if size is None or not out.is_dir():
        print("bss_headroom: SKIPPED (no arm-none-eabi-size, or nothing built)")
        return 0
    bad = 0
    for elf in sorted(out.glob("*")):
        if elf.suffix or not elf.is_file():
            continue
        r = subprocess.run([size, str(elf)], capture_output=True, text=True)
        rows = [ln.split() for ln in r.stdout.strip().splitlines()[1:]]
        if not rows or len(rows[0]) < 3:
            continue
        bss = int(rows[0][2])
        left = RAM_BYTES - bss
        flag = "" if left >= STACK_FLOOR else "  <-- BELOW THE STACK FLOOR"
        print(f"bss_headroom: {elf.name:16s} bss={bss:6d} stack_left={left:6d} (floor {STACK_FLOOR}){flag}")
        bad += left < STACK_FLOOR
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
