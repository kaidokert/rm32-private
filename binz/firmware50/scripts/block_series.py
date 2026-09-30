#!/usr/bin/env python3
"""ENV-77: the per-block current series from a `sag-capture` dump with the block ring (SAGBLK rows).

    python scripts/block_series.py CAPTURE...

Per-block mA = (zero_start - shunt_sum) * 5000 / ma_allow: the firmware's own AverageCurrent block value. Blocks
whose recorder saw n != 100 scans are dropped (misaligned). Reports mean, sd, max, the autocorrelation at lags 1..50
and corr(block mA, block bus mean). Verdict (predeclared): (A) aliasing if corr > -0.2 and some lag 2..50 has
autocorrelation >= 0.4; (R) real if corr <= -0.5; else undecided. Refuses captures without SAGBLK rows or calibration.
"""
import re, statistics as st, sys


def acf(x, lag):
    m = st.mean(x)
    v = sum((a - m) ** 2 for a in x)
    return sum((x[i] - m) * (x[i + lag] - m) for i in range(len(x) - lag)) / v if v else 0.0


def corr(x, y):
    mx, my = st.mean(x), st.mean(y)
    sx = sum((a - mx) ** 2 for a in x) ** 0.5
    sy = sum((b - my) ** 2 for b in y) ** 0.5
    return sum((a - mx) * (b - my) for a, b in zip(x, y)) / (sx * sy) if sx and sy else 0.0


for p in sys.argv[1:]:
    t = open(p, errors="replace").read()
    name = p.replace("\\", "/").split("/")[-1]
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow = g("zero_start"), g("ma_allow")
    rows = [tuple(int(v) for v in l.split()[1:4]) for l in t.splitlines() if l.startswith("SAGBLK ")]
    if not rows or zero is None or allow is None:
        print(f"{name}: REFUSED (rows {len(rows)}, zero {zero}, allow {allow})")
        continue
    good = [(s, b) for s, b, n in rows if n == 100]
    ma = [(zero - s) * 5000 / allow for s, _ in good]
    bus = [b for _, b in good]
    if len(ma) < 200:
        print(f"{name}: REFUSED (only {len(ma)} aligned blocks of {len(rows)})")
        continue
    ac = [(lag, acf(ma, lag)) for lag in range(1, 51)]
    pk = max(ac[1:], key=lambda a: a[1])
    c = corr(ma, bus)
    srt = sorted(ma)
    p99 = srt[int(0.99 * len(srt))]
    v = "A (aliasing)" if c > -0.2 and pk[1] >= 0.4 else ("R (real)" if c <= -0.5 else "undecided")
    print(f"{name}: blocks {len(ma)}/{len(rows)} | mA mean {st.mean(ma):.0f} sd {st.pstdev(ma):.0f} p99 {p99:.0f} "
          f"max {max(ma):.0f} | acf lag1 {ac[0][1]:+.2f}, peak lag {pk[0]} {pk[1]:+.2f} | corr(mA,bus) {c:+.2f} -> {v}")
    top = sorted(range(len(ma)), key=lambda i: -ma[i])[:8]
    print("   top blocks (index: mA, bus, neighbours -1/+1):",
          [(i, round(ma[i]), bus[i], round(ma[i - 1]) if i else None, round(ma[i + 1]) if i + 1 < len(ma) else None)
           for i in sorted(top)])
