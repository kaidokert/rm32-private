#!/usr/bin/env python3
"""Portability gate (design §9): the portable core names no STM32 peripheral.

Scans every .rs file under rm32/src. An STM32 name (timer, comparator,
EXTI, DMA, UART, ADC instance, GPIO pin, chip family, ...) may appear
only in a comment, and that comment must cite AM32 (the comment block it
sits in mentions "AM32"). Anywhere else -- code, string literals, or a
comment that is not an AM32 citation -- is a violation.

Prints each violation as file:line: text and exits 1 if there are any.
`--stats` also prints how many AM32-citing comment mentions were allowed.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORE = ROOT / "rm32" / "src"

# Peripheral instance and chip names. Generic words (timer, comparator)
# are fine; numbered instances and STM32 pin names are not.
NAME = re.compile(
    r"\b("
    r"TIM[0-9]{1,2}|COMP[0-9]|EXTI[0-9_]*|DMA[0-9](_CH[0-9])?|"
    r"US?ART[0-9]|LPUART[0-9]|ADC[0-9]|GPIO[A-H]|"
    r"NVIC|IWDG|RCC|SYSCFG|FLASH_ACR|"
    r"P[A-F][0-9]{1,2}|"
    r"STM32\w*|stm32\w*|[LGFH][0-9]{3}[A-Z0-9]*"
    r")\b"
)
AM32 = re.compile(r"AM32")


def split_comment(line, in_block):
    """Return (code, comment, in_block) for one source line.

    Handles //, /* */ (one level), and string literals well enough for
    this tree (no raw strings containing comment markers)."""
    code, comment = [], []
    i, n = 0, len(line)
    in_str = False
    while i < n:
        if in_block:
            end = line.find("*/", i)
            if end < 0:
                comment.append(line[i:])
                return "".join(code), "".join(comment), True
            comment.append(line[i:end])
            i = end + 2
            in_block = False
            continue
        c = line[i]
        if in_str:
            code.append(c)
            if c == "\\" and i + 1 < n:
                code.append(line[i + 1])
                i += 2
                continue
            if c == '"':
                in_str = False
            i += 1
            continue
        if c == '"':
            in_str = True
            code.append(c)
            i += 1
            continue
        if line.startswith("//", i):
            comment.append(line[i:])
            break
        if line.startswith("/*", i):
            in_block = True
            i += 2
            continue
        code.append(c)
        i += 1
    return "".join(code), "".join(comment), in_block


def scan(path):
    """Yield (lineno, kind, text) violations; kind = code | comment.

    A comment line is an AM32 citation when its contiguous comment block
    (adjacent comment-only lines, or the trailing comment itself)
    mentions AM32."""
    lines = path.read_text(encoding="utf-8").splitlines()
    parsed = []
    in_block = False
    for line in lines:
        code, comment, in_block = split_comment(line, in_block)
        parsed.append((code, comment))
    # Comment blocks: runs of lines with no code and some comment.
    block_cites = [False] * len(parsed)
    i = 0
    while i < len(parsed):
        code, comment = parsed[i]
        if comment and not code.strip():
            j = i
            while j < len(parsed) and parsed[j][1] and not parsed[j][0].strip():
                j += 1
            cites = any(AM32.search(parsed[k][1]) for k in range(i, j))
            for k in range(i, j):
                block_cites[k] = cites
            i = j
        else:
            block_cites[i] = bool(comment and AM32.search(comment))
            i += 1
    allowed = 0
    for idx, (code, comment) in enumerate(parsed):
        if NAME.search(code):
            yield idx + 1, "code", lines[idx].strip()
        if comment and NAME.search(comment):
            if block_cites[idx]:
                allowed += 1
            else:
                yield idx + 1, "comment", lines[idx].strip()
    scan.allowed += allowed


scan.allowed = 0


SELFTEST = """let t = TIM6; // bad: code
let s = "PA2"; // bad: string literal in code
/// Ticks the TIM6 handler. (bad: comment without citation)
fn tick() {}
// AM32 Src/main.c: tenKhzRoutine runs on TIM6 (ok: cites)
// continued block, COMP2 still ok
let x = 1; /* ok: AM32's COMP2 */
let timer = 5; // ok: no peripheral names
"""
SELFTEST_BAD = {1, 2, 3}


def selftest(tmp):
    """The gate must flag exactly the planted lines, or it is not a gate."""
    f = tmp / "selftest.rs"
    f.write_text(SELFTEST, encoding="utf-8")
    got = {lineno for lineno, _, _ in scan(f)}
    scan.allowed = 0
    if got != SELFTEST_BAD:
        print(f"stm32_name_gate: SELFTEST FAILED: flagged {sorted(got)}, "
              f"expected {sorted(SELFTEST_BAD)}", file=sys.stderr)
        return False
    return True


def main():
    import tempfile
    with tempfile.TemporaryDirectory() as d:
        if not selftest(Path(d)):
            return 2
    files = sorted(CORE.rglob("*.rs"))
    if not files:
        print(f"stm32_name_gate: no sources under {CORE}", file=sys.stderr)
        return 1
    bad = []
    for f in files:
        for lineno, kind, text in scan(f):
            bad.append(f"{f.relative_to(ROOT).as_posix()}:{lineno}: [{kind}] {text}")
    for b in bad:
        print(b)
    if "--stats" in sys.argv:
        print(f"files={len(files)} violations={len(bad)} am32_citations={scan.allowed}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
