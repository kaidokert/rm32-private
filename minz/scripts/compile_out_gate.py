#!/usr/bin/env python3
r"""Wide-arithmetic compile-out gate for the krabilorean M4 qualification.

Scans an ELF for the AEABI / compiler-rt runtime helper routines that a
personality's codegen would call if wide (64/128-bit) multiply, divide, or
shift were NOT folded away. A "32-bit personality claims no wide helpers"
becomes a checkable assertion: run the gate on a binary whose reachable call
graph is JUST that personality's hot path (a per-personality micro-lib), and
require the 64/128-bit helper set to be empty.

Usage:
  python scripts/compile_out_gate.py <elf> [--forbid 64|128|all] [--json]

Detection is by symbol presence (nm) AND by `bl <helper>` call sites in the
disassembly (objdump -d), so a helper that is linked-but-unreachable (defined
in .text yet never called) is reported separately from one actually called.
"""
import argparse
import json
import re
import shutil
import subprocess
import sys

# 64-bit (doubleword) EABI + compiler-rt helpers
HELPERS_64 = [
    "__aeabi_ldivmod", "__aeabi_uldivmod", "__aeabi_lmul",
    "__aeabi_llsl", "__aeabi_llsr", "__aeabi_lasr",
    "__aeabi_lcmp", "__aeabi_ulcmp",
    "__divdi3", "__udivdi3", "__moddi3", "__umoddi3", "__muldi3",
    "__ashldi3", "__lshrdi3", "__ashrdi3", "__divmoddi4", "__udivmoddi4",
]
# 128-bit (tetraword) compiler-rt helpers — the u128 tell
HELPERS_128 = [
    "__udivti3", "__divti3", "__modti3", "__umodti3", "__multi3",
    "__ashlti3", "__lshrti3", "__ashrti3", "__udivmodti4", "__divmodti4",
    "__multf3", "__muloti4", "__umulti3",
]


def tool(name):
    for cand in (f"arm-none-eabi-{name}", f"rust-{name}", name):
        if shutil.which(cand):
            return cand
    sys.exit(f"no {name} in PATH (need arm-none-eabi-{name} or rust-{name})")


def defined_and_undefined(elf):
    """Return set of all symbol names referenced (defined U/T/t or undefined U)."""
    out = subprocess.run([tool("nm"), elf], capture_output=True, text=True).stdout
    names = set()
    for line in out.splitlines():
        parts = line.split()
        if not parts:
            continue
        names.add(parts[-1])
    return names


def called_helpers(elf, helpers):
    """objdump -d and find `bl`/`b` targets naming a helper."""
    out = subprocess.run([tool("objdump"), "-d", elf], capture_output=True, text=True).stdout
    called = {}
    # match: '...  bl  <addr> <__helper>' or '<__helper+0x..>'
    pat = re.compile(r"\b(?:bl|b|blx)\b.*<([A-Za-z_][A-Za-z0-9_]*)")
    for m in pat.finditer(out):
        sym = m.group(1)
        base = sym.split("+")[0]
        if base in helpers:
            called[base] = called.get(base, 0) + 1
    return called


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("elf")
    ap.add_argument("--forbid", choices=["64", "128", "all"], default="all")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    forbid = {"64": HELPERS_64, "128": HELPERS_128, "all": HELPERS_64 + HELPERS_128}[a.forbid]
    syms = defined_and_undefined(a.elf)
    present = sorted(h for h in forbid if h in syms)
    called = called_helpers(a.elf, set(forbid))

    result = {
        "elf": a.elf,
        "forbid_class": a.forbid,
        "present_symbols": present,
        "called": called,
        "present_64": sorted(h for h in HELPERS_64 if h in syms),
        "present_128": sorted(h for h in HELPERS_128 if h in syms),
        "clean": len(present) == 0,
    }
    if a.json:
        print(json.dumps(result, indent=2))
    else:
        tag = a.elf.split("/")[-1]
        if result["clean"]:
            print(f"[CLEAN] {tag}: no forbidden {a.forbid}-bit helpers present")
        else:
            print(f"[WIDE]  {tag}: {len(present)} forbidden helper(s) present")
            for h in present:
                c = called.get(h)
                where = f"called x{c}" if c else "linked, no direct bl seen"
                print(f"          {h:<20} {where}")
    sys.exit(0 if result["clean"] else 1)


if __name__ == "__main__":
    main()
