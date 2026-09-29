#!/usr/bin/env python3
"""Print the listing lines (source + instructions) leading up to an address: python scripts/isr_ctx.py LISTING HEXADDR [N]"""
import sys
lst, addr = sys.argv[1], sys.argv[2].lower().lstrip("0")
n = int(sys.argv[3]) if len(sys.argv) > 3 else 24
lines = open(lst, encoding="utf-8", errors="replace").read().splitlines()
for i, l in enumerate(lines):
    if l.strip().startswith(addr + ":"):
        print("\n".join(lines[max(0, i - n):i + 3]))
        break
else:
    sys.exit("address not found")
