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
    # **Every limit gates, not just the RAM ceiling.** Until E198 this returned
    # only `bss_ceiling()`'s verdict, so a violated structure limit printed its
    # number, exited 0, and was reported as a pass -- which is how a 103-line
    # function reached a qualified image and an archived gate file with nobody
    # noticing (the number was in `captures/gates/e187-gates.txt:120` all
    # along). A gate whose exit code ignores its own findings is not a gate.
    breaches = []
    if len(longs):
        breaches.append(f"{len(longs)} function(s) over 100 lines")
    if len(code_lines(bin_text)) >= 1500:
        breaches.append("bin over 1500 lines")
    if unsafe_bin > 10:
        breaches.append(f"{unsafe_bin} unsafe in bin")
    if bits_bin or bits_src:
        breaches.append("register writes outside hw/")
    if sm_bin or sm_src:
        breaches.append("static mut")
    rc = bss_ceiling()
    if breaches:
        print("STRUCTURE LIMITS BREACHED: " + "; ".join(breaches))
        rc = 1
    return rc


# The G071 has 36 KB of RAM and no stack guard: when static data grows, the
# stack silently overlaps it. E185 lost an evening to exactly that -- E180's
# wider chain `Beat` left under 4 KB for a stack whose largest single frame
# (`Controller::run`) reserves 5076 B, so SP descended *into* the ring the COM
# root was writing, and the deaths looked like a timer-register fault.
#
# The model is `RAM - (.data + .bss)`. E185's version omitted `.data`
# (644-660 B in every image), i.e. it was optimistic by ~660 B about the exact
# quantity that had just cost an evening (E186 SS5).
RAM_BYTES = 36 * 1024
STACK_FLOOR = 8 * 1024
# The largest single stack frame in the tree, measured from the disassembly
# (`Controller::run`'s prologue). Reported beside the headroom so the margin is
# a number rather than a hope; not a bound -- callees and four ISR roots sit on
# top of it.
LARGEST_FRAME = 5076


def bss_ceiling() -> int:
    """Check the built images *and* the archived ones against the stack floor."""
    import shutil
    import subprocess

    size = shutil.which("arm-none-eabi-size")
    if size is None:
        print("bss_headroom: FAIL -- arm-none-eabi-size not found, so the RAM "
              "ceiling could not be checked at all")
        return 1
    out = pathlib.Path("target/thumbv6m-none-eabi/release")
    # Cargo's own lock files live in the same directory and have no suffix.
    built = (
        [
            p
            for p in sorted(out.glob("*"))
            if p.is_file() and not p.suffix and not p.name.startswith(".")
        ]
        if out.is_dir()
        else []
    )
    # The archived images are what actually gets flashed (`--elf captures/elf/..
    # --flash`), so they are checked too -- but a historical image cannot be
    # made to pass retroactively, so they are reported and named unflashable
    # rather than failing the build. Only the current images gate.
    archived = sorted(pathlib.Path("captures/elf").glob("*.elf"))
    elfs = [(p, True) for p in built] + [(p, False) for p in archived]

    # **A gate that can pass by finding nothing is not a gate** (E268). This
    # used to fail only when built *and* archived were both empty -- and
    # `captures/elf/` is never empty, so on a clean tree it checked zero
    # current images and still returned 0. Only the built images gate; the
    # archived ones are reported and cannot be made to pass retroactively. So
    # the coverage this check actually achieved was invisible in its verdict.
    #
    # Now: the current binaries must be present, and any that are missing are
    # named. This is the crate's own instrument-must-fail-loudly rule applied
    # to the instrument that enforces the RAM ceiling -- which is the one that
    # E185's sub-millisecond deaths were traced to.
    want = {"shell-pwm", "edge-capture", "chain-capture", "sag-capture"}
    have = {p.name for p in built}
    missing = sorted(want - have)
    if missing:
        print(
            "bss_headroom: FAIL -- these images are not built, so the RAM "
            f"ceiling gated none of them: {', '.join(missing)}. "
            "Run `cargo build --release` first; a clean tree would otherwise "
            "pass this check without measuring anything."
        )
        return 1
    if not elfs:
        print("bss_headroom: FAIL -- nothing built and nothing archived to check")
        return 1
    bad = 0
    unflashable = []
    for elf, gates in elfs:
        r = subprocess.run([size, str(elf)], capture_output=True, text=True)
        rows = [ln.split() for ln in r.stdout.strip().splitlines()[1:]]
        if not rows or len(rows[0]) < 3:
            print(f"bss_headroom: FAIL -- could not size {elf.name}")
            bad += 1
            continue
        data, bss = int(rows[0][1]), int(rows[0][2])
        left = RAM_BYTES - data - bss
        flag = "" if left >= STACK_FLOOR else "  <-- BELOW THE STACK FLOOR"
        if left < LARGEST_FRAME and not flag:
            flag = "  <-- BELOW THE LARGEST KNOWN FRAME"
        print(
            f"bss_headroom: {elf.name:34s} data={data:5d} bss={bss:6d} "
            f"stack_left={left:6d} (floor {STACK_FLOOR}, largest frame {LARGEST_FRAME}){flag}"
        )
        if left < STACK_FLOOR:
            if gates:
                bad += 1
            else:
                unflashable.append(elf.name)
    if unflashable:
        print(
            f"bss_headroom: {len(unflashable)} ARCHIVED image(s) are below the stack "
            "floor and MUST NOT BE FLASHED again:"
        )
        for name in unflashable:
            print(f"  {name}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
