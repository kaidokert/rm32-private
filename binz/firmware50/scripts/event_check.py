#!/usr/bin/env python3
"""ENV-85: classify the first >= 4750 mA hold block in a frozen block-ring v3 dump (SAGBLK sum bus n half duty).

    python scripts/event_check.py CAPTURE...

Trigger = the first block >= 4750 mA (the ring keeps ~256 blocks after it, then stops). Prints +-12 blocks (mA, bus,
duty). Classification (predeclared): (D) a duty change within the 3 blocks before the trigger; (S) no duty change in
the 10 blocks before and the current rising over >= 2 blocks into it; (T) otherwise, if the neighbours (+-1) are within
2 sd of the pre-trigger mean; else 'other'. Refuses dumps without the duty column or without a >= 4750 block.
"""
import re, statistics as st, sys

for p in sys.argv[1:]:
    t = open(p, errors="replace").read()
    name = p.replace("\\", "/").split("/")[-1]
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    zero, allow = g("zero_start"), g("ma_allow")
    rows = [[int(v) for v in l.split()[1:]] for l in t.splitlines() if l.startswith("SAGBLK ")]
    if not rows or len(rows[0]) != 5 or zero is None:
        print(f"{name}: REFUSED (no duty column / calibration)")
        continue
    ma = [(zero - r[0]) * 5000 / allow if r[2] == 100 else float("nan") for r in rows]
    bus = [r[1] for r in rows]
    duty = [r[4] for r in rows]
    trig = next((i for i, x in enumerate(ma) if x == x and x >= 4750), None)
    if trig is None:
        print(f"{name}: REFUSED (no block >= 4750 in the ring; max {max(x for x in ma if x == x):.0f})")
        continue
    pre = [x for x in ma[max(0, trig - 60):trig - 3] if x == x]
    m, sd = st.mean(pre), st.pstdev(pre)
    dchg3 = any(duty[j] != duty[j - 1] for j in range(max(1, trig - 3), trig + 1))
    dchg10 = any(duty[j] != duty[j - 1] for j in range(max(1, trig - 10), trig + 1))
    rising = trig >= 2 and ma[trig - 1] > m + sd and ma[trig - 2] > m
    nb_ok = all(abs(ma[j] - m) <= 2 * sd for j in (trig - 1, trig + 1) if 0 <= j < len(ma) and ma[j] == ma[j])
    cls = "D (duty-driven)" if dchg3 else ("S (spontaneous surge)" if (not dchg10 and rising) else ("T (tail)" if nb_ok else "other"))
    print(f"{name}: trigger block {trig}/{len(rows)} = {ma[trig]:.0f} mA; pre-mean {m:.0f} sd {sd:.0f} -> {cls}")
    for j in range(max(0, trig - 12), min(len(rows), trig + 13)):
        mark = "<<" if j == trig else "  "
        print(f"   {j - trig:+3d} {ma[j]:6.0f} mA  bus {bus[j]}  duty {duty[j]} {mark}")
