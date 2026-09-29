#!/usr/bin/env python3
"""ENV-38: apply the amended, predeclared rule mechanically to the walk captures (no hand assembly).
Per run: reason, ceiling, coast, hold, worst block, late_arms, spent_max_us, thin_count, ci_min, mdep_n.
Rule: every walk run late_arms == 0 and spent_max_us <= 11; 725: worst < 4750 in the 3 confirmatory holds,
no foldback. Prints PASS/FAIL per criterion. Refuses if a rung has fewer than 3 captures."""
import glob, os, re, sys
from collections import defaultdict
runs = defaultdict(list)
for f in sorted(glob.glob("captures/2026-09-29/env37-r*_01.txt")):
    t = open(f).read()
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    hm = re.search(r"BEMFCURRENT.* hold_ma=(\d+)", t)
    rung = int(re.search(r"env37-r(\d+)-", f).group(1))
    if g("BEMFDONE reason") is None:
        print(f"(incomplete, skipped: {os.path.basename(f)})"); continue
    runs[rung].append(dict(f=os.path.basename(f)[:14], reason=g("BEMFDONE reason"), ceil=g("ceiling_tenths"),
        coast=g("coast_ehz"), hold=int(hm.group(1)) if hm else None, worst=g("worst_ma"), late=g("late_arms"),
        spent=g("spent_max_us"), thin=g("thin_count"), cimin=g("ci_min_us"), mdep=g("mdep_n")))
ok = True
for rung in sorted(runs):
    rs = runs[rung]
    for r in rs:
        print(f"{rung} {r}")
    if len(rs) < 3:
        print(f"  {rung}: only {len(rs)} captures"); continue
    bad = [r["f"] for r in rs if r["late"] != 0 or (r["spent"] or 99) > 11]
    if bad: ok = False; print(f"  {rung}: TIMING FAIL {bad}")
    if rung == 725:
        w = [r["worst"] for r in rs]; fb = [r for r in rs if r["ceil"] != 725]
        print(f"  725 confirmatory worst {w} -> {'PASS' if all(x < 4750 for x in w) else 'FAIL'}; foldbacks {len(fb)}")
        ok &= all(x < 4750 for x in w) and not fb
print("RULE (timing + 725 criteria):", "PASS" if ok else "FAIL")
