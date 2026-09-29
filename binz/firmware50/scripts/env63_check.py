#!/usr/bin/env python3
"""ENV-63 (A6 stage 2): apply the amended, predeclared rule mechanically to the walk captures (no hand assembly).
Per run: reason, ceiling, coast, hold, worst block, late_arms, spent_max_us, thin_count, ci_min, mdep_n.
Rule: every walk run late_arms == 0 and spent_max_us <= 11; 725: worst < 4750 in the 3 confirmatory holds,
no foldback. Prints PASS/FAIL per criterion. Refuses if a rung has fewer than 3 captures."""
import glob, os, re, sys
from collections import defaultdict
runs = defaultdict(list)
INCOMPLETE = []
for f in sorted(glob.glob("captures/2026-09-29/env63-r[0-9]*_01.txt")):
    t = open(f).read()
    g = lambda k: (lambda m: int(m.group(1)) if m else None)(re.search(k + r"=(-?\d+)", t))
    hm = re.search(r"BEMFCURRENT.* hold_ma=(\d+)", t)
    rung = int(re.search(r"env63-r(\d+)-", f).group(1))
    if rung < 525:
        continue
    if g("BEMFDONE reason") is None:
        # ENV-52 review: a capture with no BEMFDONE is a failure to report, not a skip.
        print(f"  INCOMPLETE CAPTURE (no BEMFDONE): {os.path.basename(f)}"); INCOMPLETE.append(os.path.basename(f)); continue
    runs[rung].append(dict(f=os.path.basename(f)[:14], reason=g("BEMFDONE reason"), ceil=g("ceiling_tenths"),
        coast=g("coast_ehz"), hold=int(hm.group(1)) if hm else None, worst=g("worst_ma"), late=g("late_arms"),
        spent=g("spent_max_us"), thin=g("thin_count"), cimin=g("ci_min_us"), mdep=g("mdep_n")))
ok = True
for rung in sorted(runs):
    rs = runs[rung]
    for r in rs:
        print(f"{rung} {r}")
    # ENV-51/52: a non-normal stop (reason != 2) is a failure -- checked BEFORE the run-count
    # test, because a latch usually ends a rung before its third run.
    stopped = [(r["f"], r["reason"]) for r in rs if r["reason"] != 2]
    if stopped: ok = False; print(f"  {rung}: STOP-REASON FAIL {stopped}")
    if len(rs) < 3:
        print(f"  {rung}: only {len(rs)} captures"); continue
    bad = [r["f"] for r in rs if r["late"] != 0 or (r["spent"] or 99) > 11]
    if bad: ok = False; print(f"  {rung}: TIMING FAIL {bad}")
    if rung == 725:
        w = [r["worst"] for r in rs]; fb = [r for r in rs if r["ceil"] != 725]
        print(f"  725 confirmatory worst {w} -> {'PASS' if all(x < 4750 for x in w) else 'FAIL'}; foldbacks {len(fb)}")
        ok &= all(x < 4750 for x in w) and not fb
if INCOMPLETE: ok = False
need = [525, 550, 575, 600, 625, 650, 675, 700, 725]
missing = [r for r in need if len(runs.get(r, [])) < 3]
if missing:
    print("RULE: INCOMPLETE (rungs lacking 3 completed captures:", missing, ")", "- so far", "no failure" if ok else "FAIL")
else:
    print("RULE (timing + 725 criteria):", "PASS" if ok else "FAIL")
