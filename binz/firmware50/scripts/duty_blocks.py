#!/usr/bin/env python3
"""ENV-87: block statistics restricted to one applied duty (block ring v3: SAGBLK sum bus n half duty).

    python scripts/duty_blocks.py DUTY CAPTURE...
"""
import re, statistics as st, sys
D = int(sys.argv[1])
for p in sys.argv[2:]:
    t = open(p, errors="replace").read()
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow = g("zero_start"), g("ma_allow")
    rows = [[int(v) for v in l.split()[1:]] for l in t.splitlines() if l.startswith("SAGBLK ")]
    x = [(zero - r[0]) * 5000 / allow for r in rows if len(r) == 5 and r[4] == D and r[2] == 100]
    name = p.replace("\\", "/").split("/")[-1]
    if len(x) < 50:
        print(f"{name}: REFUSED ({len(x)} blocks at duty {D})")
        continue
    m = st.mean(x); v = sum((a - m) ** 2 for a in x)
    a1 = sum((x[i] - m) * (x[i + 1] - m) for i in range(len(x) - 1)) / v
    print(f"{name}: {len(x)} blocks at {D} | mean {m:.0f} sd {st.pstdev(x):.0f} max {max(x):.0f} "
          f">=4750: {sum(1 for a in x if a >= 4750)} | acf1 {a1:+.2f} | ceiling {g('ceiling_tenths')} worst {g('worst_ma')}")
