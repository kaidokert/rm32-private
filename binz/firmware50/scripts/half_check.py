#!/usr/bin/env python3
"""ENV-79: split-half reliability of the per-block current (block ring v2: SAGBLK sum bus n half).

    python scripts/half_check.py CAPTURE...

Half-block mA: h1 = (zero/2 - (half << 3)) * 5000 / (ma_allow/2) over scans 1..50, h2 from the rest (sum - half<<3)
over scans 51..100 (blocks with n != 100 dropped). r = corr(h1, h2) across blocks. Verdict (predeclared): (S) sampling
if r <= 0.2; (R) real if r >= 0.5; else mixed (r^2 ~ real share of block variance). Refuses rings without the half column.
"""
import re, statistics as st, sys


def corr(x, y):
    mx, my = st.mean(x), st.mean(y)
    sx = sum((a - mx) ** 2 for a in x) ** 0.5
    sy = sum((b - my) ** 2 for b in y) ** 0.5
    return sum((a - mx) * (b - my) for a, b in zip(x, y)) / (sx * sy)


for p in sys.argv[1:]:
    t = open(p, errors="replace").read()
    name = p.replace("\\", "/").split("/")[-1]
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow = g("zero_start"), g("ma_allow")
    rows = [[int(v) for v in l.split()[1:]] for l in t.splitlines() if l.startswith("SAGBLK ")]
    if not rows or len(rows[0]) != 4 or zero is None:
        print(f"{name}: REFUSED (no half column / calibration)")
        continue
    h1, h2, blk = [], [], []
    for s, b, n, h in rows:
        if n != 100:
            continue
        a = h << 3
        h1.append((zero / 2 - a) * 5000 / (allow / 2))
        h2.append((zero / 2 - (s - a)) * 5000 / (allow / 2))
        blk.append((zero - s) * 5000 / allow)
    r = corr(h1, h2)
    v = "S (sampling)" if r <= 0.2 else ("R (real)" if r >= 0.5 else "mixed")
    print(f"{name}: blocks {len(blk)} | block mA mean {st.mean(blk):.0f} sd {st.pstdev(blk):.0f} max {max(blk):.0f} | "
          f"half sd {st.pstdev(h1):.0f}/{st.pstdev(h2):.0f} | r(halves) {r:+.2f} (r2 {max(r,0)**2:.2f}) -> {v}")
