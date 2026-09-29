#!/usr/bin/env python3
"""ENV-26: does a late crossing come from the level revisit?

Accept rows (kind 1) now carry flag bit 6 = this accept was dispatched by the foreground
level revisit (image built with `chain-origin`). Interval = coarse at_us delta (1 us, 65.5 ms
wrap, unambiguous over a ~35 ms ring). For each accept: origin (EDGE / REVISIT) and whether
its interval is LONG (> +25 % over the 6-accept running mean), SHORT (< -25 %) or normal.
Refuses a capture where no accept carries the tag (image without chain-origin)."""
import sys, statistics as st
for path in sys.argv[1:]:
    A = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CHAIN ") and l.split()[1] == "1"]
    at = [a[1] for a in A]; flag = [a[7] for a in A]; step = [a[6] for a in A]
    iv = [((b - a) & 0xFFFF) for a, b in zip(at, at[1:])]
    pad = [a[8] if len(a) > 8 else None for a in A]
    rev = [(f & 0x40) != 0 for f in flag]
    assert any(rev), "REFUSED: no revisit-tagged accept (image lacks chain-origin?)"
    tab = {}
    refusals = {}
    per_step = {s: [0, 0] for s in range(1, 7)}
    longs = []
    for k in range(6, len(iv)):
        ref = st.mean(iv[k-6:k]); x = iv[k]; r = rev[k+1]
        c = "LONG" if x > 1.25 * ref else "SHORT" if x < 0.75 * ref else "normal"
        tab[(c, r)] = tab.get((c, r), 0) + 1
        per_step[step[k+1]][1 if r else 0] += 1
        if c == "LONG":
            p = pad[k+1]
            longs.append((step[k+1], "REV" if r else "edge", x, round(ref),
                          None if p is None else f"early{p >> 4}/filt{p & 15}"))
        if pad[k+1] is not None:
            refs = refusals.setdefault(c, [0, 0, 0])
            refs[0] += pad[k+1] >> 4; refs[1] += pad[k+1] & 15; refs[2] += 1
    n = sum(tab.values())
    print(f"{path.split('/')[-1]}: {n} accepts, revisit-originated {sum(v for (c,r),v in tab.items() if r)} ({100*sum(v for (c,r),v in tab.items() if r)/n:.1f}%)")
    for c in ("normal", "LONG", "SHORT"):
        e, r = tab.get((c, False), 0), tab.get((c, True), 0)
        print(f"   {c:6s}: edge {e:4d}  revisit {r:4d}  -> revisit share {100*r/max(1,e+r):5.1f}%")
    for c in ("normal", "LONG", "SHORT"):
        r = refusals.get(c)
        if r and r[2]:
            print(f"   {c:6s}: mean refusals before accept -- too-early {r[0]/r[2]:.2f}, filter {r[1]/r[2]:.2f}  (n={r[2]})")
    print("   revisit share by step:", "  ".join(f"s{s}:{100*v[1]/max(1,sum(v)):.0f}%" for s, v in per_step.items()))
    print("   LONG intervals (step, origin, interval, mean):", longs[:14])
