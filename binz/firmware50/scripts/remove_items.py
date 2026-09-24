#!/usr/bin/env python3
"""Remove whole top-level Rust items (fn / const / static / struct / enum /
impl-less type) by name from a source file, together with the doc comments,
line comments and attributes directly above them (up to the previous blank
line). Used for the E110 probe retirement, driven by the compiler's own
dead-code list, so nothing live can be removed without the build noticing.

Usage:
    python scripts/remove_items.py FILE NAME [NAME ...]
"""

from __future__ import annotations

import re
import sys

ITEM = r"^(pub(\([^)]*\))?\s+)?(const\s+fn|fn|const|static(\s+mut)?|struct|enum|type)\s+{name}\b"


def item_span(lines: list[str], name: str) -> tuple[int, int] | None:
    pat = re.compile(ITEM.format(name=re.escape(name)))
    for i, line in enumerate(lines):
        if pat.match(line):
            break
    else:
        return None
    # Walk back over attributes and comments that belong to the item.
    start = i
    while start > 0 and re.match(r"^\s*(///|//|#\[|#!\[)", lines[start - 1]):
        start -= 1
    # Walk forward to the end of the item.
    depth = 0
    opened = False
    j = i
    while j < len(lines):
        s = re.sub(r"//.*", "", lines[j])
        s = re.sub(r"'(\\.|[^'\\])'", "''", s)
        s = re.sub(r'"(\\.|[^"\\])*"', '""', s)
        for ch in s:
            if ch in "{[(":
                depth += 1
                opened = True
            elif ch in "}])":
                depth -= 1
        if depth == 0 and (opened and s.rstrip().endswith(("}", ";")) or s.rstrip().endswith(";")):
            break
        j += 1
    end = j
    # Swallow one following blank line to keep spacing tidy.
    if end + 1 < len(lines) and lines[end + 1].strip() == "":
        end += 1
    return start, end


def main() -> int:
    path = sys.argv[1]
    names = sys.argv[2:]
    lines = open(path, encoding="utf-8").read().split("\n")
    for name in names:
        span = item_span(lines, name)
        if span is None:
            print(f"not found: {name}")
            continue
        a, b = span
        print(f"removed {name}: lines {a + 1}-{b + 1} ({b - a + 1})")
        del lines[a : b + 1]
    open(path, "w", encoding="utf-8", newline="").write("\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
