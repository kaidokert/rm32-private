#!/usr/bin/env python3
"""Issue #7 side-by-side table: the `[run]` summary of every captured
bf_ladder / bf_slam log, per board, with PASS/CHECK by the scripts' own
rule. Feeds the HTML comparison report.

Usage: issue7_table.py [DIR]   (default captures/gc/issue7)
       logs named <board>_<ladder|slam>_<n>.log (bf_* --esclog)
Exits 1 if any log lacks a [run] line (never guesses a row).
"""
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from fc_bench import parse_run  # noqa: E402

NAME_RE = re.compile(r"^(l431|binz)_(ladder|slam)_(\d+)\.log$")
MIN_COMMS = {"ladder": 500_000, "slam": 100_000}


def main():
    d = Path(sys.argv[1] if len(sys.argv) > 1 else "captures/gc/issue7")
    rows, bad = [], []
    for f in sorted(d.glob("*.log")):
        m = NAME_RE.match(f.name)
        if not m:
            continue
        text = f.read_text(encoding="ascii", errors="replace")
        runs = [r for r in map(parse_run, text.splitlines()) if r]
        resets = sum(1 for ln in text.splitlines()
                     if "RESET:" in ln or "] boot" in ln or "last reset:" in ln)
        if not runs:
            bad.append(f.name)
            continue
        r = runs[-1]
        test = m.group(2)
        ok = r["starts"] == 1 and r["dsy"] == 0 and r["comms"] > MIN_COMMS[test]
        rows.append({"board": m.group(1), "test": test, "n": int(m.group(3)),
                     "pass": ok, "log_resets_whole_file": resets, **r})
    print(json.dumps(rows, indent=1))
    if bad:
        print(f"no [run] line in: {bad}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
