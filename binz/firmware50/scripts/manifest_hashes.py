#!/usr/bin/env python3
"""Inventory every preserved image and capture, with size and SHA-256.

`captures/` is preserved on disk but not committed -- the images alone are
114 MB. This writes `captures/MANIFEST-hashes.txt`, which *is* committed, so
an uncommitted artefact can still be identified, checked for tampering, or
matched to the entry that cites it.

Each capture's own first line records the hash of the image that produced it
(`# elf_sha256 ...`), so image and capture can be tied together from the
inventory alone.

Usage:  python scripts/manifest_hashes.py [--check]

`--check` re-hashes and reports anything added, removed or changed since the
inventory was written, and exits non-zero if so.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import pathlib
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
OUT = REPO / "captures" / "MANIFEST-hashes.txt"
# Directories worth inventorying: images, run captures, and the derived
# artefacts entries cite. `__pycache__` and the fixture's mutable ladder state
# are excluded -- the first is noise, the second changes with every run.
SKIP = {"__pycache__"}
# The fixture rewrites this on every run, so a `--check` would report it as
# CHANGED for a reason unrelated to tampering (E174).
SKIP_FILES = {"ladder_state.json"}


def entries() -> list[tuple[str, int, str]]:
    rows = []
    for p in sorted((REPO / "captures").rglob("*")):
        if not p.is_file() or p.name == OUT.name:
            continue
        if any(part in SKIP for part in p.parts) or p.name in SKIP_FILES:
            continue
        h = hashlib.sha256(p.read_bytes()).hexdigest().upper()
        rows.append((p.relative_to(REPO).as_posix(), p.stat().st_size, h))
    return rows


def render(rows: list[tuple[str, int, str]]) -> str:
    total = sum(n for _, n, _ in rows)
    head = (
        f"# firmware50 preserved artefacts: {len(rows)} files, {total / 1e6:.1f} MB\n"
        f"# written {datetime.date.today().isoformat()} by scripts/manifest_hashes.py\n"
        "# path<TAB>bytes<TAB>sha256\n"
    )
    return head + "".join(f"{p}\t{n}\t{h}\n" for p, n, h in rows)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="compare instead of writing")
    a = ap.parse_args()
    rows = entries()
    if not a.check:
        OUT.write_text(render(rows), encoding="utf-8", newline="\n")
        print(f"{len(rows)} artefacts inventoried -> {OUT.relative_to(REPO).as_posix()}")
        return 0
    if not OUT.exists():
        print("no inventory to check against")
        return 1
    was = {}
    for line in OUT.read_text(encoding="utf-8").splitlines():
        if line.startswith("#") or not line.strip():
            continue
        p, n, h = line.split("\t")
        was[p] = (int(n), h)
    now = {p: (n, h) for p, n, h in rows}
    bad = 0
    for p in sorted(set(was) | set(now)):
        if p not in now:
            print(f"MISSING  {p}")
            bad += 1
        elif p not in was:
            print(f"added    {p}")
        elif was[p] != now[p]:
            print(f"CHANGED  {p}")
            bad += 1
    print(f"{len(now)} artefacts, {bad} missing or changed")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
