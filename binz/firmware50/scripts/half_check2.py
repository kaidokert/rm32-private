#!/usr/bin/env python3
"""ENV-88: split-half reliability, detrended, on block ring v3 with `blk-half` (SAGBLK sum bus n half 0).

    python scripts/half_check2.py CAPTURE...

h1 = first 50 scans' mA, h2 = last 50's (as half_check.py). Each series is detrended by subtracting its centred
11-block moving mean (removes the ramp and any slow drift), then r = corr(h1', h2'). Verdict (predeclared):
(S) sampling if r <= 0.2; (R) real if r >= 0.5; else mixed. Also reports the raw r and the block sd.
"""
import re, statistics as st, sys


def corr(x, y):
    mx, my = st.mean(x), st.mean(y)
    sx = sum((a - mx) ** 2 for a in x) ** 0.5
    sy = sum((b - my) ** 2 for b in y) ** 0.5
    return sum((a - mx) * (b - my) for a, b in zip(x, y)) / (sx * sy)


def detrend(x, w=5):
    return [x[i] - st.mean(x[i - w:i + w + 1]) for i in range(w, len(x) - w)]


for p in sys.argv[1:]:
    t = open(p, errors="replace").read()
    name = p.replace("\\", "/").split("/")[-1]
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow = g("zero_start"), g("ma_allow")
    rows = [[int(v) for v in l.split()[1:]] for l in t.splitlines() if l.startswith("SAGBLK ")]
    if not rows or len(rows[0]) != 5 or zero is None:
        print(f"{name}: REFUSED")
        continue
    h1, h2, blk = [], [], []
    for s, b, n, h, _ in rows:
        if n != 100:
            continue
        a = h << 3
        h1.append((zero / 2 - a) * 5000 / (allow / 2))
        h2.append((zero / 2 - (s - a)) * 5000 / (allow / 2))
        blk.append((zero - s) * 5000 / allow)
    r_raw = corr(h1, h2)
    r = corr(detrend(h1), detrend(h2))
    v = "S (sampling)" if r <= 0.2 else ("R (real)" if r >= 0.5 else "mixed")
    print(f"{name}: blocks {len(blk)} | block sd {st.pstdev(blk):.0f} detrended-block sd {st.pstdev(detrend(blk)):.0f} "
          f"max {max(blk):.0f} | r raw {r_raw:+.2f}, r detrended {r:+.2f} -> {v} | ceiling {g('ceiling_tenths')}")
