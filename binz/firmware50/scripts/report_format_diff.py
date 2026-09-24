#!/usr/bin/env python3
"""Compare two captures' report *format*: the sequence of line prefixes and,
per line, the sequence of keys. Values may differ; the shape may not. Used to
show a telemetry refactor (E117 onward) left the fixture's contract intact.

Usage: python scripts/report_format_diff.py OLD.txt NEW.txt
"""

import pathlib
import sys


def shape(path: pathlib.Path) -> list[tuple[str, tuple[str, ...]]]:
    out = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        toks = line.split()
        if not toks:
            continue
        head = toks[0] if "=" not in toks[0] else ""
        keys = tuple(t.split("=", 1)[0] for t in toks if "=" in t)
        if head.isupper() or keys:
            out.append((head, keys))
    return out


def main() -> int:
    a, b = (pathlib.Path(p) for p in sys.argv[1:3])
    sa, sb = shape(a), shape(b)
    if sa == sb:
        print(f"SAME FORMAT: {len(sa)} lines")
        return 0
    for i, (x, y) in enumerate(zip(sa, sb)):
        if x != y:
            print(f"line {i}: {x}\n     vs {y}")
            return 1
    print(f"length differs: {len(sa)} vs {len(sb)}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
