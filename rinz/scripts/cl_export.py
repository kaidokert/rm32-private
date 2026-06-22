#!/usr/bin/env python3
"""Export captured BEMF frames + the per-sector oracle for the Rust cl_replay harness.

The closed-loop tracker lives in Rust (`rinz::cl`, the identical code the firmware ISR
runs). This script does the capture parsing (which already lives in scope_common) and
computes the offline multi-harmonic oracle, then writes a trivial text file the Rust
harness reads — so the Rust side stays a thin replay loop and the b85/hex/header
parsing isn't duplicated. The oracle travels with the frames so scoring is a column
compare (see cl_score.py).

    python scripts/cl_export.py logs/sweep_<ts> -o logs/cl_frames.txt
"""

from __future__ import annotations

import argparse
from pathlib import Path

from zc_validate import load_captures
from scope_cl_validate import oracle_crossings


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("paths", nargs="+", type=Path)
    ap.add_argument("-o", "--out", type=Path, default=Path("logs/cl_frames.txt"))
    args = ap.parse_args()

    caps = load_captures(args.paths)
    n = 0
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("w", encoding="ascii") as f:
        for ci, cap in enumerate(caps):
            try:
                hz = float(cap.debug.get("hz", "0") or 0)
            except ValueError:
                continue
            if hz <= 0 or len(cap.channels) < 3 or cap.interleaved:
                continue
            orc = oracle_crossings(cap)
            frames = min(len(c) for c in cap.channels)
            f.write(f"CAP idx={ci} hz={hz:.0f} sample_hz={cap.sample_hz:.0f} frames={frames}\n")
            f.write("ORACLE " + " ".join(
                f"{(orc.get(s) if orc.get(s) is not None else -1.0):.1f}" for s in range(6)
            ) + "\n")
            for i in range(frames):
                f.write(f"{cap.channels[0][i]} {cap.channels[1][i]} {cap.channels[2][i]}\n")
            n += 1
    print(f"exported {n} captures -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
